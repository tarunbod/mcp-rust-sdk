//! WebSocket Transport Implementation
//! 
//! This module provides a transport implementation that uses WebSocket protocol
//! for communication. This transport is ideal for:
//! - Network-based client-server communication
//! - Real-time bidirectional messaging
//! - Web-based applications
//! - Scenarios requiring secure communication (WSS)
//!
//! The implementation uses tokio-tungstenite for WebSocket functionality and
//! provides thread-safe connection management through Arc and Mutex.

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use futures_util::SinkExt;
use std::{pin::Pin, sync::Arc};
use tokio::sync::Mutex;
use tokio_tungstenite::{
    connect_async,
    tungstenite::protocol::Message as WsMessage,
    WebSocketStream,
};
use url::Url;

use crate::{
    error::{Error, ErrorCode},
    protocol::Notification,
    transport::{Message, Transport},
};

/// Type alias for the WebSocket connection stream
type WebSocketConnection = WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// A transport implementation that uses WebSocket protocol for communication.
///
/// This transport provides bidirectional communication over WebSocket protocol,
/// supporting both secure (WSS) and standard (WS) connections.
///
/// # Thread Safety
///
/// The implementation is thread-safe, using Arc and Mutex to protect the WebSocket
/// connection. This allows the transport to be used safely across multiple
/// threads in an async context.
///
/// # Message Flow
///
/// - Input: Messages are received as WebSocket frames and parsed as JSON-RPC messages
/// - Output: Messages are serialized to JSON and sent as WebSocket text frames
///
/// # Example
///
/// ```rust
/// use mcp_rust_sdk::transport::WebSocketTransport;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let transport = WebSocketTransport::new("ws://localhost:8080").await?;
///     // Use transport with a client or server...
///     Ok(())
/// }
/// ```
pub struct WebSocketTransport {
    /// Thread-safe handle to the WebSocket connection
    connection: Arc<Mutex<WebSocketConnection>>,
}

impl WebSocketTransport {
    /// Creates a new WebSocket transport by establishing a connection to the specified URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The WebSocket URL to connect to (ws:// or wss://)
    ///
    /// # Returns
    ///
    /// Returns a Result containing:
    /// - Ok: The new WebSocketTransport instance
    /// - Err: An error if connection fails
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The URL is invalid
    /// - The connection cannot be established
    /// - The WebSocket handshake fails
    pub async fn new(url: &str) -> Result<Self, Error> {
        let url = Url::parse(url).map_err(|e| Error::Other(e.to_string()))?;
        
        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|e| Error::Other(format!("WebSocket connection failed: {}", e)))?;

        Ok(Self {
            connection: Arc::new(Mutex::new(ws_stream)),
        })
    }

    /// Converts an MCP message to a WebSocket message.
    ///
    /// # Arguments
    ///
    /// * `message` - The MCP message to convert
    ///
    /// # Returns
    ///
    /// Returns a Result containing:
    /// - Ok: The WebSocket message
    /// - Err: An error if serialization fails
    fn convert_to_ws_message(message: &Message) -> Result<WsMessage, Error> {
        let json = serde_json::to_string(&message)?;
        Ok(WsMessage::Text(json))
    }

    /// Parses a WebSocket message into an MCP message.
    ///
    /// # Arguments
    ///
    /// * `message` - The WebSocket message to parse
    ///
    /// # Returns
    ///
    /// Returns a Result containing:
    /// - Ok: The parsed MCP message
    /// - Err: An error if parsing fails or message type is unsupported
    ///
    /// # Message Types
    ///
    /// - Text: Parsed as JSON-RPC message
    /// - Binary: Not supported, returns error
    /// - Ping/Pong: Ignored with error
    /// - Close: Returns error indicating connection closure
    fn parse_ws_message(message: WsMessage) -> Result<Message, Error> {
        match message {
            WsMessage::Text(text) => {
                serde_json::from_str(&text).map_err(|e| Error::Serialization(e.to_string()))
            }
            WsMessage::Binary(_) => Err(Error::protocol(
                ErrorCode::InvalidRequest,
                "Binary WebSocket messages are not supported",
            )),
            WsMessage::Ping(_) | WsMessage::Pong(_) => {
                // Ignore ping/pong messages
                Err(Error::protocol(
                    ErrorCode::InvalidRequest,
                    "Received ping/pong message",
                ))
            }
            WsMessage::Close(_) => Err(Error::protocol(
                ErrorCode::InvalidRequest,
                "WebSocket connection closed",
            )),
            WsMessage::Frame(_) => Err(Error::protocol(
                ErrorCode::InvalidRequest,
                "Raw WebSocket frames are not supported",
            )),
        }
    }
}

#[async_trait]
impl Transport for WebSocketTransport {
    /// Sends a message over the WebSocket connection.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to send
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the message was successfully sent,
    /// or an error if sending failed.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - Failed to convert the message to WebSocket format
    /// - Failed to acquire the connection lock
    /// - Failed to send the message over WebSocket
    async fn send(&self, message: Message) -> Result<(), Error> {
        let ws_message = Self::convert_to_ws_message(&message)?;
        let mut connection = self.connection.lock().await;
        connection
            .send(ws_message)
            .await
            .map_err(|e| Error::Other(format!("Failed to send WebSocket message: {}", e)))
    }

    /// Creates a stream of messages received from the WebSocket connection.
    ///
    /// # Returns
    ///
    /// Returns a pinned box containing a stream that yields Result<Message, Error>.
    /// The stream will continue until the WebSocket connection is closed or an error occurs.
    ///
    /// # Message Flow
    ///
    /// 1. WebSocket messages are received from the connection
    /// 2. Each message is parsed into an MCP message
    /// 3. Messages are yielded through the stream
    fn receive(&self) -> Pin<Box<dyn Stream<Item = Result<Message, Error>> + Send>> {
        let connection = self.connection.clone();
        
        let stream = futures::stream::unfold((), move |_| {
            let connection = connection.clone();
            async move {
                let mut guard = connection.lock().await;
                match guard.next().await {
                    Some(Ok(ws_message)) => {
                        let result = Self::parse_ws_message(ws_message);
                        Some((result, ()))
                    }
                    Some(Err(e)) => {
                        Some((Err(Error::Other(format!("WebSocket error: {}", e))), ()))
                    }
                    None => None,
                }
            }
        });

        Box::pin(stream)
    }

    /// Closes the WebSocket connection.
    ///
    /// This method will attempt to perform a clean shutdown of the WebSocket connection
    /// by sending a close frame and waiting for the connection to close.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the connection was successfully closed,
    /// or an error if closing failed.
    async fn close(&self) -> Result<(), Error> {
        let mut connection = self.connection.lock().await;
        connection
            .close(None)
            .await
            .map_err(|e| Error::Other(format!("Failed to close WebSocket connection: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::JSONRPC_VERSION;
    use serde_json::json;

    /// Tests message conversion between MCP and WebSocket formats:
    /// - Converting MCP message to WebSocket message
    /// - Parsing WebSocket message back to MCP message
    /// - Handling various message types and formats
    #[test]
    fn test_message_conversion() {
        // Test basic notification
        let basic_message = Message::Notification(Notification {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: "test".to_string(),
            params: Some(serde_json::Value::Null),
        });

        // Test JSON structure
        let json_str = serde_json::to_string(&basic_message).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(value["type"], "notification");
        assert_eq!(value["jsonrpc"], JSONRPC_VERSION);
        assert_eq!(value["method"], "test");
        assert_eq!(value["params"], serde_json::Value::Null);

        // Test complex notification with JSON object
        let complex_message = Message::Notification(Notification {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: "test".to_string(),
            params: Some(json!({
                "key": "value",
                "number": 42
            })),
        });

        // Test WebSocket conversion
        let ws_message = WebSocketTransport::convert_to_ws_message(&complex_message).unwrap();
        let text = match &ws_message {
            WsMessage::Text(text) => {
                let value: serde_json::Value = serde_json::from_str(text).unwrap();
                assert_eq!(value["type"], "notification");
                assert_eq!(value["jsonrpc"], JSONRPC_VERSION);
                assert_eq!(value["method"], "test");
                assert!(value["params"].is_object());
                assert_eq!(value["params"]["key"], "value");
                assert_eq!(value["params"]["number"], 42);
                text
            }
            _ => panic!("Expected Text message"),
        };

        // Test conversion back from WebSocket message
        let parsed_message = WebSocketTransport::parse_ws_message(WsMessage::Text(text.clone())).unwrap();
        match parsed_message {
            Message::Notification(n) => {
                assert_eq!(n.jsonrpc, JSONRPC_VERSION);
                assert_eq!(n.method, "test");
                assert!(n.params.is_some());
                if let Some(params) = n.params {
                    assert_eq!(params["key"], "value");
                    assert_eq!(params["number"], 42);
                }
            }
            _ => panic!("Expected Notification"),
        }
    }

    /// Tests handling of invalid WebSocket messages:
    /// - Binary messages (unsupported)
    /// - Invalid JSON
    /// - Ping/Pong messages
    /// - Close frames
    #[test]
    fn test_invalid_messages() {
        // Test invalid JSON
        let invalid_json = WsMessage::Text("not valid json".to_string());
        assert!(matches!(
            WebSocketTransport::parse_ws_message(invalid_json),
            Err(Error::Serialization(_))
        ));

        // Test unsupported message types
        let binary = WsMessage::Binary(vec![1, 2, 3]);
        assert!(matches!(
            WebSocketTransport::parse_ws_message(binary),
            Err(Error::Protocol { code: ErrorCode::InvalidRequest, .. })
        ));

        let ping = WsMessage::Ping(vec![]);
        assert!(matches!(
            WebSocketTransport::parse_ws_message(ping),
            Err(Error::Protocol { code: ErrorCode::InvalidRequest, .. })
        ));

        let close = WsMessage::Close(None);
        assert!(matches!(
            WebSocketTransport::parse_ws_message(close),
            Err(Error::Protocol { code: ErrorCode::InvalidRequest, .. })
        ));
    }
}
