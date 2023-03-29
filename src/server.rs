use futures::Stream; // 非同期ストリームを実装するために使用
use std::collections::HashMap; // キーと値のペアを格納するため使用
use std::pin::Pin; // 大きなデータ構造を保持するために使用される非同期のピン留めポインタ
use std::sync::Arc; // スレッドセーフな参照カウントを提供
use tokio::sync::{mpsc, RwLock}; // 非同期通信を可能にするマルチプロデューサ/マルチコンシューマのチャンネル、非同期的にアクセス可能な読み取り.書き込み可能なロックを提供
use tonic::transport::Server; // gRPCサーバーの開始と停止を行うために使用
use tonic::{Request, Response, Status}; // gRPCのリクエスト、レスポンス、ステータスを定義するために使用される

use chat::{
    chat_req_server::{ChatReq, ChatReqServer},
    Empty, Msg, Req,
};

pub mod chat {
    tonic::include_proto!("chat"); // The string specified here must match the proto package name
}

// 複数のユーザーによって共有されるチャットメッセージを管理するために使用する
// ユーザー名をキーに、mpsc::Sender<Msg>を値として持つHashMap
//
#[derive(Debug)]
struct Shared {
    senders: HashMap<String, mpsc::Sender<Msg>>,
}

impl Shared {
    fn new() -> Self {
        Shared {
            senders: HashMap::new(),
        }
    }

    async fn broadcast(&self, msg: Msg) {
        // 現在接続されている全てのクライアントにメッセージをブロードキャストする
        // senders HashMapに格納されている全てのmpsc::Senderに対してclone()メソッドを呼び出してそれぞれのチャネルを複製し、sendメソッドで送信する
        // clone()メソッドを呼び出すこどで、self.senders内のmpsc::Senderの所有権を移動せず、broadcast()内で複数回使えるようにする
        // clone()されたチャネルの一つがエラーを返す場合でも、他のチャネルでメッセージの送信を続けることができる
        for (name, tx) in &self.senders {
            match tx.send(msg.clone()).await {
                Ok(_) => {}
                Err(_) => {
                    println!("[Broadcast] SendError: to {}, {:?}", name, msg)
                }
            }
        }
    }
}

#[derive(Debug)]
struct ChatService {
    shared: Arc<RwLock<Shared>>,
}

impl ChatService {
    fn new(shared: Arc<RwLock<Shared>>) -> Self {
        ChatService { shared }
    }
}

#[tonic::async_trait]
impl ChatReq for ChatService {
    type ConnectServerStream =
        Pin<Box<dyn Stream<Item = Result<Msg, Status>> + Send + Sync + 'static>>;

    // クライアントからの接続要求があった場合に、サーバーがクライアントに送信するためのストリームを作成して返す
    async fn connect_server(
        &self,
        request: Request<Req>,
    ) -> Result<Response<Self::ConnectServerStream>, Status> {
        let name = request.into_inner().user_name;
        let (stream_tx, stream_rx) = mpsc::channel(1); // Fn usage

        // クライアントからの接続要求があった場合に、サーバーがクライアントに送信するためのストリームを作成して返却する
        let (tx, mut rx) = mpsc::channel(1);
        {
            // sendersマップに新しい接続のためのmpsc::Senderを追加する
            self.shared.write().await.senders.insert(name.clone(), tx);
        }

        let shared_clone = self.shared.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match stream_tx.send(Ok(msg)).await {
                    Ok(_) => {}
                    Err(_) => {
                        println!("[Remote] stream tx sending error. Remote {}", &name);
                        shared_clone.write().await.senders.remove(&name);
                    }
                }
            }
        });

        Ok(Response::new(Box::pin(
            tokio_stream::wrappers::ReceiverStream::new(stream_rx),
        )))
    }
    async fn send_msg(&self, request: Request<Msg>) -> Result<Response<Empty>, Status> {
        let req_data = request.into_inner();
        let user_name = req_data.user_name;
        let content = req_data.content;
        let msg = Msg { user_name, content };

        Ok(Response::new(Empty {}))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "0.0.0.0:50051".parse().unwrap();
    println!("SendMsg Server listening on: {}", addr);

    let shared = Arc::new(RwLock::new(Shared::new()));
    let chat_service = ChatService::new(shared.clone());

    Server::builder()
        .add_service(ChatReqServer::new(chat_service))
        .serve(addr)
        .await?;

    Ok(())
}
