use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// Payload size categories
pub enum PayloadSize {
    Small,  // 1-100 bytes
    Medium, // 1-10 KB
    Large,  // 100 KB - 1 MB
}

/// Generate a small payload (String)
pub fn generate_small_payload() -> String {
    "Hello, World! This is a small payload for benchmarking.".to_string()
}

/// Generate a medium payload (Vec<u8>)
pub fn generate_medium_payload() -> Vec<u8> {
    // Generate ~5KB of data
    (0..5120).map(|i| (i % 256) as u8).collect()
}

/// Generate a large payload (Vec<Vec<u8>> with metadata)
pub fn generate_large_payload() -> (Vec<Vec<u8>>, String) {
    // Generate ~500KB of data across multiple vectors
    let mut items = Vec::new();
    for _ in 0..100 {
        let item: Vec<u8> = (0..5120).map(|i| (i % 256) as u8).collect();
        items.push(item);
    }
    let metadata = "Large payload metadata for benchmarking".to_string();
    (items, metadata)
}

/// Server handle for managing server lifecycle
pub struct ServerHandle {
    pub shutdown_tx: Option<oneshot::Sender<()>>,
    pub handle: JoinHandle<()>,
    pub address: String,
}

impl ServerHandle {
    pub fn address(&self) -> &str {
        &self.address
    }

    pub async fn shutdown(self) {
        if let Some(tx) = self.shutdown_tx {
            let _ = tx.send(());
        }
        self.handle.abort();
        let _ = self.handle.await;
    }
}

type HandlerFn = Box<
    dyn Fn(
            Vec<u8>,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<Vec<u8>, geerpc::RPCStatus>> + Send>,
        > + Send
        + Sync,
>;

/// Setup a test server with the given handler
pub async fn setup_server(
    handler: HandlerFn,
) -> Result<ServerHandle, Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?.to_string();

    let mut server = geerpc::server::RPCServer::new();
    // Wrap handler in Arc so it can be cloned for multiple calls
    let handler = Arc::new(handler);
    let handler_clone = handler.clone();
    server.register_service(
        "TestService".to_string(),
        "Echo".to_string(),
        Box::new(move |payload: Vec<u8>| {
            let handler = handler_clone.clone();
            handler(payload)
        }),
    );

    let handlers = Arc::new(server.handlers);
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

    let handle = tokio::spawn(async move {
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

    // Give server a moment to start and be ready for connections
    tokio::time::sleep(Duration::from_millis(100)).await;

    Ok(ServerHandle {
        shutdown_tx: Some(shutdown_tx),
        handle,
        address: addr,
    })
}

/// Calculate percentiles from a sorted vector of durations
pub fn calculate_percentiles(durations: &[Duration], percentile: f64) -> Duration {
    if durations.is_empty() {
        return Duration::ZERO;
    }
    let index = ((durations.len() - 1) as f64 * percentile / 100.0) as usize;
    durations[index.min(durations.len() - 1)]
}

/// Statistics collector for latency measurements
pub struct LatencyStats {
    durations: Vec<Duration>,
}

impl LatencyStats {
    pub fn new() -> Self {
        Self {
            durations: Vec::new(),
        }
    }

    pub fn record(&mut self, duration: Duration) {
        self.durations.push(duration);
    }

    pub fn calculate_stats(&mut self) -> (Duration, Duration, Duration, Duration) {
        self.durations.sort();
        let p50 = calculate_percentiles(&self.durations, 50.0);
        let p95 = calculate_percentiles(&self.durations, 95.0);
        let p99 = calculate_percentiles(&self.durations, 99.0);
        let mean = if self.durations.is_empty() {
            Duration::ZERO
        } else {
            let sum: Duration = self.durations.iter().sum();
            sum / self.durations.len() as u32
        };
        (mean, p50, p95, p99)
    }

    pub fn clear(&mut self) {
        self.durations.clear();
    }
}

impl Default for LatencyStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Throughput calculator
pub struct ThroughputCalculator {
    start: Instant,
    count: u64,
}

impl ThroughputCalculator {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            count: 0,
        }
    }

    pub fn increment(&mut self) {
        self.count += 1;
    }

    pub fn calculate(&self) -> f64 {
        let elapsed = self.start.elapsed();
        if elapsed.as_secs_f64() > 0.0 {
            self.count as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        }
    }

    pub fn reset(&mut self) {
        self.start = Instant::now();
        self.count = 0;
    }
}

impl Default for ThroughputCalculator {
    fn default() -> Self {
        Self::new()
    }
}
