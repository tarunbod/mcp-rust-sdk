use async_trait::async_trait;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{
    error::{Error, ErrorCode},
    protocol::{Request, RequestId, Response, Notification},
    transport::{Message, Transport},
    types::{ClientCapabilities, Implementation, ServerCapabilities},
};

/// Trait for implementing MCP server handlers
#[async_trait]
pub trait ServerHandler: Send + Sync {
    /// Handle initialization
    async fn initialize(
        &self,
        implementation: Implementation,
        capabilities: ClientCapabilities,
    ) -> Result<ServerCapabilities, Error>;

    /// Handle shutdown request
    async fn shutdown(&self) -> Result<(), Error>;

    /// Handle custom method calls
    async fn handle_method(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Error>;
}

/// Server state
pub struct Server {
    transport: Arc<dyn Transport>,
    handler: Arc<dyn ServerHandler>,
    initialized: Arc<RwLock<bool>>,
}

impl Server {
    /// Create a new MCP server
    pub fn new(transport: Arc<dyn Transport>, handler: Arc<dyn ServerHandler>) -> Self {
        Self {
            transport,
            handler,
            initialized: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the server
    pub async fn start(&self) -> Result<(), Error> {
        let mut stream = self.transport.receive();

        while let Some(message) = stream.next().await {
            match message? {
                Message::Request(request) => {
                    let response = self.handle_request(request).await?;
                    self.transport.send(Message::Response(response)).await?;
                }
                Message::Notification(notification) => {
                    match notification.method.as_str() {
                        "exit" => break,
                        "initialized" => {
                            *self.initialized.write().await = true;
                        }
                        _ => {
                            // Handle other notifications
                        }
                    }
                }
                Message::Response(_) => {
                    // Server shouldn't receive responses
                    return Err(Error::protocol(
                        ErrorCode::InvalidRequest,
                        "Server received unexpected response",
                    ));
                }
            }
        }

        Ok(())
    }

    async fn handle_request(&self, request: Request) -> Result<Response, Error> {
        let initialized = *self.initialized.read().await;

        match request.method.as_str() {
            "initialize" => {
                if initialized {
                    return Err(Error::protocol(
                        ErrorCode::InvalidRequest,
                        "Server already initialized",
                    ));
                }

                let params: serde_json::Value = request.params.unwrap_or(serde_json::json!({}));
                let implementation: Implementation =
                    serde_json::from_value(params.get("implementation").cloned().unwrap_or_default())?;
                let capabilities: ClientCapabilities =
                    serde_json::from_value(params.get("capabilities").cloned().unwrap_or_default())?;

                let result = self.handler.initialize(implementation, capabilities).await?;
                Ok(Response::success(request.id, Some(serde_json::to_value(result)?)))
            }
            "shutdown" => {
                if !initialized {
                    return Err(Error::protocol(
                        ErrorCode::ServerNotInitialized,
                        "Server not initialized",
                    ));
                }

                self.handler.shutdown().await?;
                Ok(Response::success(request.id, None))
            }
            _ => {
                if !initialized {
                    return Err(Error::protocol(
                        ErrorCode::ServerNotInitialized,
                        "Server not initialized",
                    ));
                }

                let result = self.handler.handle_method(&request.method, request.params).await?;
                Ok(Response::success(request.id, Some(result)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        protocol::{Request, RequestId, Notification},
        transport::{Message, stdio::StdioTransport},
    };
    use tokio::time::Duration;

    struct TestHandler;

    #[async_trait]
    impl ServerHandler for TestHandler {
        async fn initialize(
            &self,
            _implementation: Implementation,
            _capabilities: ClientCapabilities,
        ) -> Result<ServerCapabilities, Error> {
            Ok(ServerCapabilities::default())
        }

        async fn shutdown(&self) -> Result<(), Error> {
            Ok(())
        }

        async fn handle_method(
            &self,
            _method: &str,
            _params: Option<serde_json::Value>,
        ) -> Result<serde_json::Value, Error> {
            Ok(serde_json::json!({}))
        }
    }

    #[tokio::test]
    async fn test_server_initialization() {
        // Create transport and server
        let (transport, tx) = StdioTransport::new();
        let server = Server::new(Arc::new(transport), Arc::new(TestHandler));

        // Start server in background
        let server_handle = tokio::spawn(async move {
            if let Err(e) = server.start().await {
                eprintln!("Server error: {}", e);
            }
        });

        // Give server time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Send initialize request
        let init_request = Request::new(
            "initialize",
            Some(serde_json::json!({
                "implementation": {
                    "name": "test-client",
                    "version": "0.1.0"
                },
                "capabilities": {},
                "protocolVersion": "2024-11-05"
            })),
            RequestId::Number(1),
        );

        tx.send(Ok(Message::Request(init_request))).unwrap();

        // Give server time to process request
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Send shutdown request
        let shutdown_request = Request::new(
            "shutdown",
            None,
            RequestId::Number(2),
        );
        tx.send(Ok(Message::Request(shutdown_request))).unwrap();

        // Give server time to process request
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Send exit notification
        let exit_notification = Notification::new("exit", None);
        let _ = tx.send(Ok(Message::Notification(exit_notification)));

        // Wait for server to stop with timeout
        let _ = tokio::time::timeout(Duration::from_secs(1), server_handle).await;
    }
}
