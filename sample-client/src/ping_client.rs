use std::sync::Arc;

use async_trait::async_trait;
use geerpc::{
    RPCStatus, StatusCode,
    client::{RPCClient, RPCClientBuilder},
    server::RPCServer,
};
use serde::{Deserialize, Serialize};

const SERVICE_NAME: &str = "Ping";
const METHOD_PING: &str = "ping";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PingRequest {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PingResponse {
    pub message: String,
}

pub struct PingClient {
    inner: Arc<RPCClient>,
}

impl PingClient {
    pub async fn try_new(address: String) -> Result<Self, geerpc::Error> {
        let inner = RPCClientBuilder::new().address(address).build().await?;
        Ok(Self { inner })
    }

    pub async fn ping(&self, request: PingRequest) -> Result<PingResponse, geerpc::Error> {
        let response = self
            .inner
            .call(SERVICE_NAME, METHOD_PING, geerpc::encode_payload(request)?)
            .await?;

        geerpc::decode_payload::<PingResponse>(response)
    }
}

#[async_trait]
pub trait PingService: Send + Sync + 'static {
    async fn ping(&self, request: PingRequest) -> Result<PingResponse, geerpc::Error>;
}

pub struct PingServer<T: PingService> {
    inner: Arc<T>,
}

impl<T: PingService> PingServer<T> {
    pub fn new(service: T) -> Self {
        Self {
            inner: Arc::new(service),
        }
    }

    pub fn register(&self, server: &mut RPCServer) {
        let inner = self.inner.clone();
        server.register_service(
            SERVICE_NAME.to_string(),
            METHOD_PING.to_string(),
            Box::new(move |payload: Vec<u8>| {
                let inner = inner.clone();
                Box::pin(async move {
                    let request =
                        geerpc::decode_payload::<PingRequest>(payload).map_err(|_| RPCStatus {
                            code: StatusCode::Internal,
                            message: "Failed to decode request".to_string(),
                        })?;
                    let response = inner.ping(request).await.map_err(|_| RPCStatus {
                        code: StatusCode::Internal,
                        message: "Failed to ping".to_string(),
                    })?;
                    geerpc::encode_payload(response).map_err(|_| RPCStatus {
                        code: StatusCode::Internal,
                        message: "Failed to encode response".to_string(),
                    })
                })
            }),
        );
    }
}
