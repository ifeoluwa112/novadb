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
    pub seller_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub price_cents: i32,
}
