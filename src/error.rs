use std::fmt;
use thiserror::Error;

/// MCP error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,
    ServerNotInitialized = -32002,
    UnknownErrorCode = -32001,
}

/// Main error type for MCP operations
#[derive(Error, Debug, Clone)]
pub enum Error {
    #[error("MCP protocol error: {code:?} - {message}")]
    Protocol {
        code: ErrorCode,
        message: String,
        data: Option<serde_json::Value>,
    },

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Other error: {0}")]
    Other(String),
}

impl Error {
    pub fn protocol(code: ErrorCode, message: impl Into<String>) -> Self {
        Error::Protocol {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(self, data: serde_json::Value) -> Self {
        match self {
            Error::Protocol {
                code,
                message,
                data: _,
            } => Error::Protocol {
                code,
                message,
                data: Some(data),
            },
            _ => self,
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err.to_string())
    }
}

impl From<ErrorCode> for i32 {
    fn from(code: ErrorCode) -> Self {
        code as i32
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
