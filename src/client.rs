use chat::chat_req_client::ChatReqClient;
use chat::{Empty, Msg, Req};
use std::io::{self, Write};
use tonic::transport::Channel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = GreeterClient::connect("http://[::1]:50051").await?;

    let client = ChatReqClient::new(channel);

    loop {
        let user_name = "YourUsername".to_string();

        print!("Enter a user_name (or 'exit' to quit): ");
        io::stdout().flush()?;

        io::stdin().read_line(&mut user_name)?;

        if user_name.trim() == "exit" {
            break;
        }

        let mut message = String::new();
        print!("Enter a message (or 'exit' to quit): ");
        io::stdout().flush()?;

        io::stdin().read_line(&mut message)?;

        if message.trim() == "exit" {
            break;
        }

        let msg = Msg {
            user_name: user_name.trim().to_string(),
            content: message.trim().to_string(),
        };

        client.send_msg(tonic::Request::new(msg)).await?;
    }

    Ok(())
}
