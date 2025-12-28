use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Instant;
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

// Echo service implementations
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

async fn setup_throughput_small() -> Result<
    (common::ServerHandle, bench_small::BenchSmallClient),
    Box<dyn std::error::Error + Send + Sync>,
> {
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

    let client = bench_small::BenchSmallClient::try_new(addr).await?;

    Ok((server_handle, client))
}

fn bench_throughput_small(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (server_handle, client) = rt.block_on(setup_throughput_small()).unwrap();
    let client = std::sync::Arc::new(client);

    let mut group = c.benchmark_group("throughput_small");

    for num_requests in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_requests),
            num_requests,
            |b, &num_requests| {
                b.to_async(&rt).iter(|| {
                    let client = client.clone();
                    async move {
                        let request = bench_small::EchoRequest {
                            message: generate_small_payload(),
                        };
                        let start = Instant::now();
                        for _ in 0..num_requests {
                            let _response = client.echo(black_box(request.clone())).await.unwrap();
                        }
                        let elapsed = start.elapsed();
                        let throughput = num_requests as f64 / elapsed.as_secs_f64();
                        criterion::black_box(throughput);
                    }
                });
            },
        );
    }
    group.finish();

    rt.block_on(server_handle.shutdown());
}

async fn setup_throughput_medium() -> Result<
    (common::ServerHandle, bench_medium::BenchMediumClient),
    Box<dyn std::error::Error + Send + Sync>,
> {
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

    let client = bench_medium::BenchMediumClient::try_new(addr).await?;

    Ok((server_handle, client))
}

fn bench_throughput_medium(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (server_handle, client) = rt.block_on(setup_throughput_medium()).unwrap();
    let client = std::sync::Arc::new(client);

    let mut group = c.benchmark_group("throughput_medium");

    for num_requests in [100, 1000, 5000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_requests),
            num_requests,
            |b, &num_requests| {
                b.to_async(&rt).iter(|| {
                    let client = client.clone();
                    async move {
                        let request = bench_medium::EchoRequest {
                            data: generate_medium_payload(),
                        };
                        let start = Instant::now();
                        for _ in 0..num_requests {
                            let _response = client.echo(black_box(request.clone())).await.unwrap();
                        }
                        let elapsed = start.elapsed();
                        let throughput = num_requests as f64 / elapsed.as_secs_f64();
                        criterion::black_box(throughput);
                    }
                });
            },
        );
    }
    group.finish();

    rt.block_on(server_handle.shutdown());
}

async fn setup_throughput_large() -> Result<
    (common::ServerHandle, bench_large::BenchLargeClient),
    Box<dyn std::error::Error + Send + Sync>,
> {
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

    let client = bench_large::BenchLargeClient::try_new(addr).await?;

    Ok((server_handle, client))
}

fn bench_throughput_large(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (server_handle, client) = rt.block_on(setup_throughput_large()).unwrap();
    let client = std::sync::Arc::new(client);

    let mut group = c.benchmark_group("throughput_large");

    for num_requests in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_requests),
            num_requests,
            |b, &num_requests| {
                b.to_async(&rt).iter(|| {
                    let client = client.clone();
                    async move {
                        let (items, metadata) = generate_large_payload();
                        let request = bench_large::EchoRequest { items, metadata };
                        let start = Instant::now();
                        for _ in 0..num_requests {
                            let _response = client.echo(black_box(request.clone())).await.unwrap();
                        }
                        let elapsed = start.elapsed();
                        let throughput = num_requests as f64 / elapsed.as_secs_f64();
                        criterion::black_box(throughput);
                    }
                });
            },
        );
    }
    group.finish();

    rt.block_on(server_handle.shutdown());
}

criterion_group!(
    benches,
    bench_throughput_small,
    bench_throughput_medium,
    bench_throughput_large
);
criterion_main!(benches);
