use axum::extract::State;

use crate::error::ApiError;
use crate::state::AppState;

/// Liveness: "is the process running?" Checks nothing else, on purpose.
pub async fn healthz() -> &'static str {
    "ok"
}

/// Readiness: "can this instance actually serve traffic right now?"
pub async fn readyz(State(state): State<AppState>) -> Result<&'static str, ApiError> {
    sqlx::query("SELECT 1").execute(&state.pool).await?;
    state.sessions.get("__readyz__").await?;
    Ok("ready")
}