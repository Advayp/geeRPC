// Note: This test file uses internal APIs via the internal-testing feature
// These APIs are not part of the public interface
#[cfg(feature = "internal-testing")]
use geerpc::client::{RPCClient, RPCClientBuilder};
#[cfg(feature = "internal-testing")]
use geerpc::handle_connection;
#[cfg(feature = "internal-testing")]
use geerpc::server::RPCServer;
#[cfg(feature = "internal-testing")]
use std::sync::Arc;
#[cfg(feature = "internal-testing")]
use std::time::Duration;
#[cfg(feature = "internal-testing")]
use tokio::time::sleep;

// Helper function to setup server on random port
#[cfg(feature = "internal-testing")]
async fn setup_server(server: RPCServer) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handlers = Arc::new(server.handlers);

    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let handlers = handlers.clone();
            tokio::spawn(async move {
                let _ = handle_connection(stream, handlers).await;
            });
        }
    });

    sleep(Duration::from_millis(50)).await;
    format!("{}", addr)
}

// Helper to create client with read loop running
#[cfg(feature = "internal-testing")]
async fn create_client_with_read_loop(addr: String) -> Arc<RPCClient> {
    let client = RPCClientBuilder::new().address(addr).build().await.unwrap();
    sleep(Duration::from_millis(20)).await;
    client
}

#[tokio::test]
#[cfg(feature = "internal-testing")]
async fn test_e2e_simple_echo() {
    // Setup server with echo service
    let mut server = RPCServer::new();
    server.register_service(
        "Echo".to_string(),
        "echo".to_string(),
        Box::new(|payload: Vec<u8>| Box::pin(async move { Ok(payload) })),
    );

    let addr = setup_server(server).await;
    let client = create_client_with_read_loop(addr).await;

    // Test echo
    let result = client.call("Echo", "echo", vec![1, 2, 3, 4]).await.unwrap();
    assert_eq!(result, vec![1, 2, 3, 4]);
}

#[tokio::test]
#[cfg(feature = "internal-testing")]
async fn test_e2e_multiple_services() {
    // Setup server with multiple services
    let mut server = RPCServer::new();

    // Echo service
    server.register_service(
        "Echo".to_string(),
        "echo".to_string(),
        Box::new(|payload: Vec<u8>| Box::pin(async move { Ok(payload) })),
    );

    // Math service - Double
    server.register_service(
        "Math".to_string(),
        "Double".to_string(),
        Box::new(|payload: Vec<u8>| {
            Box::pin(async move {
                let result: Vec<u8> = payload.iter().map(|x| x * 2).collect();
                Ok(result)
            })
        }),
    );

    // Math service - Add
    server.register_service(
        "Math".to_string(),
        "Add".to_string(),
        Box::new(|payload: Vec<u8>| {
            Box::pin(async move {
                let sum: u8 = payload.iter().sum();
                Ok(vec![sum])
            })
        }),
    );

    // String service - Reverse
    server.register_service(
        "String".to_string(),
        "Reverse".to_string(),
        Box::new(|payload: Vec<u8>| {
            Box::pin(async move {
                let mut result = payload;
                result.reverse();
                Ok(result)
            })
        }),
    );

    let addr = setup_server(server).await;
    let client = create_client_with_read_loop(addr).await;

    // Test Echo
    let result = client.call("Echo", "echo", vec![5, 10, 15]).await.unwrap();
    assert_eq!(result, vec![5, 10, 15]);

    // Test Math.Double
    let result = client.call("Math", "Double", vec![1, 2, 3]).await.unwrap();
    assert_eq!(result, vec![2, 4, 6]);

    // Test Math.Add
    let result = client.call("Math", "Add", vec![1, 2, 3, 4]).await.unwrap();
    assert_eq!(result, vec![10]);

    // Test String.Reverse
    let result = client
        .call("String", "Reverse", vec![1, 2, 3, 4, 5])
        .await
        .unwrap();
    assert_eq!(result, vec![5, 4, 3, 2, 1]);
}

#[tokio::test]
#[cfg(feature = "internal-testing")]
async fn test_e2e_concurrent_calls_single_client() {
    // Setup server with a service that has a delay
    let mut server = RPCServer::new();
    server.register_service(
        "Delayed".to_string(),
        "sleep".to_string(),
        Box::new(|payload: Vec<u8>| {
            Box::pin(async move {
                // Sleep for a duration based on the first byte
                let delay_ms = payload.first().unwrap_or(&10) * 10;
                sleep(Duration::from_millis(delay_ms as u64)).await;
                Ok(payload)
            })
        }),
    );

    let addr = setup_server(server).await;
    let client = create_client_with_read_loop(addr).await;

    // Make concurrent calls
    let mut handles = vec![];
    for i in 0..10 {
        let client_clone = client.clone();
        let handle = tokio::spawn(async move {
            let payload = vec![i];
            let result = client_clone
                .call("Delayed", "sleep", payload.clone())
                .await
                .unwrap();
            assert_eq!(result, payload);
        });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
#[cfg(feature = "internal-testing")]
async fn test_e2e_concurrent_calls_multiple_services() {
    // Setup server with multiple services
    let mut server = RPCServer::new();

    server.register_service(
        "ServiceA".to_string(),
        "method".to_string(),
        Box::new(|payload: Vec<u8>| {
            Box::pin(async move {
                sleep(Duration::from_millis(10)).await;
                Ok(vec![payload[0] + 1])
            })
        }),
    );

    server.register_service(
        "ServiceB".to_string(),
        "method".to_string(),
        Box::new(|payload: Vec<u8>| {
            Box::pin(async move {
                sleep(Duration::from_millis(10)).await;
                Ok(vec![payload[0] + 2])
            })
        }),
    );

    server.register_service(
        "ServiceC".to_string(),
        "method".to_string(),
        Box::new(|payload: Vec<u8>| {
            Box::pin(async move {
                sleep(Duration::from_millis(10)).await;
                Ok(vec![payload[0] + 3])
            })
        }),
    );

    let addr = setup_server(server).await;
    let client = create_client_with_read_loop(addr).await;

    // Make concurrent calls to different services
    let mut handles = vec![];

    // ServiceA calls
    for i in 0..5 {
        let client_clone = client.clone();
        let handle = tokio::spawn(async move {
            let result = client_clone
                .call("ServiceA", "method", vec![i])
                .await
                .unwrap();
            assert_eq!(result, vec![i + 1]);
        });
        handles.push(handle);
    }

    // ServiceB calls
    for i in 0..5 {
        let client_clone = client.clone();
        let handle = tokio::spawn(async move {
            let result = client_clone
                .call("ServiceB", "method", vec![i])
                .await
                .unwrap();
            assert_eq!(result, vec![i + 2]);
        });
        handles.push(handle);
    }

    // ServiceC calls
    for i in 0..5 {
        let client_clone = client.clone();
        let handle = tokio::spawn(async move {
            let result = client_clone
                .call("ServiceC", "method", vec![i])
                .await
                .unwrap();
            assert_eq!(result, vec![i + 3]);
        });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
#[cfg(feature = "internal-testing")]
async fn test_e2e_high_concurrency() {
    // Setup server
    let mut server = RPCServer::new();
    server.register_service(
        "Counter".to_string(),
        "increment".to_string(),
        Box::new(|payload: Vec<u8>| {
            Box::pin(async move {
                let val = payload[0];
                Ok(vec![val + 1])
            })
        }),
    );

    let addr = setup_server(server).await;
    let client = create_client_with_read_loop(addr).await;

    // Make 100 concurrent calls
    let mut handles = vec![];
    for i in 0..100 {
        let client_clone = client.clone();
        let handle = tokio::spawn(async move {
            let val = (i % 256) as u8;
            let result = client_clone
                .call("Counter", "increment", vec![val])
                .await
                .unwrap();
            assert_eq!(result, vec![val + 1]);
        });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
#[cfg(feature = "internal-testing")]
async fn test_e2e_interleaved_calls() {
    // Setup server with fast and slow services
    let mut server = RPCServer::new();

    server.register_service(
        "Fast".to_string(),
        "method".to_string(),
        Box::new(|payload: Vec<u8>| {
            Box::pin(async move {
                // No delay
                Ok(vec![payload[0] * 2])
            })
        }),
    );

    server.register_service(
        "Slow".to_string(),
        "method".to_string(),
        Box::new(|payload: Vec<u8>| {
            Box::pin(async move {
                sleep(Duration::from_millis(100)).await;
                Ok(vec![payload[0] * 3])
            })
        }),
    );

    let addr = setup_server(server).await;
    let client = create_client_with_read_loop(addr).await;

    // Start slow call first
    let client_slow = client.clone();
    let slow_handle = tokio::spawn(async move {
        let result = client_slow.call("Slow", "method", vec![5]).await.unwrap();
        assert_eq!(result, vec![15]);
    });

    // Give it a moment to start
    sleep(Duration::from_millis(10)).await;

    // Make multiple fast calls that should complete before slow
    let mut fast_handles = vec![];
    for i in 0..5 {
        let client_fast = client.clone();
        let handle = tokio::spawn(async move {
            let result = client_fast.call("Fast", "method", vec![i]).await.unwrap();
            assert_eq!(result, vec![i * 2]);
        });
        fast_handles.push(handle);
    }

    // All fast calls should complete
    for handle in fast_handles {
        handle.await.unwrap();
    }

    // Slow call should still complete correctly
    slow_handle.await.unwrap();
}

#[tokio::test]
#[cfg(feature = "internal-testing")]
async fn test_e2e_large_payload() {
    // Setup server
    let mut server = RPCServer::new();
    server.register_service(
        "Data".to_string(),
        "process".to_string(),
        Box::new(|payload: Vec<u8>| {
            Box::pin(async move {
                // Return reversed payload
                let mut result = payload;
                result.reverse();
                Ok(result)
            })
        }),
    );

    let addr = setup_server(server).await;
    let client = create_client_with_read_loop(addr).await;

    // Create large payload (1MB)
    let large_payload: Vec<u8> = (0..1_000_000).map(|x| (x % 256) as u8).collect();
    let mut expected = large_payload.clone();
    expected.reverse();

    let result = client.call("Data", "process", large_payload).await.unwrap();

    assert_eq!(result.len(), 1_000_000);
    assert_eq!(result, expected);
}

#[tokio::test]
#[cfg(feature = "internal-testing")]
async fn test_e2e_mixed_payload_sizes() {
    // Setup server
    let mut server = RPCServer::new();
    server.register_service(
        "Echo".to_string(),
        "echo".to_string(),
        Box::new(|payload: Vec<u8>| Box::pin(async move { Ok(payload) })),
    );

    let addr = setup_server(server).await;
    let client = create_client_with_read_loop(addr).await;

    // Make concurrent calls with different payload sizes
    let mut handles = vec![];

    for size in [1, 10, 100, 1000, 10000, 100000].iter() {
        let client_clone = client.clone();
        let size = *size;
        let handle = tokio::spawn(async move {
            let payload: Vec<u8> = (0..size).map(|x| (x % 256) as u8).collect();
            let result = client_clone
                .call("Echo", "echo", payload.clone())
                .await
                .unwrap();
            assert_eq!(result, payload);
        });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        handle.await.unwrap();
    }
}
