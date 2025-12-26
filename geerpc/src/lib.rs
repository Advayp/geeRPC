use serde::{Deserialize, Serialize};
use snafu::Snafu;
use snafu::prelude::*;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

pub mod client;
pub mod server;

// Re-export internal functions for testing purposes only
// This is not part of the public API and should not be used by library consumers
#[doc(hidden)]
#[cfg(any(test, feature = "internal-testing"))]
pub use server::handle_connection;

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
    DeserializeFailed { source: bincode::Error },

    #[snafu(display("Failed to serialize frame: {}", source))]
    SerializeFailed { source: bincode::Error },

    #[snafu(display("Failed to connect to address: {}", source))]
    ConnectFailed { source: std::io::Error },

    #[snafu(display("Address is required"))]
    AddressRequired,

    #[snafu(display("Failed to receive response from channel: {}", source))]
    RecvFailed {
        source: tokio::sync::oneshot::error::RecvError,
    },

    #[snafu(display("Failed to send response to channel"))]
    SendFailed,
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
pub(crate) struct RPCEnvelope {
    pub(crate) version: u8,
    pub(crate) sequence_number: u64,
    pub(crate) service_name: ServiceName,
    pub(crate) method_name: MethodName,
    pub(crate) status: Option<RPCStatus>,
    pub(crate) payload: Vec<u8>,
}

pub(crate) async fn write_frame<W>(stream: &mut W, envelope: RPCEnvelope) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    let frame = bincode::serialize(&envelope).context(SerializeFailedSnafu)?;
    let length = frame.len();
    let mut buffer = vec![0; 4];
    buffer[0..4].copy_from_slice(&(length as u32).to_be_bytes());
    stream.write_all(&buffer).await.context(WriteFailedSnafu)?;
    stream.write_all(&frame).await.context(WriteFailedSnafu)?;
    Ok(())
}

pub(crate) async fn read_raw_frame<R>(stream: &mut R) -> Result<Vec<u8>>
where
    R: AsyncReadExt + Unpin,
{
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
