use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

#[path = "common.rs"]
mod common;
use common::*;

// Generate code for small payload service
geerpc::rpc_gen!("bench_small.yaml", module = "bench_small");

// Generate code for medium payload service
geerpc::rpc_gen!("bench_medium.yaml", module = "bench_medium");

// Generate code for large payload service
geerpc::rpc_gen!("bench_large.yaml", module = "bench_large");

// Echo service implementation
struct EchoSmallService;

#[async_trait::async_trait]
impl bench_small::BenchSmallService for EchoSmallService {
    async fn echo(
        &self,
        request: bench_small::EchoRequest,
    ) -> Result<bench_small::EchoResponse, geerpc::Error> {
        Ok(bench_small::EchoResponse {
            message: request.message,
        })
    }
}

async fn bench_concurrency_small_inner(
    num_concurrent: usize,
) -> Result<(f64, Duration, Duration, Duration), Box<dyn std::error::Error + Send + Sync>> {
    let service = EchoSmallService;
    let mut server = geerpc::server::RPCServer::new();
    bench_small::BenchSmallServer::new(service).register(&mut server);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?.to_string();
    let handlers = std::sync::Arc::new(server.handlers);
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let server_handle_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            let handlers = handlers.clone();
                            tokio::spawn(async move {
                                let _ = geerpc::server::handle_connection(stream, handlers).await;
                            });
                        }
                        Err(e) => {
                            eprintln!("Accept error: {}", e);
                            break;
                        }
                    }
                }
                _ = &mut shutdown_rx => {
                    break;
                }
            }
        }
    });

    let server_handle = common::ServerHandle {
        shutdown_tx: Some(shutdown_tx),
        handle: server_handle_task,
        address: addr.clone(),
    };

    // Create a single client that will handle all concurrent requests
    let client = bench_small::BenchSmallClient::try_new(addr).await?;
    let client = std::sync::Arc::new(client);

    let request = bench_small::EchoRequest {
        message: generate_small_payload(),
    };

    let start = Instant::now();
    let mut handles = Vec::new();

    // Make multiple concurrent requests using the same client
    for _ in 0..num_concurrent {
        let client = client.clone();
        let request = request.clone();
        let handle = tokio::spawn(async move {
            let req_start = Instant::now();
            match client.echo(black_box(request)).await {
                Ok(_response) => req_start.elapsed(),
                Err(e) => {
                    eprintln!("RPC call failed: {}", e);
                    Duration::ZERO
                }
            }
        });
        handles.push(handle);
    }

    let mut latencies = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(latency) => {
                if latency > Duration::ZERO {
                    latencies.push(latency);
                }
            }
            Err(e) => {
                eprintln!("Task join error: {}", e);
            }
        }
    }

    let total_time = start.elapsed();

    // Only calculate throughput and percentiles if we have successful requests
    if latencies.is_empty() {
        return Err(format!("No successful requests out of {} attempted", num_concurrent).into());
    }

    // Calculate throughput based on successful requests
    let successful_count = latencies.len();
    let throughput = successful_count as f64 / total_time.as_secs_f64();

    latencies.sort();
    let p50 = calculate_percentiles(&latencies, 50.0);
    let p95 = calculate_percentiles(&latencies, 95.0);
    let p99 = calculate_percentiles(&latencies, 99.0);

    server_handle.shutdown().await;

    Ok((throughput, p50, p95, p99))
}

fn bench_concurrency_small(c: &mut Criterion) {
    // Create runtime with more threads for high concurrency
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrency_small");

    // Reduced max concurrency to avoid overwhelming the system
    for num_concurrent in [1, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_concurrent),
            num_concurrent,
            |b, &num_concurrent| {
                b.to_async(&rt).iter(|| async {
                    match bench_concurrency_small_inner(num_concurrent).await {
                        Ok((throughput, p50, p95, p99)) => {
                            criterion::black_box((throughput, p50, p95, p99));
                        }
                        Err(e) => {
                            eprintln!("Benchmark failed: {}", e);
                            // Return zero values to avoid panicking
                            criterion::black_box((
                                0.0,
                                Duration::ZERO,
                                Duration::ZERO,
                                Duration::ZERO,
                            ));
                        }
                    }
                });
            },
        );
    }
    group.finish();
}

// Echo service implementation for medium
struct EchoMediumService;

#[async_trait::async_trait]
impl bench_medium::BenchMediumService for EchoMediumService {
    async fn echo(
        &self,
        request: bench_medium::EchoRequest,
    ) -> Result<bench_medium::EchoResponse, geerpc::Error> {
        Ok(bench_medium::EchoResponse { data: request.data })
    }
}

async fn bench_concurrency_medium_inner(
    num_concurrent: usize,
) -> Result<(f64, Duration, Duration, Duration), Box<dyn std::error::Error + Send + Sync>> {
    let service = EchoMediumService;
    let mut server = geerpc::server::RPCServer::new();
    bench_medium::BenchMediumServer::new(service).register(&mut server);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?.to_string();
    let handlers = std::sync::Arc::new(server.handlers);
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let server_handle_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            let handlers = handlers.clone();
                            tokio::spawn(async move {
                                let _ = geerpc::server::handle_connection(stream, handlers).await;
                            });
                        }
                        Err(e) => {
                            eprintln!("Accept error: {}", e);
                            break;
                        }
                    }
                }
                _ = &mut shutdown_rx => {
                    break;
                }
            }
        }
    });

    let server_handle = common::ServerHandle {
        shutdown_tx: Some(shutdown_tx),
        handle: server_handle_task,
        address: addr.clone(),
    };

    // Create a single client that will handle all concurrent requests
    let client = bench_medium::BenchMediumClient::try_new(addr).await?;
    let client = std::sync::Arc::new(client);

    let request = bench_medium::EchoRequest {
        data: generate_medium_payload(),
    };

    let start = Instant::now();
    let mut handles = Vec::new();

    // Make multiple concurrent requests using the same client
    for _ in 0..num_concurrent {
        let client = client.clone();
        let request = request.clone();
        let handle = tokio::spawn(async move {
            let req_start = Instant::now();
            match client.echo(black_box(request)).await {
                Ok(_response) => req_start.elapsed(),
                Err(e) => {
                    eprintln!("RPC call failed: {}", e);
                    Duration::ZERO
                }
            }
        });
        handles.push(handle);
    }

    let mut latencies = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(latency) => {
                if latency > Duration::ZERO {
                    latencies.push(latency);
                }
            }
            Err(e) => {
                eprintln!("Task join error: {}", e);
            }
        }
    }

    let total_time = start.elapsed();

    // Only calculate throughput and percentiles if we have successful requests
    if latencies.is_empty() {
        return Err(format!("No successful requests out of {} attempted", num_concurrent).into());
    }

    // Calculate throughput based on successful requests
    let successful_count = latencies.len();
    let throughput = successful_count as f64 / total_time.as_secs_f64();

    latencies.sort();
    let p50 = calculate_percentiles(&latencies, 50.0);
    let p95 = calculate_percentiles(&latencies, 95.0);
    let p99 = calculate_percentiles(&latencies, 99.0);

    server_handle.shutdown().await;

    Ok((throughput, p50, p95, p99))
}

fn bench_concurrency_medium(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrency_medium");

    for num_concurrent in [1, 10, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_concurrent),
            num_concurrent,
            |b, &num_concurrent| {
                b.to_async(&rt).iter(|| async {
                    match bench_concurrency_medium_inner(num_concurrent).await {
                        Ok((throughput, p50, p95, p99)) => {
                            criterion::black_box((throughput, p50, p95, p99));
                        }
                        Err(e) => {
                            eprintln!("Benchmark failed: {}", e);
                            criterion::black_box((
                                0.0,
                                Duration::ZERO,
                                Duration::ZERO,
                                Duration::ZERO,
                            ));
                        }
                    }
                });
            },
        );
    }
    group.finish();
}

// Echo service implementation for large
struct EchoLargeService;

#[async_trait::async_trait]
impl bench_large::BenchLargeService for EchoLargeService {
    async fn echo(
        &self,
        request: bench_large::EchoRequest,
    ) -> Result<bench_large::EchoResponse, geerpc::Error> {
        Ok(bench_large::EchoResponse {
            items: request.items,
            metadata: request.metadata,
        })
    }
}

async fn bench_concurrency_large_inner(
    num_concurrent: usize,
) -> Result<(f64, Duration, Duration, Duration), Box<dyn std::error::Error + Send + Sync>> {
    let service = EchoLargeService;
    let mut server = geerpc::server::RPCServer::new();
    bench_large::BenchLargeServer::new(service).register(&mut server);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?.to_string();
    let handlers = std::sync::Arc::new(server.handlers);
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let server_handle_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            let handlers = handlers.clone();
                            tokio::spawn(async move {
                                let _ = geerpc::server::handle_connection(stream, handlers).await;
                            });
                        }
                        Err(e) => {
                            eprintln!("Accept error: {}", e);
                            break;
                        }
                    }
                }
                _ = &mut shutdown_rx => {
                    break;
                }
            }
        }
    });

    let server_handle = common::ServerHandle {
        shutdown_tx: Some(shutdown_tx),
        handle: server_handle_task,
        address: addr.clone(),
    };

    // Create a single client that will handle all concurrent requests
    let client = bench_large::BenchLargeClient::try_new(addr).await?;
    let client = std::sync::Arc::new(client);

    let (items, metadata) = generate_large_payload();
    let request = bench_large::EchoRequest {
        items: items.clone(),
        metadata: metadata.clone(),
    };

    let start = Instant::now();
    let mut handles = Vec::new();

    // Make multiple concurrent requests using the same client
    for _ in 0..num_concurrent {
        let client = client.clone();
        let request = request.clone();
        let handle = tokio::spawn(async move {
            let req_start = Instant::now();
            match client.echo(black_box(request)).await {
                Ok(_response) => req_start.elapsed(),
                Err(e) => {
                    eprintln!("RPC call failed: {}", e);
                    Duration::ZERO
                }
            }
        });
        handles.push(handle);
    }

    let mut latencies = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(latency) => {
                if latency > Duration::ZERO {
                    latencies.push(latency);
                }
            }
            Err(e) => {
                eprintln!("Task join error: {}", e);
            }
        }
    }

    let total_time = start.elapsed();

    // Only calculate throughput and percentiles if we have successful requests
    if latencies.is_empty() {
        return Err(format!("No successful requests out of {} attempted", num_concurrent).into());
    }

    // Calculate throughput based on successful requests
    let successful_count = latencies.len();
    let throughput = successful_count as f64 / total_time.as_secs_f64();

    latencies.sort();
    let p50 = calculate_percentiles(&latencies, 50.0);
    let p95 = calculate_percentiles(&latencies, 95.0);
    let p99 = calculate_percentiles(&latencies, 99.0);

    server_handle.shutdown().await;

    Ok((throughput, p50, p95, p99))
}

fn bench_concurrency_large(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrency_large");

    for num_concurrent in [1, 10, 50].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_concurrent),
            num_concurrent,
            |b, &num_concurrent| {
                b.to_async(&rt).iter(|| async {
                    match bench_concurrency_large_inner(num_concurrent).await {
                        Ok((throughput, p50, p95, p99)) => {
                            criterion::black_box((throughput, p50, p95, p99));
                        }
                        Err(e) => {
                            eprintln!("Benchmark failed: {}", e);
                            criterion::black_box((
                                0.0,
                                Duration::ZERO,
                                Duration::ZERO,
                                Duration::ZERO,
                            ));
                        }
                    }
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_concurrency_small,
    bench_concurrency_medium,
    bench_concurrency_large
);
criterion_main!(benches);
