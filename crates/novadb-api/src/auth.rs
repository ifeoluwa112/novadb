use std::sync::Arc;
use std::time::Duration;

use argon2::password_hash::{
    rand_core::{OsRng, RngCore},
    PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::{header::AUTHORIZATION, request::Parts};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

pub const SESSION_TTL_SECS: u64 = 60 * 60;
const HASH_QUEUE_TIMEOUT: Duration = Duration::from_secs(3);

pub fn hash_blocking(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
}

async fn acquire_hash_slot(limiter: &Arc<Semaphore>) -> Result<OwnedSemaphorePermit, ApiError> {
    match tokio::time::timeout(HASH_QUEUE_TIMEOUT, limiter.clone().acquire_owned()).await {
        Ok(Ok(permit)) => Ok(permit),
        Ok(Err(_closed)) => Err(ApiError::Internal),
        Err(_waited_too_long) => Err(ApiError::Unavailable),
    }
}

pub async fn hash_password(limiter: &Arc<Semaphore>, password: String) -> Result<String, ApiError> {
    let permit = acquire_hash_slot(limiter).await?;

    let result = tokio::task::spawn_blocking(move || {
        // The permit moves into the worker, so it is released only when the
        // hashing really finishes, even if the client disconnects meanwhile.
        let _permit = permit;
        hash_blocking(&password)
    })
    .await
    .map_err(|_| ApiError::Internal)?;

    result.map_err(|error| {
        eprintln!("password hashing failed: {error}");
        ApiError::Internal
    })
}

pub async fn verify_password(
    limiter: &Arc<Semaphore>,
    password: String,
    stored_hash: String,
) -> Result<bool, ApiError> {
    let permit = acquire_hash_slot(limiter).await?;

    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        match PasswordHash::new(&stored_hash) {
            Ok(parsed) => Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok(),
            Err(_) => false,
        }
    })
    .await
    .map_err(|_| ApiError::Internal)
}

pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn session_key(token: &str) -> String {
    return format!("session:{token}");
}

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
            .ok_or(ApiError::Unauthorized)?;

        // Our tokens are always exactly 64 hex characters. Anything else is
        // junk, and rejecting it here keeps garbage traffic off the session store.
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ApiError::Unauthorized);
        }

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
