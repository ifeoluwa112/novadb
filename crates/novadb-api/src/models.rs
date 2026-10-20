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

#[derive(Debug, Deserialize)]
pub struct NewListing {
    pub title: String,
    pub description: Option<String>,
    pub price_cents: i32,
}

#[derive(Debug, Deserialize)]
pub struct ListingsQuery {
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListingsPage {
    pub items: Vec<Listing>,
    /// Pass this back as `?cursor=` to get the next page. Absent on the last page.
    pub next_cursor: Option<String>,
}

// No Debug on credential-carrying types, so a stray `{:?}` can't log a password.
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

#[derive(Debug, Deserialize)]
pub struct UpdateListing {
    pub title: Option<String>,
    pub description: Option<String>,
    pub price_cents: Option<i32>,
}
