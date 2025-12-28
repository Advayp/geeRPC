# geeRPC

A minimal, educational RPC framework inspired by gRPC, built in Rust.

## Overview

geeRPC is a lightweight RPC (Remote Procedure Call) framework designed for learning and understanding how RPC systems work. It provides a simple yet complete implementation of an RPC framework with code generation capabilities, making it easy to define services and generate type-safe client and server code.

## Features

- 🚀 **Code Generation**: Generate type-safe client and server stubs from YAML service definitions at compile time
- 🔒 **Type Safety**: Full type safety with Rust's type system for requests and responses
- ⚡ **Async/Await**: Built on Tokio for high-performance asynchronous I/O
- 📦 **Binary Protocol**: Efficient binary serialization using bincode
- 🔄 **Request/Response Matching**: Automatic sequence number tracking for concurrent requests
- 🛡️ **Error Handling**: Comprehensive error handling with status codes (Ok, InvalidArgument, NotFound, etc.)
- 🎯 **Simple API**: Clean, intuitive API for both client and server usage
- 📝 **YAML-Based**: Define services using simple YAML files

## Architecture

geeRPC consists of two main components:

1. **geerpc**: The core RPC library providing:

   - `RPCServer`: Multi-connection server with handler registration
   - `RPCClient`: Client with automatic request/response matching
   - Frame-based protocol with length-prefixed messages
   - Serialization/deserialization utilities

2. **geerpc-codegen**: Procedural macro for code generation:
   - `rpc_gen!` macro that reads YAML service definitions
   - Generates client structs, server traits, and request/response types
   - Supports custom module names and selective generation

## Quick Start

### 1. Define Your Service

Create a YAML file (e.g., `ping.yaml`):

```yaml
service: Ping
methods:
  Ping:
    request:
      message: String
    response:
      message: String
```

### 2. Generate Code

In your Rust code, use the `rpc_gen!` macro:

```rust
use geerpc::rpc_gen;

rpc_gen!("ping.yaml");
```

This generates a `ping` module with:

- `PingClient`: Type-safe client
- `PingService`: Trait to implement
- `PingServer`: Server wrapper
- `PingRequest` and `PingResponse`: Request/response types

### 3. Implement the Server

```rust
use async_trait::async_trait;
use geerpc::server::RPCServer;

struct MyPingService;

#[async_trait]
impl ping::PingService for MyPingService {
    async fn ping(&self, request: ping::PingRequest)
        -> Result<ping::PingResponse, geerpc::Error>
    {
        Ok(ping::PingResponse {
            message: format!("Echo: {}", request.message),
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut server = RPCServer::new();
    ping::PingServer::new(MyPingService).register(&mut server);
    server.serve("127.0.0.1:8081").await?;
    Ok(())
}
```

### 4. Use the Client

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ping::PingClient::try_new("127.0.0.1:8081".to_string()).await?;
    let response = client.ping(ping::PingRequest {
        message: "Hello, world!".to_string(),
    }).await?;
    println!("Response: {}", response.message);
    Ok(())
}
```

## Project Structure

```
geeRPC/
├── geerpc/              # Core RPC library
│   ├── src/
│   │   ├── lib.rs       # Main library entry point
│   │   ├── client.rs    # RPC client implementation
│   │   └── server.rs    # RPC server implementation
│   └── tests/           # Integration tests
├── geerpc-codegen/      # Code generation macro
│   ├── src/
│   │   └── lib.rs       # Procedural macro implementation
│   └── tests/           # Code generation tests
├── sample-server/       # Example server implementation
├── sample-client/       # Example client implementation
└── ping.yaml            # Example service definition
```

## Protocol Details

geeRPC uses a frame-based protocol:

1. **Frame Format**: Each frame consists of:

   - 4-byte length prefix (big-endian u32)
   - Serialized `RPCEnvelope` (using bincode)

2. **RPC Envelope**:

   - Version number
   - Sequence number (for request/response matching)
   - Service name and method name
   - Optional status (for errors)
   - Payload (serialized request/response)

3. **Maximum Frame Size**: 8MB

## Supported Types

The code generator supports:

- **Primitives**: `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`, `f32`, `f64`, `bool`, `usize`, `isize`
- **Standard Types**: `String`, `Vec<T>`, `Option<T>`
- **Nested Types**: Any combination of the above

## Examples

See the `sample-server/` and `sample-client/` directories for complete working examples.

For more detailed documentation on code generation, see [geerpc-codegen/README.md](geerpc-codegen/README.md).

## Building and Testing

Build the entire workspace:

```bash
cargo build
```

Run all tests:

```bash
cargo test
```

Run code generation tests:

```bash
cargo test --package geerpc-codegen
```

Run integration tests:

```bash
cargo test --package geerpc
```

## Performance

geeRPC has been extensively benchmarked to evaluate its performance characteristics. Below are key highlights from our comprehensive benchmark suite.

### Key Performance Metrics

**Latency (End-to-End Round Trip):**

- Small payloads (~100 bytes): **43.6 µs**
- Medium payloads (~10 KB): **67.8 µs**
- Large payloads (~1 MB): **2.65 ms**

**Throughput (Sequential):**

- Small payloads: **~23,000 req/s**
- Medium payloads: **~14,700 req/s**
- Large payloads: **~380 req/s**

**Concurrent Throughput (100 in-flight requests):**

- Small payloads: **~200,000 req/s**
- Medium payloads: **~172,000 req/s**
- Large payloads: **~1,645 req/s**

**Concurrency Scaling:**

- Small payloads maintain stable ~480-500 µs latency even with 100 concurrent requests
- Medium payloads show improved latency with concurrency (2.85ms → 582µs)
- Server-side concurrent processing enables efficient parallel request handling

### Comparison with gRPC

| Metric                                    | geeRPC             | gRPC (C++/Go)    | gRPC (Rust/tonic) |
| ----------------------------------------- | ------------------ | ---------------- | ----------------- |
| **Small Payload Latency**                 | **43.6 µs**        | ~50-200 µs       | ~100-500 µs       |
| **Large Payload Latency**                 | **2.65 ms**        | ~5-10 ms         | ~5-10 ms          |
| **Small Payload Throughput (Sequential)** | **~23,000 req/s**  | ~100k-500k req/s | ~50k-200k req/s   |
| **Small Payload Throughput (Concurrent)** | **~200,000 req/s** | ~500k-1M req/s   | ~200k-500k req/s  |
| **Large Payload Throughput**              | **~380 req/s**     | ~1k-10k req/s    | ~1k-10k req/s     |

For detailed benchmark results and analysis, see [BENCHMARK_REPORT.md](BENCHMARK_REPORT.md).

## Dependencies

- **tokio**: Async runtime
- **serde**: Serialization framework
- **bincode**: Binary serialization
- **snafu**: Error handling
- **async-trait**: Async trait support
- **serde_yaml**: YAML parsing (codegen only)
