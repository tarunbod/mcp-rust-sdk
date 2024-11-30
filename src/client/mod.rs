use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{
    error::Error,
    protocol::{Notification, Request, RequestId},
    transport::{Message, Transport},
    types::{ClientCapabilities, Implementation, ServerCapabilities},
};

/// MCP client state
pub struct Client {
    transport: Arc<dyn Transport>,
    server_capabilities: Arc<RwLock<Option<ServerCapabilities>>>,
    request_counter: Arc<RwLock<i64>>,
}

impl Client {
    /// Create a new MCP client with the given transport
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        Self {
            transport,
            server_capabilities: Arc::new(RwLock::new(None)),
            request_counter: Arc::new(RwLock::new(0)),
        }
    }

    /// Initialize the client
    pub async fn initialize(
        &self,
        implementation: Implementation,
        capabilities: ClientCapabilities,
    ) -> Result<ServerCapabilities, Error> {
        let params = serde_json::json!({
            "implementation": implementation,
            "capabilities": capabilities,
            "protocolVersion": crate::LATEST_PROTOCOL_VERSION,
        });

        let response = self
            .request("initialize", Some(params))
            .await?;

        let server_capabilities: ServerCapabilities = serde_json::from_value(response)?;
        *self.server_capabilities.write().await = Some(server_capabilities.clone());

        // Send initialized notification
        self.notify("initialized", None).await?;

        Ok(server_capabilities)
    }

    /// Send a request to the server
    pub async fn request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Error> {
        let mut counter = self.request_counter.write().await;
        *counter += 1;
        let id = RequestId::Number(*counter);

        let request = Request::new(method, params, id.clone());
        self.transport.send(Message::Request(request)).await?;

        // TODO: Implement response handling
        // This is a simplified version that doesn't actually wait for the response
        Err(Error::protocol(
            crate::error::ErrorCode::InternalError,
            "Response handling not implemented",
        ))
    }

    /// Send a notification to the server
    pub async fn notify(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<(), Error> {
        let notification = Notification::new(method, params);
        self.transport.send(Message::Notification(notification)).await
    }

    /// Get the server capabilities
    pub async fn capabilities(&self) -> Option<ServerCapabilities> {
        self.server_capabilities.read().await.clone()
    }

    /// Close the client connection
    pub async fn shutdown(&self) -> Result<(), Error> {
        // Send shutdown request
        self.request("shutdown", None).await?;

        // Send exit notification
        self.notify("exit", None).await?;

        // Close transport
        self.transport.close().await
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::transport::stdio::StdioTransport;

//     #[tokio::test]
//     async fn test_client_initialization() {
//         let (transport, _sender) = StdioTransport::new();
//         let client = Client::new(Arc::new(transport));

//         let implementation = Implementation {
//             name: "test-client".to_string(),
//             version: "0.1.0".to_string(),
//         };

//         let capabilities = ClientCapabilities::default();

//         // This will fail because we haven't implemented response handling
//         let result = client.initialize(implementation, capabilities).await;
//         assert!(result.is_err());
//     }
// }
