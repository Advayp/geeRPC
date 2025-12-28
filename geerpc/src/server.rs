use crate::read_raw_frame;
use crate::write_frame;
use crate::AcceptFailedSnafu;
use crate::BindFailedSnafu;
use crate::DeserializeFailedSnafu;
use crate::MethodName;
use crate::RPCEnvelope;
use crate::RPCStatus;
use crate::Result;
use crate::ServiceName;
use crate::StatusCode;
use snafu::prelude::*;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

type Handler = Box<
    dyn Fn(Vec<u8>) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, RPCStatus>> + Send>>
        + Send
        + Sync,
>;

pub struct RPCServer {
    pub handlers: HashMap<(ServiceName, MethodName), Handler>,
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
            tokio::select! {
                result = listener.accept() => {
                    let (stream, _) = result.context(AcceptFailedSnafu)?;
                    tracing::info!("Accepted connection from {}", stream.peer_addr().unwrap());
                    let handlers = handlers.clone();
                    tokio::spawn(async move {
                        let peer_addr = stream.peer_addr().ok();
                        if let Err(e) = handle_connection(stream, handlers).await {
                            tracing::warn!("Error handling connection from {:?}: {}", peer_addr, e);
                        }
                    });
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Shutting down server");
                    break;
                }
            }
        }
        Ok(())
    }

    pub fn register_service(&mut self, service: ServiceName, method: MethodName, handler: Handler) {
        self.handlers.insert((service, method), handler);
    }
}

async fn write_error_frame<W>(stream: &mut W, error: RPCStatus, sequence_number: u64) -> Result<()>
where
    W: tokio::io::AsyncWriteExt + Unpin,
{
    let envelope = RPCEnvelope {
        version: 1,
        sequence_number,
        service_name: "".to_string(),
        method_name: "".to_string(),
        status: Some(error.clone()),
        payload: Vec::new(),
    };
    write_frame(stream, envelope).await
}

// This function is internal but exposed for integration testing
// It should not be used directly by library consumers
#[cfg_attr(not(any(test, feature = "internal-testing")), allow(dead_code))]
#[cfg_attr(any(test, feature = "internal-testing"), doc(hidden))]
pub async fn handle_connection(
    stream: TcpStream,
    handlers: Arc<HashMap<(ServiceName, MethodName), Handler>>,
) -> Result<()> {
    let peer_addr = stream.peer_addr().ok();
    tracing::info!("Starting connection handler for {:?}", peer_addr);

    // Split stream into read and write halves for concurrent processing
    let (read_half, write_half) = tokio::io::split(stream);
    let write_half = Arc::new(tokio::sync::Mutex::new(write_half));

    // Channel to send responses from handler tasks to the writer task
    let (response_tx, mut response_rx) = mpsc::unbounded_channel::<RPCEnvelope>();

    // Spawn writer task that writes responses as they complete
    let write_half_clone = write_half.clone();
    let peer_addr_clone = peer_addr;
    let writer_handle = tokio::spawn(async move {
        while let Some(envelope) = response_rx.recv().await {
            let mut write = write_half_clone.lock().await;
            if let Err(e) = write_frame(&mut *write, envelope.clone()).await {
                tracing::warn!("Error writing response for {:?}: {}", peer_addr_clone, e);
                break;
            }
        }
    });

    // Main loop: read frames and spawn handler tasks
    let mut read_half = read_half;
    loop {
        let frame = match read_raw_frame(&mut read_half).await {
            Ok(frame) => frame,
            Err(crate::Error::ConnectionClosed) => {
                tracing::info!("Connection closed gracefully from {:?}", peer_addr);
                drop(response_tx); // Close channel to signal writer to stop
                let _ = writer_handle.await;
                return Ok(());
            }
            Err(e) => {
                tracing::info!("Connection error from {:?}: {}", peer_addr, e);
                drop(response_tx); // Close channel to signal writer to stop
                let _ = writer_handle.await;
                return Err(e);
            }
        };

        let envelope: RPCEnvelope =
            match bincode::deserialize(&frame).context(DeserializeFailedSnafu) {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("Failed to deserialize frame from {:?}: {}", peer_addr, e);
                    continue;
                }
            };

        tracing::info!(
            "Received request: service={}, method={}, seq={}, payload_size={}",
            envelope.service_name,
            envelope.method_name,
            envelope.sequence_number,
            envelope.payload.len()
        );

        // Check if handler exists using references (no clone needed for check)
        let handler_exists = handlers
            .get(&(envelope.service_name.clone(), envelope.method_name.clone()))
            .is_some();

        if handler_exists {
            // Move fields from envelope into the task (no need to clone before check)
            let service_name = envelope.service_name;
            let method_name = envelope.method_name;
            let sequence_number = envelope.sequence_number;
            let payload = envelope.payload;
            let handlers_clone = handlers.clone();
            let response_tx = response_tx.clone();
            let peer_addr_for_task = peer_addr;

            // Spawn a task to handle this request concurrently
            tokio::spawn(async move {
                // Get handler from the cloned handlers map
                let handler = handlers_clone.get(&(service_name.clone(), method_name.clone()));
                if let Some(handler) = handler {
                    tracing::info!(
                        "Found handler for {}.{}, executing...",
                        service_name,
                        method_name
                    );
                    let result = handler(payload).await;

                    let response_envelope = match result {
                        Ok(result) => {
                            tracing::info!(
                                "Handler succeeded for {}.{}, response_size={}",
                                service_name,
                                method_name,
                                result.len()
                            );
                            RPCEnvelope {
                                version: 1,
                                sequence_number,
                                service_name: service_name.clone(),
                                method_name: method_name.clone(),
                                status: Some(RPCStatus {
                                    code: StatusCode::Ok,
                                    message: "Success".to_string(),
                                }),
                                payload: result,
                            }
                        }
                        Err(e) => {
                            tracing::info!(
                                "Handler returned error for {}.{}: {:?}",
                                service_name,
                                method_name,
                                e
                            );
                            // Create error response envelope
                            RPCEnvelope {
                                version: 1,
                                sequence_number,
                                service_name: service_name.clone(),
                                method_name: method_name.clone(),
                                status: Some(e),
                                payload: Vec::new(),
                            }
                        }
                    };

                    // Send response to writer task
                    if response_tx.send(response_envelope).is_err() {
                        tracing::warn!(
                            "Failed to send response to writer task for {:?}",
                            peer_addr_for_task
                        );
                    }
                }
            });
        } else {
            // Handler not found - send error response through channel
            tracing::info!(
                "Handler not found for {}.{}",
                envelope.service_name,
                envelope.method_name
            );
            let error_envelope = RPCEnvelope {
                version: 1,
                sequence_number: envelope.sequence_number,
                service_name: envelope.service_name,
                method_name: envelope.method_name,
                status: Some(RPCStatus {
                    code: StatusCode::NotFound,
                    message: "Method not found".to_string(),
                }),
                payload: Vec::new(),
            };
            if response_tx.send(error_envelope).is_err() {
                tracing::warn!("Failed to send error response for {:?}", peer_addr);
                drop(response_tx);
                let _ = writer_handle.await;
                return Err(crate::Error::WriteFailed {
                    source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Channel closed"),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::MAX_FRAME_SIZE;

    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    // Helper function to create a test envelope
    fn create_test_envelope(service: &str, method: &str, payload: Vec<u8>) -> RPCEnvelope {
        RPCEnvelope {
            version: 1,
            sequence_number: 42,
            service_name: service.to_string(),
            method_name: method.to_string(),
            status: None,
            payload,
        }
    }

    // Helper function to serialize an envelope into a frame with length prefix
    fn serialize_frame(envelope: &RPCEnvelope) -> Vec<u8> {
        let payload_bytes = bincode::serialize(envelope).unwrap();
        let length = payload_bytes.len() as u32;

        let mut frame = Vec::new();
        frame.extend_from_slice(&length.to_be_bytes());
        frame.extend_from_slice(&payload_bytes);
        frame
    }

    // Helper function to deserialize a frame
    async fn deserialize_frame(data: &[u8]) -> RPCEnvelope {
        assert!(data.len() >= 4, "Frame too short");
        let length = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        assert_eq!(data.len() - 4, length, "Length mismatch");

        let payload_data = &data[4..];
        bincode::deserialize(payload_data).unwrap()
    }

    #[test]
    fn test_envelope_serialization() {
        let envelope = create_test_envelope("TestService", "TestMethod", vec![1, 2, 3, 4]);

        // Test bincode serialization
        let serialized = bincode::serialize(&envelope).unwrap();

        // Test deserialization
        let deserialized: RPCEnvelope = bincode::deserialize(&serialized).unwrap();
        assert_eq!(deserialized.service_name, "TestService");
        assert_eq!(deserialized.method_name, "TestMethod");
        assert_eq!(deserialized.version, 1);
        assert_eq!(deserialized.sequence_number, 42);
        assert_eq!(deserialized.payload, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_frame_serialization() {
        let envelope = create_test_envelope("MyService", "MyMethod", vec![10, 20, 30]);
        let frame = serialize_frame(&envelope);

        // Check that frame has length prefix
        assert!(frame.len() >= 4);
        let length = u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
        assert_eq!(frame.len() - 4, length);
    }

    #[tokio::test]
    async fn test_frame_deserialization() {
        let envelope = create_test_envelope("Service1", "Method1", vec![5, 10, 15]);
        let frame = serialize_frame(&envelope);

        let deserialized = deserialize_frame(&frame).await;
        assert_eq!(deserialized, envelope);
    }

    #[tokio::test]
    async fn test_rpc_server_successful_call() {
        // Create a server with a test handler
        let mut server = RPCServer::new();
        server.register_service(
            "Calculator".to_string(),
            "Add".to_string(),
            Box::new(|payload: Vec<u8>| {
                Box::pin(async move {
                    // Simple handler that doubles the input
                    let result: Vec<u8> = payload.iter().map(|x| x * 2).collect();
                    Ok(result)
                })
            }),
        );

        // Start server in background
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handlers = Arc::new(server.handlers);

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let _ = handle_connection(stream, handlers).await;
        });

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Connect as client
        let mut client = TcpStream::connect(addr).await.unwrap();

        // Send request
        let request = create_test_envelope("Calculator", "Add", vec![1, 2, 3]);
        let request_frame = serialize_frame(&request);
        client.write_all(&request_frame).await.unwrap();

        // Read response
        let mut length_buf = [0u8; 4];
        client.read_exact(&mut length_buf).await.unwrap();
        let length = u32::from_be_bytes(length_buf) as usize;

        let mut response_buf = vec![0u8; length];
        client.read_exact(&mut response_buf).await.unwrap();

        let response: RPCEnvelope = bincode::deserialize(&response_buf).unwrap();

        // Verify response
        assert_eq!(response.service_name, "Calculator");
        assert_eq!(response.method_name, "Add");
        assert_eq!(response.sequence_number, 42);
        assert_eq!(response.payload, vec![2, 4, 6]); // Doubled
        assert_eq!(response.status.as_ref().unwrap().code, StatusCode::Ok);
    }

    #[tokio::test]
    async fn test_rpc_server_method_not_found() {
        // Create server with no handlers
        let server = RPCServer::new();

        // Start server
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handlers = Arc::new(server.handlers);

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let _ = handle_connection(stream, handlers).await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Connect and send request for non-existent method
        let mut client = TcpStream::connect(addr).await.unwrap();
        let request = create_test_envelope("Unknown", "Method", vec![1, 2, 3]);
        let request_frame = serialize_frame(&request);
        client.write_all(&request_frame).await.unwrap();

        // Read response
        let mut length_buf = [0u8; 4];
        client.read_exact(&mut length_buf).await.unwrap();
        let length = u32::from_be_bytes(length_buf) as usize;

        let mut response_buf = vec![0u8; length];
        client.read_exact(&mut response_buf).await.unwrap();

        let response: RPCEnvelope = bincode::deserialize(&response_buf).unwrap();

        // Verify error response
        assert_eq!(response.sequence_number, 42); // Must match request
        assert_eq!(response.status.as_ref().unwrap().code, StatusCode::NotFound);
        assert_eq!(
            response.status.as_ref().unwrap().message,
            "Method not found"
        );
    }

    #[tokio::test]
    async fn test_multiple_requests() {
        // Create server with handler
        let mut server = RPCServer::new();
        server.register_service(
            "Echo".to_string(),
            "echo".to_string(),
            Box::new(|payload: Vec<u8>| Box::pin(async move { Ok(payload) })),
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handlers = Arc::new(server.handlers);

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let _ = handle_connection(stream, handlers).await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let mut client = TcpStream::connect(addr).await.unwrap();

        // Send multiple requests
        for i in 0..3 {
            let request = RPCEnvelope {
                version: 1,
                sequence_number: i,
                service_name: "Echo".to_string(),
                method_name: "echo".to_string(),
                status: None,
                payload: vec![i as u8],
            };

            let request_frame = serialize_frame(&request);
            client.write_all(&request_frame).await.unwrap();

            // Read response
            let mut length_buf = [0u8; 4];
            client.read_exact(&mut length_buf).await.unwrap();
            let length = u32::from_be_bytes(length_buf) as usize;

            let mut response_buf = vec![0u8; length];
            client.read_exact(&mut response_buf).await.unwrap();

            let response: RPCEnvelope = bincode::deserialize(&response_buf).unwrap();

            assert_eq!(response.sequence_number, i);
            assert_eq!(response.payload, vec![i as u8]);
        }
    }

    #[test]
    fn test_status_code_serialization() {
        let status = RPCStatus {
            code: StatusCode::Ok,
            message: "Success".to_string(),
        };

        let serialized = bincode::serialize(&status).unwrap();
        let deserialized: RPCStatus = bincode::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.code, StatusCode::Ok);
        assert_eq!(deserialized.message, "Success");
    }

    #[test]
    fn test_all_status_codes() {
        let codes = vec![
            StatusCode::Ok,
            StatusCode::InvalidArgument,
            StatusCode::NotFound,
            StatusCode::DeadlineExceeded,
            StatusCode::Unavailable,
            StatusCode::Internal,
        ];

        for code in codes {
            let status = RPCStatus {
                code,
                message: "Test".to_string(),
            };

            let serialized = bincode::serialize(&status).unwrap();
            let deserialized: RPCStatus = bincode::deserialize(&serialized).unwrap();
            assert_eq!(deserialized.code, code);
        }
    }

    #[test]
    fn test_large_payload_serialization() {
        // Test with a large payload (but under MAX_FRAME_SIZE)
        let large_payload: Vec<u8> = (0..10000).map(|x| (x % 256) as u8).collect();
        let envelope = create_test_envelope("Service", "Method", large_payload.clone());

        let frame = serialize_frame(&envelope);
        assert!(frame.len() < MAX_FRAME_SIZE);

        let length = u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
        assert_eq!(frame.len() - 4, length);
    }

    #[tokio::test]
    async fn test_handler_error_response() {
        // Create server with a handler that returns an error
        let mut server = RPCServer::new();
        server.register_service(
            "Faulty".to_string(),
            "error".to_string(),
            Box::new(|_payload: Vec<u8>| {
                Box::pin(async move {
                    Err(RPCStatus {
                        code: StatusCode::Internal,
                        message: "Handler error".to_string(),
                    })
                })
            }),
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handlers = Arc::new(server.handlers);

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let _ = handle_connection(stream, handlers).await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let mut client = TcpStream::connect(addr).await.unwrap();
        let request = create_test_envelope("Faulty", "error", vec![]);
        let request_frame = serialize_frame(&request);
        client.write_all(&request_frame).await.unwrap();

        // Read error response
        let mut length_buf = [0u8; 4];
        client.read_exact(&mut length_buf).await.unwrap();
        let length = u32::from_be_bytes(length_buf) as usize;

        let mut response_buf = vec![0u8; length];
        client.read_exact(&mut response_buf).await.unwrap();

        let response: RPCEnvelope = bincode::deserialize(&response_buf).unwrap();

        // Verify error response matches request sequence_number
        assert_eq!(response.sequence_number, 42); // Must match request
        assert_eq!(response.status.as_ref().unwrap().code, StatusCode::Internal);
        assert_eq!(response.status.as_ref().unwrap().message, "Handler error");
    }

    #[tokio::test]
    async fn test_graceful_client_disconnect() {
        // Create server with echo service
        let mut server = RPCServer::new();
        server.register_service(
            "Echo".to_string(),
            "echo".to_string(),
            Box::new(|payload: Vec<u8>| Box::pin(async move { Ok(payload) })),
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handlers = Arc::new(server.handlers);

        // Spawn server that should not panic or error when client disconnects
        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            // This should return Ok(()) when client disconnects gracefully
            handle_connection(stream, handlers).await
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Connect and send one request
        let mut client = TcpStream::connect(addr).await.unwrap();
        let request = create_test_envelope("Echo", "echo", vec![1, 2, 3]);
        let request_frame = serialize_frame(&request);
        client.write_all(&request_frame).await.unwrap();

        // Read response
        let mut length_buf = [0u8; 4];
        client.read_exact(&mut length_buf).await.unwrap();
        let length = u32::from_be_bytes(length_buf) as usize;
        let mut response_buf = vec![0u8; length];
        client.read_exact(&mut response_buf).await.unwrap();

        // Close the client connection gracefully
        drop(client);

        // Wait for server to handle the disconnect
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Server should have returned Ok(()) from handle_connection
        let result = server_handle.await.unwrap();
        assert!(
            result.is_ok(),
            "Server should handle graceful disconnect without error"
        );
    }
}
