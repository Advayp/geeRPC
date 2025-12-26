// End-to-end test that verifies generated code works with the geerpc library
use geerpc::rpc_gen;
use std::time::Duration;
use tokio::time::sleep;

// Generate the ping service
rpc_gen!("tests/fixtures/simple.yaml", module = "ping_e2e");

// Implement the service
struct MyPingService;

#[async_trait::async_trait]
impl ping_e2e::PingService for MyPingService {
    async fn ping(
        &self,
        request: ping_e2e::PingRequest,
    ) -> Result<ping_e2e::PingResponse, geerpc::Error> {
        Ok(ping_e2e::PingResponse {
            message: format!("Echo: {}", request.message),
        })
    }
}

#[tokio::test]
async fn test_end_to_end_communication() {
    let addr = "127.0.0.1:50053";

    // Start server in background task
    let server_handle = tokio::spawn(async move {
        let mut server = geerpc::server::RPCServer::new();
        let service = MyPingService;
        let ping_server = ping_e2e::PingServer::new(service);
        ping_server.register(&mut server);

        // Run server (this will run until the task is cancelled)
        let _ = server.serve(addr).await;
    });

    // Give server time to start
    sleep(Duration::from_millis(100)).await;

    // Create client and make a call
    let client = ping_e2e::PingClient::try_new(addr.to_string())
        .await
        .expect("Failed to create client");

    let request = ping_e2e::PingRequest {
        message: "Hello, World!".to_string(),
    };

    let response = client.ping(request).await.expect("Failed to call ping");

    assert_eq!(response.message, "Echo: Hello, World!");

    // Clean up
    server_handle.abort();
}

// Test with multiple methods
rpc_gen!("tests/fixtures/multi_method.yaml", module = "user_e2e");

struct MyUserService;

#[async_trait::async_trait]
impl user_e2e::UserService for MyUserService {
    async fn get_user(
        &self,
        request: user_e2e::GetUserRequest,
    ) -> Result<user_e2e::GetUserResponse, geerpc::Error> {
        Ok(user_e2e::GetUserResponse {
            name: format!("User {}", request.id),
            email: format!("user{}@example.com", request.id),
        })
    }

    async fn create_user(
        &self,
        _request: user_e2e::CreateUserRequest,
    ) -> Result<user_e2e::CreateUserResponse, geerpc::Error> {
        Ok(user_e2e::CreateUserResponse {
            id: 123,
            success: true,
        })
    }

    async fn delete_user(
        &self,
        _request: user_e2e::DeleteUserRequest,
    ) -> Result<user_e2e::DeleteUserResponse, geerpc::Error> {
        Ok(user_e2e::DeleteUserResponse { success: true })
    }
}

#[tokio::test]
async fn test_multi_method_service() {
    let addr = "127.0.0.1:50054";

    // Start server
    let server_handle = tokio::spawn(async move {
        let mut server = geerpc::server::RPCServer::new();
        let service = MyUserService;
        let user_server = user_e2e::UserServer::new(service);
        user_server.register(&mut server);

        let _ = server.serve(addr).await;
    });

    sleep(Duration::from_millis(100)).await;

    // Create client
    let client = user_e2e::UserClient::try_new(addr.to_string())
        .await
        .expect("Failed to create client");

    // Test get_user
    let get_response = client
        .get_user(user_e2e::GetUserRequest { id: 42 })
        .await
        .expect("Failed to call get_user");
    assert_eq!(get_response.name, "User 42");
    assert_eq!(get_response.email, "user42@example.com");

    // Test create_user
    let create_response = client
        .create_user(user_e2e::CreateUserRequest {
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
        })
        .await
        .expect("Failed to call create_user");
    assert_eq!(create_response.id, 123);
    assert!(create_response.success);

    // Test delete_user
    let delete_response = client
        .delete_user(user_e2e::DeleteUserRequest { id: 42 })
        .await
        .expect("Failed to call delete_user");
    assert!(delete_response.success);

    // Clean up
    server_handle.abort();
}

// Test complex types
rpc_gen!("tests/fixtures/complex_types.yaml", module = "data_e2e");

struct MyDataService;

#[async_trait::async_trait]
impl data_e2e::DataServiceService for MyDataService {
    async fn process_data(
        &self,
        request: data_e2e::ProcessDataRequest,
    ) -> Result<data_e2e::ProcessDataResponse, geerpc::Error> {
        let processed = request.data.len();
        let result = vec![
            format!("Processed {} bytes", processed),
            format!("Metadata: {:?}", request.metadata),
        ];

        Ok(data_e2e::ProcessDataResponse {
            result,
            error: None,
            processed,
        })
    }
}

#[tokio::test]
async fn test_complex_types_service() {
    let addr = "127.0.0.1:50055";

    // Start server
    let server_handle = tokio::spawn(async move {
        let mut server = geerpc::server::RPCServer::new();
        let service = MyDataService;
        let data_server = data_e2e::DataServiceServer::new(service);
        data_server.register(&mut server);

        let _ = server.serve(addr).await;
    });

    sleep(Duration::from_millis(100)).await;

    // Create client
    let client = data_e2e::DataServiceClient::try_new(addr.to_string())
        .await
        .expect("Failed to create client");

    // Test with complex types
    let response = client
        .process_data(data_e2e::ProcessDataRequest {
            data: vec![1, 2, 3, 4, 5],
            metadata: Some("test metadata".to_string()),
            count: 100,
            active: true,
        })
        .await
        .expect("Failed to call process_data");

    assert_eq!(response.processed, 5);
    assert_eq!(response.result.len(), 2);
    assert!(response.error.is_none());

    // Clean up
    server_handle.abort();
}
