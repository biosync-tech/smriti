use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::errors::AppError;

/// Uniform envelope for all web UI API responses.
///
/// Success: `{ "data": T, "error": null }`
/// Failure: `{ "data": null, "error": { "code": "...", "message": "..." } }`
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T: Serialize> {
    pub data: Option<T>,
    pub error: Option<ApiErrorBody>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub code: String,
    pub message: String,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            data: Some(data),
            error: None,
        }
    }

    pub fn err(code: impl Into<String>, message: impl Into<String>) -> ApiResponse<()> {
        ApiResponse {
            data: None,
            error: Some(ApiErrorBody {
                code: code.into(),
                message: message.into(),
            }),
        }
    }
}

/// Axum extractor-compatible error type that serialises to ApiResponse envelope.
pub struct WebError(pub AppError);

impl From<AppError> for WebError {
    fn from(e: AppError) -> Self {
        Self(e)
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        let (status, code) = match &self.0 {
            AppError::NoteNotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            AppError::AgentNotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            AppError::TagNotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            AppError::Conflict(_) => (StatusCode::CONFLICT, "CONFLICT"),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            AppError::InvalidWikiLink(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
        };
        let body = ApiResponse::<()>::err(code, self.0.to_string());
        (status, Json(body)).into_response()
    }
}

/// Convenience alias used in web handlers.
pub type WebResult<T> = Result<Json<ApiResponse<T>>, WebError>;

/// Wrap a successful value in the standard envelope.
pub fn ok<T: Serialize>(data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse::ok(data))
}
