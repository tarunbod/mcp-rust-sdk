use serde::{Deserialize, Serialize};
use std::fmt;

use crate::error::{Error, ErrorCode};

pub const LATEST_PROTOCOL_VERSION: &str = "2024-11-05";
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[LATEST_PROTOCOL_VERSION, "2024-10-07"];
pub const JSONRPC_VERSION: &str = "2.0";

/// A unique identifier for a request
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    String(String),
    Number(i64),
}

/// Base JSON-RPC request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    pub id: RequestId,
}

/// Base JSON-RPC notification structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// Base JSON-RPC response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: RequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ResponseError>,
}

/// JSON-RPC error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl Request {
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>, id: RequestId) -> Self {
        Self {
            jsonrpc: crate::JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
            id,
        }
    }
}

impl Notification {
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: crate::JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
        }
    }
}

impl Response {
    pub fn success(id: RequestId, result: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: crate::JSONRPC_VERSION.to_string(),
            id,
            result,
            error: None,
        }
    }

    pub fn error(id: RequestId, error: ResponseError) -> Self {
        Self {
            jsonrpc: crate::JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

impl From<Error> for ResponseError {
    fn from(err: Error) -> Self {
        match err {
            Error::Protocol {
                code,
                message,
                data,
            } => ResponseError {
                code: code.into(),
                message,
                data,
            },
            Error::Transport(msg) => ResponseError {
                code: ErrorCode::InternalError.into(),
                message: format!("Transport error: {}", msg),
                data: None,
            },
            Error::Serialization(err) => ResponseError {
                code: ErrorCode::ParseError.into(),
                message: err.to_string(),
                data: None,
            },
            Error::Io(err) => ResponseError {
                code: ErrorCode::InternalError.into(),
                message: err.to_string(),
                data: None,
            },
            Error::Other(msg) => ResponseError {
                code: ErrorCode::InternalError.into(),
                message: msg,
                data: None,
            },
        }
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestId::String(s) => write!(f, "{}", s),
            RequestId::Number(n) => write!(f, "{}", n),
        }
    }
}
