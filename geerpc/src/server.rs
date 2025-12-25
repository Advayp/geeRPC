use crate::AcceptFailedSnafu;
use crate::BindFailedSnafu;
use crate::MethodName;
use crate::Result;
use crate::RpcStatus;
use crate::ServiceName;
use snafu::prelude::*;
use std::collections::HashMap;
use std::future::Future;
use std::net::TcpListener;
use std::net::TcpStream;
use std::pin::Pin;
use std::sync::Arc;

type Handler = Box<
    dyn Fn(Vec<u8>) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, RpcStatus>> + Send>>
        + Send
        + Sync,
>;

pub struct RPCServer {
    handlers: HashMap<(ServiceName, MethodName), Handler>,
}

impl RPCServer {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn serve(self, address: &str) -> Result<()> {
        let listener = TcpListener::bind(address).context(BindFailedSnafu)?;
        let handlers = Arc::new(self.handlers);
        tracing::info!("Server is running on {}", address);
        loop {
            let (stream, _) = listener.accept().context(AcceptFailedSnafu)?;
            tracing::info!("Accepted connection from {}", stream.peer_addr().unwrap());
            let handlers = handlers.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, handlers).await {
                    tracing::error!("Error handling connection: {}", e);
                }
            });
        }
    }

    pub fn register_service(&mut self, service: ServiceName, method: MethodName, handler: Handler) {
        self.handlers.insert((service, method), handler);
    }
}

async fn handle_connection(
    _stream: TcpStream,
    _handlers: Arc<HashMap<(ServiceName, MethodName), Handler>>,
) -> Result<()> {
    Ok(())
}
