use std::sync::Arc;
use async_trait::async_trait;
use mcp_rust_sdk::{
    error::Error,
    server::{Server, ServerHandler},
    transport::websocket::WebSocketTransport,
    types::{ClientCapabilities, Implementation, ServerCapabilities},
};

struct ExampleHandler;

#[async_trait]
impl ServerHandler for ExampleHandler {
    async fn initialize(
        &self,
        implementation: Implementation,
        _capabilities: ClientCapabilities,
    ) -> Result<ServerCapabilities, Error> {
        println!("Client connected: {} v{}", implementation.name, implementation.version);
        Ok(ServerCapabilities::default())
    }

    async fn shutdown(&self) -> Result<(), Error> {
        println!("Server shutting down");
        Ok(())
    }

    async fn handle_method(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Error> {
        println!("Received method call: {} with params: {:?}", method, params);
        Ok(serde_json::json!({
            "status": "success",
            "message": "Hello from Rust MCP server!"
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create WebSocket transport
    let transport = WebSocketTransport::new("ws://localhost:8080").await?;
    
    // Create MCP server
    let server = Server::new(Arc::new(transport), Arc::new(ExampleHandler));

    println!("Starting MCP server on ws://localhost:8080...");
    server.start().await?;

    Ok(())
}
