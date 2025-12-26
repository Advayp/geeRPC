# geerpc-codegen

Code generation for geeRPC client and server stubs from YAML service definitions.

## Overview

This crate provides a procedural macro `gen!` that generates type-safe RPC client and server code from YAML service definitions at compile time.

## Usage

### 1. Define Your Service in YAML

Create a YAML file describing your service:

```yaml
# ping.yaml
service: Ping
methods:
  Ping:
    request:
      message: String
    response:
      message: String
```

### 2. Generate Code

Use the `gen!` macro in your Rust code:

```rust
use geerpc::gen;

// Simplest form - generates both client and server
rpc_gen!("ping.yaml");

// The macro creates a module named 'ping' (snake_case of service name)
// You can now use:
// - ping::PingClient
// - ping::PingService
// - ping::PingServer
// - ping::PingRequest
// - ping::PingResponse
```

### 3. Implement and Use

```rust
// Implement the service
struct MyPingService;

#[async_trait::async_trait]
impl ping::PingService for MyPingService {
    async fn ping(&self, request: ping::PingRequest) 
        -> Result<ping::PingResponse, geerpc::Error> 
    {
        Ok(ping::PingResponse {
            message: format!("Echo: {}", request.message),
        })
    }
}

// Use the server
let mut server = geerpc::server::RPCServer::new();
let service = MyPingService;
let ping_server = ping::PingServer::new(service);
ping_server.register(&mut server);
server.serve("127.0.0.1:50051").await?;

// Use the client
let client = ping::PingClient::try_new("127.0.0.1:50051".to_string()).await?;
let response = client.ping(ping::PingRequest {
    message: "Hello".to_string(),
}).await?;
```

## Macro Options

The `gen!` macro supports several options:

### Default Behavior

```rust
rpc_gen!("service.yaml");
```

- Generates both client and server code
- Module name is the snake_case version of the service name

### Custom Module Name

```rust
rpc_gen!("service.yaml", module = "my_custom_name");
```

### Client Only

```rust
rpc_gen!("service.yaml", client = true, server = false);
```

### Server Only

```rust
rpc_gen!("service.yaml", client = false, server = true);
```

### All Options Combined

```rust
rpc_gen!("service.yaml", client = true, server = false, module = "my_client");
```

## YAML Schema

### Service Definition

```yaml
service: ServiceName  # Used to generate struct names (e.g., ServiceNameClient)
methods:
  MethodName:         # Method name (converted to snake_case)
    request:
      field1: Type1   # Field name and Rust type
      field2: Type2
    response:
      result: Type3
      error: Option<String>
```

### Supported Types

- Primitives: `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`, `f32`, `f64`, `bool`, `usize`, `isize`
- Standard library: `String`, `Vec<T>`, `Option<T>`
- Any combination of the above

### Example with Multiple Methods and Fields

```yaml
service: User
methods:
  GetUser:
    request:
      id: u64
    response:
      name: String
      email: String
      age: u32
  
  CreateUser:
    request:
      name: String
      email: String
      age: u32
    response:
      id: u64
      success: bool
  
  DeleteUser:
    request:
      id: u64
    response:
      success: bool
```

## Generated Code

For each service definition, the macro generates:

### Request/Response Structs

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MethodNameRequest {
    pub field1: Type1,
    pub field2: Type2,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MethodNameResponse {
    pub result: Type3,
}
```

### Client (if enabled)

```rust
pub struct ServiceNameClient {
    inner: Arc<RPCClient>,
}

impl ServiceNameClient {
    pub async fn try_new(address: String) -> Result<Self, geerpc::Error> { ... }
    
    pub async fn method_name(&self, request: MethodNameRequest) 
        -> Result<MethodNameResponse, geerpc::Error> { ... }
}
```

### Server (if enabled)

```rust
#[async_trait]
pub trait ServiceNameService: Send + Sync + 'static {
    async fn method_name(&self, request: MethodNameRequest) 
        -> Result<MethodNameResponse, geerpc::Error>;
}

pub struct ServiceNameServer<T: ServiceNameService> {
    inner: Arc<T>,
}

impl<T: ServiceNameService> ServiceNameServer<T> {
    pub fn new(service: T) -> Self { ... }
    pub fn register(&self, server: &mut RPCServer) { ... }
}
```

## Features

- ✅ Compile-time code generation
- ✅ Type-safe request/response handling
- ✅ Automatic snake_case conversion for Rust idioms
- ✅ Support for multiple methods per service
- ✅ Support for multiple fields per request/response
- ✅ Support for generic types (Vec, Option)
- ✅ Optional client/server generation
- ✅ Custom module names

## Examples

See the `examples/` directory for complete working examples.

## Testing

Run the tests with:

```bash
cargo test --package geerpc-codegen
```

This includes:
- Unit tests for utility functions
- Integration tests for code generation
- End-to-end tests with actual client/server communication

