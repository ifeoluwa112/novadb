use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::Semaphore;

use crate::nova::NovaClient;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub sessions: NovaClient,
    /// Caps how many Argon2 hashes run at once (each uses ~19 MiB and a full core).
    pub hash_limiter: Arc<Semaphore>,
    /// A real hash of a throwaway password, used to equalize login timing.
    pub dummy_hash: Arc<String>,
}