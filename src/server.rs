use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use chat::{
    chat_req_server::{ChatReq, ChatReqServer},
    Empty, Msg, Req,
};

pub mod chat {
    tonic::include_proto!("chat"); /

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

    async fn connect_server(
        &self,
        request: Request<Req>,
    ) -> Result<Response<Self::ConnectServerStream>, Status> {
        let name = request.into_inner().user_name;
        let (stream_tx, stream_rx) = mpsc::channel(1); // Fn usage

        let (tx, mut rx) = mpsc::channel(1);
        {
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

    async fn send_msg(&self, request: Request<Msg>) -> Result<Response<Msg>, Status> {
        let req_data = request.into_inner();
        let user_name = req_data.user_name;
        let content = req_data.content;
        let msg = Msg { user_name, content };

        Ok(Response::new(msg))
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
