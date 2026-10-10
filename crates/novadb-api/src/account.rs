use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::ApiError;
use crate::models::{LoginRequest, LoginResponse, SignupRequest, UserProfile};
use crate::state::AppState;

const MIN_PASSWORD_LEN: usize = 8;
const MAX_PASSWORD_LEN: usize = 128; // stops someone making us hash a 10 MB "password"

pub async fn signup(
    State(state): State<AppState>,
    Json(payload): Json<SignupRequest>,
) -> Result<(StatusCode, Json<UserProfile>), ApiError> {
    // Normalize first, so "Zed@Mail.com" and "zed@mail.com" are the same account.
    let email = payload.email.trim().to_lowercase();
    let display_name = payload.display_name.trim().to_string();

    if email.len() < 3 || !email.contains('@') {
        return Err(ApiError::BadRequest("a valid email is required".into()));
    }
    if display_name.is_empty() {
        return Err(ApiError::BadRequest("display_name must not be empty".into()));
    }
    if payload.password.len() < MIN_PASSWORD_LEN {
        return Err(ApiError::BadRequest(format!(
            "password must be at least {MIN_PASSWORD_LEN} characters"
        )));
    }
    if payload.password.len() > MAX_PASSWORD_LEN {
        return Err(ApiError::BadRequest(format!(
            "password must be at most {MAX_PASSWORD_LEN} characters"
        )));
    }

    let password_hash = auth::hash_password(payload.password).await?;

    let user = sqlx::query_as::<_, UserProfile>(
        "INSERT INTO users (email, password_hash, display_name)
         VALUES ($1, $2, $3)
         RETURNING id, email, display_name, created_at",
    )
    .bind(&email)
    .bind(password_hash)
    .bind(&display_name)
    .fetch_one(&state.pool)
    .await?; // a duplicate email becomes a 409 via the 23505 mapping

    Ok((StatusCode::CREATED, Json(user)))
}

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let email = payload.email.trim().to_lowercase();

    if payload.password.len() > MAX_PASSWORD_LEN {
        return Err(ApiError::InvalidCredentials);
    }

    let row = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, password_hash FROM users WHERE email = $1",
    )
    .bind(&email)
    .fetch_optional(&state.pool)
    .await?;

    // Unknown email and wrong password produce the *same* error, so an
    // attacker cannot use this endpoint to discover which emails exist.
    let (user_id, stored_hash) = row.ok_or(ApiError::InvalidCredentials)?;

    if !auth::verify_password(payload.password, stored_hash).await? {
        return Err(ApiError::InvalidCredentials);
    }

    let token = auth::generate_token();
    state
        .sessions
        .set_with_ttl(
            &auth::session_key(&token),
            &user_id.to_string(),
            auth::SESSION_TTL_SECS,
        )
        .await?;

    Ok(Json(LoginResponse {
        token,
        expires_in_seconds: auth::SESSION_TTL_SECS,
    }))
}

pub async fn logout(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<StatusCode, ApiError> {
    state.sessions.delete(&auth::session_key(&user.token)).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn me(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<UserProfile>, ApiError> {
    let profile = sqlx::query_as::<_, UserProfile>(
        "SELECT id, email, display_name, created_at FROM users WHERE id = $1",
    )
    .bind(user.user_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(profile))
}