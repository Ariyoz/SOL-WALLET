// error.rs — Axum error type that produces clean JSON responses
//
// Internal details are logged but NEVER leaked to the client.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use wallet_core::WalletError;

#[derive(Debug)]
pub enum ApiError {
    Wallet(WalletError),
    NotFound(String),
    BadRequest(String),
    Internal(String),
    #[allow(dead_code)] // reserved for future rate-limiting middleware
    RateLimit,
}

impl From<WalletError> for ApiError {
    fn from(e: WalletError) -> Self {
        ApiError::Wallet(e)
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, user_message) = match &self {
            ApiError::Wallet(e) => {
                // Log the internal error.
                tracing::warn!("Wallet error: {}", e);
                let status = match e {
                    WalletError::InvalidAddress(_) => StatusCode::BAD_REQUEST,
                    WalletError::InvalidAmount(_) => StatusCode::BAD_REQUEST,
                    WalletError::InvalidMnemonic(_) => StatusCode::BAD_REQUEST,
                    WalletError::InsufficientFunds { .. } => StatusCode::UNPROCESSABLE_ENTITY,
                    WalletError::ConnectionFailed(_) => StatusCode::SERVICE_UNAVAILABLE,
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                };
                (status, e.user_message())
            }
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "An internal error occurred. Please try again.".into(),
                )
            }
            ApiError::RateLimit => (
                StatusCode::TOO_MANY_REQUESTS,
                "Too many requests. Please wait a moment and try again.".into(),
            ),
        };

        (
            status,
            Json(json!({
                "error": true,
                "message": user_message,
            })),
        )
            .into_response()
    }
}
