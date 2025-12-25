use snafu::Snafu;

pub mod server;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Failed to bind to address: {}", source))]
    BindFailed { source: std::io::Error },

    #[snafu(display("Failed to accept connection: {}", source))]
    AcceptFailed { source: std::io::Error },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone)]
pub struct RpcStatus {
    pub code: StatusCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy)]
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
