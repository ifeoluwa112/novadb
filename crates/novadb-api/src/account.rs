use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::ApiError;
use crate::models::{LoginRequest, LoginResponse, SignupRequest, UserProfile};
use crate::state::AppState;

const MIN_PASSWORD_LEN: usize = 8;
const MAX_PASSWORD_LEN: usize = 128;
const MAX_EMAIL_LEN: usize = 254;
const MAX_DISPLAY_NAME_LEN: usize = 60;

pub async fn signup(
    State(state): State<AppState>,
    Json(payload): Json<SignupRequest>,
) -> Result<(StatusCode, Json<UserProfile>), ApiError> {
    let email = payload.email.trim().to_lowercase();
    let display_name = payload.display_name.trim().to_string();

    if email.len() < 3 || email.len() > MAX_EMAIL_LEN || !email.contains('@') {
        return Err(ApiError::BadRequest("a valid email is required".into()));
    }
    if display_name.is_empty() || display_name.chars().count() > MAX_DISPLAY_NAME_LEN {
        return Err(ApiError::BadRequest(format!(
            "display_name must be 1-{MAX_DISPLAY_NAME_LEN} characters"
        )));
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

    let password_hash = auth::hash_password(&state.hash_limiter, payload.password).await?;

    let user = sqlx::query_as::<_, UserProfile>(
        "INSERT INTO users (email, password_hash, display_name)
         VALUES ($1, $2, $3)
         RETURNING id, email, display_name, created_at",
    )
    .bind(&email)
    .bind(password_hash)
    .bind(&display_name)
    .fetch_one(&state.pool)
    .await?;

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

    let Some((user_id, stored_hash)) = row else {
        // Unknown email: do the same expensive work a real check would do,
        // so the response time doesn't reveal which emails exist.
        let _ = auth::verify_password(
            &state.hash_limiter,
            payload.password,
            state.dummy_hash.to_string(),
        )
        .await?;
        return Err(ApiError::InvalidCredentials);
    };

    if !auth::verify_password(&state.hash_limiter, payload.password, stored_hash).await? {
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