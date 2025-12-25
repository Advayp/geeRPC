use serde::{Deserialize, Serialize};
use snafu::Snafu;

pub mod server;

const MAX_FRAME_SIZE: usize = 8 * 1024 * 1024; // 8MB

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Failed to bind to address: {}", source))]
    BindFailed { source: std::io::Error },

    #[snafu(display("Failed to accept connection: {}", source))]
    AcceptFailed { source: std::io::Error },

    #[snafu(display("Failed to read from stream: {}", source))]
    ReadFailed { source: std::io::Error },

    #[snafu(display("Failed to write to stream: {}", source))]
    WriteFailed { source: std::io::Error },

    #[snafu(display("Frame {length} bytes exceeds the maximum size of {MAX_FRAME_SIZE} bytes"))]
    FrameTooLarge { length: usize },

    #[snafu(display("Failed to deserialize frame: {}", source))]
    DeserializeFailed { source: serde_yaml::Error },

    #[snafu(display("Failed to serialize frame: {}", source))]
    SerializeFailed { source: serde_yaml::Error },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RPCStatus {
    pub code: StatusCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum StatusCode {
    Ok,
    InvalidArgument,
    NotFound,
    DeadlineExceeded,
    Unavailable,
    Internal,
}

type ServiceName = String;
type MethodName = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RPCEnvelope {
    pub version: u8,
    pub request_id: u64,
    pub service_name: ServiceName,
    pub method_name: MethodName,
    pub status: Option<RPCStatus>,
    pub payload: Vec<u8>,
}
