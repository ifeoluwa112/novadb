use sqlx::PgPool;

use crate::nova::NovaClient;

/// Cloning is cheap: PgPool is a shared handle, and NovaClient is one String.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub sessions: NovaClient,
}