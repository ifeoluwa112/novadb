use axum::extract::{Path, State};
use axum::Json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;
use crate::models::{Listing, ListingDetail};

pub async fn list_listings(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<Listing>>, ApiError> {
    let listings = sqlx::query_as::<_, Listing>(
        "SELECT id, title, description, price_cents, status, created_at
         FROM listings
         WHERE status = 'active'
         ORDER BY created_at DESC",
    )
    .fetch_all(&pool)
    .await?;

    Ok(Json(listings))
}

pub async fn get_listing(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<ListingDetail>, ApiError> {
    let listing = sqlx::query_as::<_, ListingDetail>(
        "SELECT
             listings.id,
             listings.title,
             listings.description,
             listings.price_cents,
             listings.status,
             listings.created_at,
             users.display_name AS seller_name
         FROM listings
         JOIN users ON listings.seller_id = users.id
         WHERE listings.id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(listing))
}