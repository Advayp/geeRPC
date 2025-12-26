// Generated using the geerpc code generation macro
// This creates a 'ping' module with:
// - ping::PingRequest and ping::PingResponse structs
// - ping::PingClient for making RPC calls
// - ping::PingService trait and ping::PingServer for server implementation
geerpc::rpc_gen!("../ping.yaml", client = true);

// Re-export the generated types for convenience
pub use ping::*;
