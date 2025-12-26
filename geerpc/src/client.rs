use std::collections::HashMap;
use std::sync::Arc;

use crate::ConnectFailedSnafu;
use crate::DeserializeFailedSnafu;
use crate::Error;
use crate::RPCEnvelope;
use crate::RecvFailedSnafu;
use crate::Result;
use crate::SendFailedSnafu;
use crate::read_raw_frame;
use crate::write_frame;
use snafu::prelude::*;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::oneshot;

type PendingRequestsMap = Arc<Mutex<HashMap<u64, oneshot::Sender<Vec<u8>>>>>;

pub struct RPCClient {
    pub read_half: Arc<Mutex<ReadHalf<TcpStream>>>,
    pub write_half: Arc<Mutex<WriteHalf<TcpStream>>>,
    pub next_sequence_number: Arc<Mutex<u64>>,
    pub pending_requests: PendingRequestsMap,
}

impl RPCClient {
    pub fn new(stream: TcpStream) -> Self {
        let (read_half, write_half) = tokio::io::split(stream);
        Self {
            read_half: Arc::new(Mutex::new(read_half)),
            write_half: Arc::new(Mutex::new(write_half)),
            next_sequence_number: Arc::new(Mutex::new(1)),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

pub struct RPCClientBuilder {
    address: Option<String>,
}

impl RPCClientBuilder {
    pub fn new() -> Self {
        Self { address: None }
    }

    pub fn address(mut self, address: String) -> Self {
        self.address = Some(address);
        self
    }

    pub async fn build(self) -> Result<Arc<RPCClient>> {
        if self.address.is_none() {
            return Err(Error::AddressRequired);
        }
        let address = self.address.unwrap();
        tracing::info!("Attempting to connect to server at {}", address);
        let stream = TcpStream::connect(&address)
            .await
            .context(ConnectFailedSnafu)?;
        tracing::info!("Successfully connected to server at {}", address);
        let (read_half, write_half) = tokio::io::split(stream);
        let client = Arc::new(RPCClient {
            read_half: Arc::new(Mutex::new(read_half)),
            write_half: Arc::new(Mutex::new(write_half)),
            next_sequence_number: Arc::new(Mutex::new(1)),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
        });

        // Spawn the read loop automatically
        let read_client = client.clone();
        tokio::spawn(async move {
            if let Err(e) = read_client.read_loop().await {
                tracing::error!("Read loop error: {}", e);
            }
        });

        Ok(client)
    }
}

async fn write_frame_with_metadata(
    write_half: &mut WriteHalf<TcpStream>,
    sequence_number: u64,
    service: &str,
    method: &str,
    payload: Vec<u8>,
) -> Result<()> {
    let envelope = RPCEnvelope {
        version: 1,
        sequence_number,
        service_name: service.to_string(),
        method_name: method.to_string(),
        status: None,
        payload,
    };
    write_frame(write_half, envelope).await
}

impl RPCClient {
    pub async fn call(&self, service: &str, method: &str, payload: Vec<u8>) -> Result<Vec<u8>> {
        let sequence_number = {
            let mut n = self.next_sequence_number.lock().await;
            let result = *n;
            *n += 1;
            result
        };
        tracing::info!(
            "Calling RPC: service={}, method={}, seq={}, payload_size={}",
            service,
            method,
            sequence_number,
            payload.len()
        );
        let (tx, rx) = oneshot::channel::<Vec<u8>>();
        self.pending_requests
            .lock()
            .await
            .insert(sequence_number, tx);
        write_frame_with_metadata(
            &mut *self.write_half.lock().await,
            sequence_number,
            service,
            method,
            payload,
        )
        .await?;
        tracing::info!("Request sent: seq={}", sequence_number);
        let result = rx.await.context(RecvFailedSnafu)?;
        tracing::info!(
            "Response received: seq={}, response_size={}",
            sequence_number,
            result.len()
        );
        Ok(result)
    }

    pub(crate) async fn read_loop(&self) -> Result<()> {
        tracing::info!("Starting client read loop");
        let mut read_half = self.read_half.lock().await;
        loop {
            let frame = match read_raw_frame(&mut *read_half).await {
                Ok(frame) => frame,
                Err(e) => {
                    tracing::info!("Read loop ending: {}", e);
                    break;
                }
            };
            let envelope: RPCEnvelope =
                bincode::deserialize(&frame).context(DeserializeFailedSnafu)?;
            let sequence_number = envelope.sequence_number;
            tracing::info!(
                "Received response frame: seq={}, service={}, method={}",
                sequence_number,
                envelope.service_name,
                envelope.method_name
            );
            let tx = self.pending_requests.lock().await.remove(&sequence_number);
            if let Some(tx) = tx {
                tracing::info!(
                    "Matched response to pending request: seq={}",
                    sequence_number
                );
                tx.send(envelope.payload)
                    .map_err(|_| SendFailedSnafu.build())?;
            } else {
                tracing::info!("No pending request found for seq={}", sequence_number);
            }
        }

        let pending_count = self.pending_requests.lock().await.len();
        tracing::info!("Cleaning up {} pending requests", pending_count);
        let mut pending = self.pending_requests.lock().await;
        for (_, tx) in pending.drain() {
            tx.send(Vec::new()).map_err(|_| SendFailedSnafu.build())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::RPCServer;

    #[tokio::test]
    async fn test_client_builder_missing_address() {
        let result = RPCClientBuilder::new().build().await;
        assert!(result.is_err());
        if let Err(e) = result {
            match e {
                crate::Error::AddressRequired => (),
                _ => panic!("Expected AddressRequired error"),
            }
        }
    }

    #[tokio::test]
    async fn test_client_builder_invalid_address() {
        let result = RPCClientBuilder::new()
            .address("invalid:address".to_string())
            .build()
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_client_single_call() {
        // Setup server
        let mut server = RPCServer::new();
        server.register_service(
            "Math".to_string(),
            "Double".to_string(),
            Box::new(|payload: Vec<u8>| {
                Box::pin(async move {
                    let result: Vec<u8> = payload.iter().map(|x| x * 2).collect();
                    Ok(result)
                })
            }),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handlers = std::sync::Arc::new(server.handlers);

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let _ = crate::server::handle_connection(stream, handlers).await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Create client
        let client = std::sync::Arc::new(
            RPCClientBuilder::new()
                .address(format!("{}", addr))
                .build()
                .await
                .unwrap(),
        );

        // Spawn read loop
        let read_client = client.clone();
        tokio::spawn(async move {
            let _ = read_client.read_loop().await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

        // Make call
        let result = client
            .call("Math", "Double", vec![1, 2, 3, 4])
            .await
            .unwrap();
        assert_eq!(result, vec![2, 4, 6, 8]);
    }

    #[tokio::test]
    async fn test_client_multiple_sequential_calls() {
        // Setup server
        let mut server = RPCServer::new();
        server.register_service(
            "Echo".to_string(),
            "echo".to_string(),
            Box::new(|payload: Vec<u8>| Box::pin(async move { Ok(payload) })),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handlers = std::sync::Arc::new(server.handlers);

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let _ = crate::server::handle_connection(stream, handlers).await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Create client
        let client = std::sync::Arc::new(
            RPCClientBuilder::new()
                .address(format!("{}", addr))
                .build()
                .await
                .unwrap(),
        );

        // Spawn read loop
        let read_client = client.clone();
        tokio::spawn(async move {
            let _ = read_client.read_loop().await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

        // Make multiple sequential calls
        for i in 0..5 {
            let payload = vec![i];
            let result = client.call("Echo", "echo", payload.clone()).await.unwrap();
            assert_eq!(result, payload);
        }
    }

    #[tokio::test]
    async fn test_client_sequence_numbers() {
        // Setup server
        let mut server = RPCServer::new();
        server.register_service(
            "Test".to_string(),
            "method".to_string(),
            Box::new(|payload: Vec<u8>| Box::pin(async move { Ok(payload) })),
        );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handlers = std::sync::Arc::new(server.handlers);

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let _ = crate::server::handle_connection(stream, handlers).await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Create client
        let client = std::sync::Arc::new(
            RPCClientBuilder::new()
                .address(format!("{}", addr))
                .build()
                .await
                .unwrap(),
        );

        // Verify initial sequence number is 1
        assert_eq!(*client.next_sequence_number.lock().await, 1);

        // Spawn read loop
        let read_client = client.clone();
        tokio::spawn(async move {
            let _ = read_client.read_loop().await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

        // Make a call
        let _ = client.call("Test", "method", vec![1]).await.unwrap();

        // Verify sequence number incremented
        assert_eq!(*client.next_sequence_number.lock().await, 2);
    }
}
