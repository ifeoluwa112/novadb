use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub enum ApiError {
    NotFound,
    BadRequest(String),
    Database(sqlx::Error),
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        if matches!(error, sqlx::Error::RowNotFound) {
            return ApiError::NotFound;
        }

        // Postgres reports constraint failures with a short error code.
        // Pull it out as an owned String first, so we stop borrowing `error`.
        let code = match &error {
            sqlx::Error::Database(db_error) => db_error.code().map(|c| c.into_owned()),
            _ => None,
        };

        match code.as_deref() {
            // 23503 = foreign key violation (e.g. seller_id doesn't exist)
            Some("23503") => ApiError::BadRequest("referenced record does not exist".into()),
            // 23514 = CHECK constraint violation (e.g. negative price)
            Some("23514") => ApiError::BadRequest("a value broke a database rule".into()),
            _ => ApiError::Database(error),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, "listing not found".to_string()),
            ApiError::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
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
