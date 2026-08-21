use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mcp::server::McpServer;
use crate::web::AppState;

/// JSON-RPC 2.0 request envelope.
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// JSON-RPC 2.0 response envelope.
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

/// POST /mcp — MCP JSON-RPC 2.0 over HTTP.
///
/// Delegates to `McpServer::dispatch_http` so both the stdio and HTTP
/// transports use identical tool routing — no duplicate logic.
pub async fn handle_mcp(
    State(state): State<AppState>,
    body: Result<Json<JsonRpcRequest>, axum::extract::rejection::JsonRejection>,
) -> (StatusCode, Json<JsonRpcResponse>) {
    let req = match body {
        Ok(Json(r)) => r,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: Value::Null,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                    }),
                }),
            );
        }
    };

    let id = req.id.clone().unwrap_or(Value::Null);
    let mcp = McpServer::new(state.db.clone());
    let response = match mcp.dispatch_http(&req.method, req.params) {
        Ok(value) => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: Some(value),
            error: None,
        },
        Err((code, message)) => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(JsonRpcError { code, message }),
        },
    };

    (StatusCode::OK, Json(response))
}
