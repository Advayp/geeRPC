use crate::ping_server::{PingRequest, PingResponse, PingServer, PingService};
use async_trait::async_trait;
use geerpc::server::RPCServer;

pub mod ping_server;

pub struct PingServerImpl;

#[async_trait]
impl PingService for PingServerImpl {
    async fn ping(&self, request: PingRequest) -> Result<PingResponse, geerpc::Error> {
        Ok(PingResponse {
            message: format!("Pong: {}", request.message),
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let mut server = RPCServer::new();
    PingServer::new(PingServerImpl).register(&mut server);
    server.serve("127.0.0.1:8080").await?;
    Ok(())
}
