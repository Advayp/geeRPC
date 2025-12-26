// Integration tests for the code generation macro
use geerpc::rpc_gen;

// Test 1: Simple service with default parameters
#[test]
fn test_simple_service_generation() {
    rpc_gen!("tests/fixtures/simple.yaml");
    
    // Verify the module was created
    let _ = ping::PingRequest {
        message: "test".to_string(),
    };
    let _ = ping::PingResponse {
        message: "response".to_string(),
    };
    
    // Type check that client and server exist
    fn _check_client_exists(_: ping::PingClient) {}
    fn _check_server_exists<T: ping::PingService>(_: ping::PingServer<T>) {}
}

// Test 2: Client-only generation
#[test]
fn test_client_only_generation() {
    rpc_gen!("tests/fixtures/simple.yaml", client = true, server = false, module = "ping_client_only");
    
    // Client should exist
    fn _check_client_exists(_: ping_client_only::PingClient) {}
    
    // Server types should NOT exist - this would fail to compile if server was generated
    // We can't easily test absence, but the presence of client is enough
}

// Test 3: Server-only generation
#[test]
fn test_server_only_generation() {
    rpc_gen!("tests/fixtures/simple.yaml", client = false, server = true, module = "ping_server_only");
    
    // Server should exist
    fn _check_server_exists<T: ping_server_only::PingService>(_: ping_server_only::PingServer<T>) {}
}

// Test 4: Custom module name
#[test]
fn test_custom_module_name() {
    rpc_gen!("tests/fixtures/simple.yaml", module = "my_custom_ping");
    
    let _ = my_custom_ping::PingRequest {
        message: "test".to_string(),
    };
}

// Test 5: Multiple methods
#[test]
fn test_multi_method_service() {
    rpc_gen!("tests/fixtures/multi_method.yaml");
    
    // Verify all request/response types exist
    let _ = user::GetUserRequest { id: 1 };
    let _ = user::GetUserResponse {
        name: "test".to_string(),
        email: "test@example.com".to_string(),
    };
    
    let _ = user::CreateUserRequest {
        name: "test".to_string(),
        email: "test@example.com".to_string(),
    };
    let _ = user::CreateUserResponse {
        id: 1,
        success: true,
    };
    
    let _ = user::DeleteUserRequest { id: 1 };
    let _ = user::DeleteUserResponse { success: true };
}

// Test 6: Multiple fields per struct
#[test]
fn test_multi_field_structs() {
    rpc_gen!("tests/fixtures/multi_field.yaml");
    
    let _ = database::QueryRequest {
        table: "users".to_string(),
        filter: "age > 18".to_string(),
        limit: 10,
        offset: 0,
    };
    
    let _ = database::QueryResponse {
        rows: vec!["row1".to_string()],
        count: 100,
        has_more: true,
    };
}

// Test 7: Complex types (Vec, Option, etc.)
#[test]
fn test_complex_types() {
    rpc_gen!("tests/fixtures/complex_types.yaml");
    
    let _ = data_service::ProcessDataRequest {
        data: vec![1, 2, 3],
        metadata: Some("test".to_string()),
        count: 42,
        active: true,
    };
    
    let _ = data_service::ProcessDataResponse {
        result: vec!["a".to_string(), "b".to_string()],
        error: None,
        processed: 10,
    };
}

// Test 8: Verify structs are serializable (important for RPC)
#[test]
fn test_serialization() {
    rpc_gen!("tests/fixtures/simple.yaml", module = "ping_serialize");
    
    let request = ping_serialize::PingRequest {
        message: "hello".to_string(),
    };
    
    // Should be able to serialize/deserialize
    let serialized = bincode::serialize(&request).unwrap();
    let deserialized: ping_serialize::PingRequest = bincode::deserialize(&serialized).unwrap();
    
    assert_eq!(request, deserialized);
}

// Test 9: Verify PartialEq works
#[test]
fn test_partial_eq() {
    rpc_gen!("tests/fixtures/simple.yaml", module = "ping_eq");
    
    let req1 = ping_eq::PingRequest {
        message: "test".to_string(),
    };
    let req2 = ping_eq::PingRequest {
        message: "test".to_string(),
    };
    let req3 = ping_eq::PingRequest {
        message: "different".to_string(),
    };
    
    assert_eq!(req1, req2);
    assert_ne!(req1, req3);
}

// Test 10: Verify Clone works
#[test]
fn test_clone() {
    rpc_gen!("tests/fixtures/simple.yaml", module = "ping_clone");
    
    let req = ping_clone::PingRequest {
        message: "test".to_string(),
    };
    let cloned = req.clone();
    
    assert_eq!(req, cloned);
}

