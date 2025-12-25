use crate::AcceptFailedSnafu;
use crate::BindFailedSnafu;
use crate::DeserializeFailedSnafu;
use crate::FrameTooLargeSnafu;
use crate::MAX_FRAME_SIZE;
use crate::MethodName;
use crate::RPCEnvelope;
use crate::RPCStatus;
use crate::ReadFailedSnafu;
use crate::Result;
use crate::SerializeFailedSnafu;
use crate::ServiceName;
use crate::StatusCode;
use crate::WriteFailedSnafu;
use snafu::prelude::*;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

type Handler = Box<
    dyn Fn(Vec<u8>) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, RPCStatus>> + Send>>
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

    pub async fn serve(self, address: &str) -> Result<()> {
        let listener = TcpListener::bind(address).await.context(BindFailedSnafu)?;
        let handlers = Arc::new(self.handlers);
        tracing::info!("Server is running on {}", address);
        loop {
            let (stream, _) = listener.accept().await.context(AcceptFailedSnafu)?;
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

async fn read_raw_frame(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut buffer = [0; 4];
    stream
        .read_exact(&mut buffer)
        .await
        .context(ReadFailedSnafu)?;
    let length = u32::from_be_bytes(buffer) as usize;

    if length > MAX_FRAME_SIZE {
        return Err(FrameTooLargeSnafu { length }.build());
    }

    let mut buffer = vec![0; length];
    stream
        .read_exact(&mut buffer)
        .await
        .context(ReadFailedSnafu)?;
    Ok(buffer)
}

async fn write_frame(stream: &mut TcpStream, envelope: RPCEnvelope) -> Result<()> {
    let frame = serde_yaml::to_string(&envelope)
        .context(SerializeFailedSnafu)?
        .into_bytes();
    let length = frame.len();
    let mut buffer = vec![0; 4];
    buffer[0..4].copy_from_slice(&(length as u32).to_be_bytes());
    stream.write_all(&buffer).await.context(WriteFailedSnafu)?;
    stream.write_all(&frame).await.context(WriteFailedSnafu)?;
    Ok(())
}

async fn write_error_frame(stream: &mut TcpStream, error: RPCStatus) -> Result<()> {
    let envelope = RPCEnvelope {
        version: 1,
        request_id: 0,
        service_name: "".to_string(),
        method_name: "".to_string(),
        status: Some(error.clone()),
        payload: Vec::new(),
    };
    write_frame(stream, envelope).await
}

async fn handle_connection(
    mut stream: TcpStream,
    handlers: Arc<HashMap<(ServiceName, MethodName), Handler>>,
) -> Result<()> {
    loop {
        let frame = read_raw_frame(&mut stream).await?;
        let envelope: RPCEnvelope =
            serde_yaml::from_slice(&frame).context(DeserializeFailedSnafu)?;

        let handler = handlers.get(&(envelope.service_name.clone(), envelope.method_name.clone()));

        if let Some(handler) = handler {
            let result = handler(envelope.payload).await;

            let result = match result {
                Ok(result) => result,
                Err(e) => {
                    return write_error_frame(&mut stream, e).await;
                }
            };

            write_frame(
                &mut stream,
                RPCEnvelope {
                    version: 1,
                    request_id: envelope.request_id.clone(),
                    service_name: envelope.service_name.clone(),
                    method_name: envelope.method_name.clone(),
                    status: Some(RPCStatus {
                        code: StatusCode::Ok,
                        message: "Success".to_string(),
                    }),
                    payload: result,
                },
            )
            .await?;
        } else {
            write_error_frame(
                &mut stream,
                RPCStatus {
                    code: StatusCode::NotFound,
                    message: "Method not found".to_string(),
                },
            )
            .await?;
        }
    }
}
