# geeRPC Benchmark Report

**Warmup**: 3 seconds per benchmark  
**Samples**: 100 iterations per measurement

---

## 1. End-to-End Latency

**Description**: Measures the time for a complete request-response round trip with connection reuse. This includes network I/O, serialization, deserialization, and handler execution.

| Payload Size       | Mean Latency | Min Latency | Max Latency |
| ------------------ | ------------ | ----------- | ----------- |
| Small (~100 bytes) | **43.6 µs**  | 43.6 µs     | 43.7 µs     |
| Medium (~10 KB)    | **67.8 µs**  | 67.6 µs     | 68.0 µs     |
| Large (~1 MB)      | **2.65 ms**  | 2.65 ms     | 2.66 ms     |

---

## 2. Serialization Performance

**Description**: Measures the isolated cost of encoding and decoding payloads using bincode, without network overhead. Encode converts Rust types to bytes, decode converts bytes back to Rust types, and roundtrip performs both operations sequentially.

### Small Payloads (~100 bytes)

| Operation | Description                    | Mean Time   | Throughput     |
| --------- | ------------------------------ | ----------- | -------------- |
| Encode    | Serialize Rust type to bytes   | **21.2 ns** | ~47M ops/sec   |
| Decode    | Deserialize bytes to Rust type | **68.2 ns** | ~14.7M ops/sec |
| Roundtrip | Encode then decode             | **68.8 ns** | ~14.5M ops/sec |

### Medium Payloads (~10 KB)

| Operation | Description                    | Mean Time   | Throughput    |
| --------- | ------------------------------ | ----------- | ------------- |
| Encode    | Serialize Rust type to bytes   | **1.54 µs** | ~650K ops/sec |
| Decode    | Deserialize bytes to Rust type | **3.16 µs** | ~316K ops/sec |
| Roundtrip | Encode then decode             | **4.78 µs** | ~209K ops/sec |

### Large Payloads (~1 MB)

| Operation | Description                    | Mean Time  | Throughput     |
| --------- | ------------------------------ | ---------- | -------------- |
| Encode    | Serialize Rust type to bytes   | **275 µs** | ~3,636 ops/sec |
| Decode    | Deserialize bytes to Rust type | **282 µs** | ~3,546 ops/sec |
| Roundtrip | Encode then decode             | **547 µs** | ~1,828 ops/sec |

---

## 3. Sequential Throughput

**Description**: Measures requests per second for sequential requests on a single connection. Requests are sent one at a time, waiting for each response before sending the next. This represents the maximum sequential throughput achievable with the framework.

### Small Payloads (~100 bytes)

| Number of Requests | Total Time to Complete | Throughput (req/s) |
| ------------------ | ---------------------- | ------------------ |
| 100                | 4.35 ms                | **~23,000 req/s**  |
| 1,000              | 43.33 ms               | **~23,080 req/s**  |
| 10,000             | 441.02 ms              | **~22,675 req/s**  |

**Peak Sequential Throughput: ~23,000 req/s**

### Medium Payloads (~10 KB)

| Number of Requests | Total Time to Complete | Throughput (req/s) |
| ------------------ | ---------------------- | ------------------ |
| 100                | 6.79 ms                | **~14,725 req/s**  |
| 1,000              | 68.02 ms               | **~14,701 req/s**  |
| 5,000              | 340.12 ms              | **~14,710 req/s**  |

**Peak Sequential Throughput: ~14,700 req/s**

### Large Payloads (~1 MB)

| Number of Requests | Total Time to Complete | Throughput (req/s) |
| ------------------ | ---------------------- | ------------------ |
| 10                 | 26.71 ms               | **~374 req/s**     |
| 50                 | 135.02 ms              | **~371 req/s**     |
| 100                | 263.49 ms              | **~380 req/s**     |

**Peak Sequential Throughput: ~380 req/s**

---

## 4. Dispatch Overhead

**Description**: Compares the time for a complete RPC call (including network I/O, serialization, and framework overhead) versus a direct function call (no network, no serialization). The overhead ratio shows how much slower the RPC call is compared to a direct call.

| Payload Size | RPC Call Time (mean) | Direct Call Time (mean) | Overhead Ratio |
| ------------ | -------------------- | ----------------------- | -------------- |
| Small        | **43.7 µs**          | 21.4 ns                 | ~2,043x        |
| Medium       | **67.5 µs**          | 72.5 ns                 | ~931x          |
| Large        | **2.65 ms**          | 13.5 µs                 | ~196x          |

---

## 5. Concurrency Scaling

**Description**: Measures latency and throughput under varying concurrent request loads. All requests use a single client connection with multiple in-flight RPCs. The server processes requests concurrently by spawning a task for each request. Mean latency is the average response time across all concurrent requests. p50, p95, and p99 are percentile latencies (50th, 95th, and 99th percentiles). Throughput is calculated as the number of successful requests divided by total time.

### Small Payloads (~100 bytes)

| Concurrent Requests | Mean Latency | p50 (median) | p95    | p99    | Throughput (req/s) |
| ------------------- | ------------ | ------------ | ------ | ------ | ------------------ |
| 1                   | **481 µs**   | 456 µs       | 506 µs | 506 µs | ~2,080 req/s       |
| 10                  | **480 µs**   | 458 µs       | 503 µs | 503 µs | ~20,833 req/s      |
| 50                  | **497 µs**   | 473 µs       | 521 µs | 521 µs | ~100,604 req/s     |
| 100                 | **500 µs**   | 475 µs       | 527 µs | 527 µs | ~200,000 req/s     |

### Medium Payloads (~10 KB)

| Concurrent Requests | Mean Latency | p50 (median) | p95     | p99     | Throughput (req/s) |
| ------------------- | ------------ | ------------ | ------- | ------- | ------------------ |
| 1                   | **2.85 ms**  | 2.68 ms      | 3.02 ms | 3.02 ms | ~351 req/s         |
| 10                  | **983 µs**   | 711 µs       | 1.40 ms | 1.40 ms | ~10,173 req/s      |
| 100                 | **582 µs**   | 545 µs       | 623 µs  | 623 µs  | ~171,821 req/s     |

### Large Payloads (~1 MB)

| Concurrent Requests | Mean Latency | p50 (median) | p95     | p99     | Throughput (req/s) |
| ------------------- | ------------ | ------------ | ------- | ------- | ------------------ |
| 1                   | **3.87 ms**  | 3.77 ms      | 3.98 ms | 3.98 ms | ~258 req/s         |
| 10                  | **9.72 ms**  | 9.57 ms      | 9.95 ms | 9.95 ms | ~1,029 req/s       |
| 50                  | **30.4 ms**  | 30.2 ms      | 30.7 ms | 30.7 ms | ~1,645 req/s       |

---

## 6. Optimization Impact

**Description**: Comparison of performance before and after optimizations. "Before" measurements used new TCP connections per request. "After" measurements use connection reuse and concurrent server-side processing.

### Latency Improvements (Connection Reuse)

| Payload Size | Before (new connections per request) | After (connection reuse) | Improvement       |
| ------------ | ------------------------------------ | ------------------------ | ----------------- |
| Small        | ~102.7 ms                            | **43.6 µs**              | **99.96% faster** |
| Medium       | ~103.1 ms                            | **67.8 µs**              | **99.93% faster** |
| Large        | ~109.8 ms                            | **2.65 ms**              | **97.6% faster**  |

### Concurrency Improvements (Server-Side Concurrent Processing)

| Payload Size            | Before (sequential server processing) | After (concurrent server processing) | Improvement       |
| ----------------------- | ------------------------------------- | ------------------------------------ | ----------------- |
| Small (100 concurrent)  | ~403 ms                               | **500 µs**                           | **99.88% faster** |
| Medium (100 concurrent) | ~175 ms                               | **582 µs**                           | **99.67% faster** |

---

## 7. Test Methodology

### Benchmark Configuration

- **Runtime**: Tokio async runtime
- **Connection Strategy**: Connection reuse (single connection per benchmark)
- **Server Processing**: Concurrent (spawns task per request)
- **Warmup**: 3 seconds per benchmark
- **Samples**: 100 iterations per measurement
- **Payload Sizes**:
  - Small: ~100 bytes (String)
  - Medium: ~10 KB (Vec<u8>)
  - Large: ~1 MB (Vec<Vec<u8>> + metadata)

### Benchmark Categories

1. **End-to-End Latency**: Single request-response round trip with connection reuse
2. **Serialization**: Isolated encode/decode operations without network overhead
3. **Sequential Throughput**: Sequential requests on single connection
4. **Dispatch Overhead**: RPC call vs direct function call comparison
5. **Concurrency Scaling**: Multiple concurrent requests on single connection

---

## 8. Raw Benchmark Data

### Serialization Benchmarks

- **encode_small**: 21.2 ns (mean) - Serialize small payload to bytes
- **decode_small**: 68.2 ns (mean) - Deserialize small payload from bytes
- **encode_medium**: 1.54 µs (mean) - Serialize medium payload to bytes
- **decode_medium**: 3.16 µs (mean) - Deserialize medium payload from bytes
- **encode_large**: 275 µs (mean) - Serialize large payload to bytes
- **decode_large**: 282 µs (mean) - Deserialize large payload from bytes
- **roundtrip_small**: 68.8 ns (mean) - Encode then decode small payload
- **roundtrip_medium**: 4.78 µs (mean) - Encode then decode medium payload
- **roundtrip_large**: 547 µs (mean) - Encode then decode large payload

### Latency Benchmarks

- **latency_small**: 43.6 µs (mean) - End-to-end latency for small payload
- **latency_medium**: 67.8 µs (mean) - End-to-end latency for medium payload
- **latency_large**: 2.65 ms (mean) - End-to-end latency for large payload

### Throughput Benchmarks (Sequential)

- **throughput_small/100**: 4.35 ms total for 100 requests → **23,000 req/s**
- **throughput_small/1000**: 43.33 ms total for 1,000 requests → **23,080 req/s**
- **throughput_small/10000**: 441.02 ms total for 10,000 requests → **22,675 req/s**
- **throughput_medium/100**: 6.79 ms total for 100 requests → **14,725 req/s**
- **throughput_medium/1000**: 68.02 ms total for 1,000 requests → **14,701 req/s**
- **throughput_medium/5000**: 340.12 ms total for 5,000 requests → **14,710 req/s**
- **throughput_large/10**: 26.71 ms total for 10 requests → **374 req/s**
- **throughput_large/50**: 135.02 ms total for 50 requests → **371 req/s**
- **throughput_large/100**: 263.49 ms total for 100 requests → **380 req/s**

### Dispatch Overhead Benchmarks

- **dispatch_overhead_small_rpc**: 43.7 µs (mean) - Complete RPC call for small payload
- **dispatch_overhead_small_direct**: 21.4 ns (mean) - Direct function call for small payload
- **dispatch_overhead_medium_rpc**: 67.5 µs (mean) - Complete RPC call for medium payload
- **dispatch_overhead_medium_direct**: 72.5 ns (mean) - Direct function call for medium payload
- **dispatch_overhead_large_rpc**: 2.65 ms (mean) - Complete RPC call for large payload
- **dispatch_overhead_large_direct**: 13.5 µs (mean) - Direct function call for large payload

### Concurrency Benchmarks (Concurrent Server Processing)

- **concurrency_small/1**: 481 µs (mean), 456 µs (p50) - 1 concurrent small payload request
- **concurrency_small/10**: 480 µs (mean), 458 µs (p50) - 10 concurrent small payload requests
- **concurrency_small/50**: 497 µs (mean), 473 µs (p50) - 50 concurrent small payload requests
- **concurrency_small/100**: 500 µs (mean), 475 µs (p50) - 100 concurrent small payload requests
- **concurrency_medium/1**: 2.85 ms (mean), 2.68 ms (p50) - 1 concurrent medium payload request
- **concurrency_medium/10**: 983 µs (mean), 711 µs (p50) - 10 concurrent medium payload requests
- **concurrency_medium/100**: 582 µs (mean), 545 µs (p50) - 100 concurrent medium payload requests
- **concurrency_large/1**: 3.87 ms (mean), 3.77 ms (p50) - 1 concurrent large payload request
- **concurrency_large/10**: 9.72 ms (mean), 9.57 ms (p50) - 10 concurrent large payload requests
- **concurrency_large/50**: 30.4 ms (mean), 30.2 ms (p50) - 50 concurrent large payload requests

---

**Report Generated**: Latest benchmark run  
**Benchmark Suite**: geerpc-benchmarks  
**Framework**: geeRPC  
**Key Features**: Connection reuse, concurrent server-side request processing
