use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Listing {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub price_cents: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ListingDetail {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub price_cents: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub seller_name: String,
}

/// Note: no seller_id. The seller is whoever is logged in.
#[derive(Debug, Deserialize)]
pub struct NewListing {
    pub title: String,
    pub description: Option<String>,
    pub price_cents: i32,
}

// No `Debug` on these two on purpose: a stray `{:?}` in a log line
// would print the password.
#[derive(Deserialize)]
pub struct SignupRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// What the outside world may see of a user. Deliberately has no
/// password_hash field, so it can never be serialized by accident.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserProfile {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub expires_in_seconds: u64,
}