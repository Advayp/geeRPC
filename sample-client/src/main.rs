use crate::ping_client::{PingClient, PingRequest};

pub mod ping_client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let client = PingClient::try_new("127.0.0.1:8080".to_string()).await?;
    let response = client
        .ping(PingRequest {
            message: "Hello, world!".to_string(),
        })
        .await?;
    println!("Response: {}", response.message);
    Ok(())
}
