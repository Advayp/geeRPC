use geerpc::server::RPCServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let server = RPCServer::new();
    server.serve("127.0.0.1:8080").await?;
    Ok(())
}
