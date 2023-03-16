use std::{convert::Infallible, env, net::SocketAddr};

use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql_warp::{graphql, BadRequest, Response};
use tonic::transport::Channel;
use warp::{http::Response as HttpResponse, Filter, Rejection};

use pb::{
    greeter_client::GreeeterClient,
    greeter_graphql::{build_graphql_schema, GreeterSchema},
};

mod pb {
    // protoから生成されたRustコードをインポート
    tonic::include_proto!("helloworld");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = ([0, 0, 0, 0], 4000).into();
    println!("listening on http://localhost:4000");

    // gRPC clientに接続
    let grpc_client = GreeterClient::connect("http://localhost:4001").await?;

    // GraphQL schemaをビルドし、gRPC clientをデータとして渡す。
    let schema = build_graphql_schema::<Channel>().data(grpc_client).finish();

    // GraphQLサービスの作成
    let graphql_post = graphql(schema).and_then(
        move |(schema, request): (GreeterSchema<_>, async_graphql::Request)| async move {
            let response = schema.execute(request).await;
            Ok::<_, Infallible>(Response::from(response))
        },
    );

    // GraphQL playgroundで使用できるようにする
    let graphql_playground = warp::path::end().and(warp::get()).map(|| {
        HttpResponse::builder()
            .header("content-type", "text/html")
            .body(playground_source(GraphQLPlaygroundConfig::new("/")))
    });

    // webサーバを実行
    let routes = graphql_playground.or(graphql_post);
    warp::serve(routes).run(addr).await;

    Ok(())
}
