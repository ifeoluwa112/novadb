use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use crate::nova::NovaError;

#[derive(Debug)]
pub enum ApiError {
    NotFound,
    BadRequest(String),
    Conflict(String),
    Unauthorized,
    InvalidCredentials,
    Internal,
    Database(sqlx::Error),
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        if matches!(error, sqlx::Error::RowNotFound) {
            return ApiError::NotFound;
        }

        let code = match &error {
            sqlx::Error::Database(db_error) => db_error.code().map(|c| c.into_owned()),
            _ => None,
        };

        match code.as_deref() {
            // 23503 = foreign key violation
            Some("23503") => ApiError::BadRequest("referenced record does not exist".into()),
            // 23514 = CHECK constraint violation
            Some("23514") => ApiError::BadRequest("a value broke a database rule".into()),
            // 23505 = unique violation (e.g. email already taken)
            Some("23505") => ApiError::Conflict("already registered".into()),
            _ => ApiError::Database(error),
        }
    }
}

impl From<NovaError> for ApiError {
    fn from(error: NovaError) -> Self {
        eprintln!("session store error: {error:?}");
        ApiError::Internal
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            ApiError::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            ApiError::Conflict(message) => (StatusCode::CONFLICT, message),
            ApiError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "authentication required".to_string(),
            ),
            ApiError::InvalidCredentials => (
                StatusCode::UNAUTHORIZED,
                "invalid email or password".to_string(),
            ),
            ApiError::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "something went wrong".to_string(),
            ),
            ApiError::Database(error) => {
                eprintln!("database error: {error:?}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "something went wrong".to_string(),
                )
            }
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}