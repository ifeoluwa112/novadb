use argon2::password_hash::{
    rand_core::{OsRng, RngCore},
    PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::{header::AUTHORIZATION, request::Parts};
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

pub const SESSION_TTL_SECS: u64 = 60 * 60; // one hour

/// Argon2 is deliberately slow and CPU-heavy. Running it directly inside an
/// async handler would freeze a tokio worker thread, so it goes to
/// `spawn_blocking`, a separate pool of threads meant for heavy work.
pub async fn hash_password(password: String) -> Result<String, ApiError> {
    let hashed = tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
    })
    .await
    .map_err(|_| ApiError::Internal)?;

    hashed.map_err(|error| {
        eprintln!("password hashing failed: {error}");
        ApiError::Internal
    })
}

pub async fn verify_password(password: String, stored_hash: String) -> Result<bool, ApiError> {
    tokio::task::spawn_blocking(move || match PasswordHash::new(&stored_hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        // A malformed stored hash can never match any password.
        Err(_) => false,
    })
    .await
    .map_err(|_| ApiError::Internal)
}

/// 32 random bytes from the operating system's secure random source,
/// written as 64 hex characters. Unguessable and unrelated to the user.
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn session_key(token: &str) -> String {
    format!("session:{token}").to_string()
}

/// The card reader. Any handler that lists `user: AuthUser` among its
/// parameters is automatically protected: axum builds this value before the
/// handler runs, and if building it fails the handler never executes.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub token: String,
}

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(ApiError::Unauthorized)?;

        let token = header
            .strip_prefix("Bearer ")
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .ok_or(ApiError::Unauthorized)?;

        // If NovaDB is unreachable this returns a 500 (we cannot verify the
        // card), not a 401. Failing closed: nobody gets in unverified.
        let stored = state.sessions.get(&session_key(token)).await?;

        let user_id = stored
            .and_then(|value| Uuid::parse_str(&value).ok())
            .ok_or(ApiError::Unauthorized)?;

        Ok(AuthUser {
            user_id,
            token: token.to_string(),
        })
    }
}
