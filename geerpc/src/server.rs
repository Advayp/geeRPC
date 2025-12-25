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
            tokio::select! {
                result = listener.accept() => {
                    let (stream, _) = result.context(AcceptFailedSnafu)?;
                    tracing::info!("Accepted connection from {}", stream.peer_addr().unwrap());
                    let handlers = handlers.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, handlers).await {
                            tracing::error!("Error handling connection: {}", e);
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

async fn write_error_frame(
    stream: &mut TcpStream,
    error: RPCStatus,
    sequence_number: u64,
) -> Result<()> {
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
                    return write_error_frame(&mut stream, e, envelope.sequence_number).await;
                }
            };

            write_frame(
                &mut stream,
                RPCEnvelope {
                    version: 1,
                    sequence_number: envelope.sequence_number.clone(),
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
                envelope.sequence_number,
            )
            .await?;
        }
    }
}

#[cfg(test)]
mod tests {
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
        let yaml = serde_yaml::to_string(envelope).unwrap();
        let payload_bytes = yaml.as_bytes();
        let length = payload_bytes.len() as u32;

        let mut frame = Vec::new();
        frame.extend_from_slice(&length.to_be_bytes());
        frame.extend_from_slice(payload_bytes);
        frame
    }

    // Helper function to deserialize a frame
    async fn deserialize_frame(data: &[u8]) -> RPCEnvelope {
        assert!(data.len() >= 4, "Frame too short");
        let length = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        assert_eq!(data.len() - 4, length, "Length mismatch");

        let yaml_data = &data[4..];
        serde_yaml::from_slice(yaml_data).unwrap()
    }

    #[test]
    fn test_envelope_serialization() {
        let envelope = create_test_envelope("TestService", "TestMethod", vec![1, 2, 3, 4]);

        // Test YAML serialization
        let yaml = serde_yaml::to_string(&envelope).unwrap();
        assert!(yaml.contains("TestService"));
        assert!(yaml.contains("TestMethod"));

        // Test deserialization
        let deserialized: RPCEnvelope = serde_yaml::from_str(&yaml).unwrap();
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

        let response: RPCEnvelope = serde_yaml::from_slice(&response_buf).unwrap();

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

        let response: RPCEnvelope = serde_yaml::from_slice(&response_buf).unwrap();

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

            let response: RPCEnvelope = serde_yaml::from_slice(&response_buf).unwrap();

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

        let yaml = serde_yaml::to_string(&status).unwrap();
        let deserialized: RPCStatus = serde_yaml::from_str(&yaml).unwrap();

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

            let yaml = serde_yaml::to_string(&status).unwrap();
            let deserialized: RPCStatus = serde_yaml::from_str(&yaml).unwrap();
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

        let response: RPCEnvelope = serde_yaml::from_slice(&response_buf).unwrap();

        // Verify error response matches request sequence_number
        assert_eq!(response.sequence_number, 42); // Must match request
        assert_eq!(response.status.as_ref().unwrap().code, StatusCode::Internal);
        assert_eq!(response.status.as_ref().unwrap().message, "Handler error");
    }
}
