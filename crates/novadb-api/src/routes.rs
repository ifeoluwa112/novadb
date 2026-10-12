use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::models::{Listing, ListingDetail, ListingsPage, ListingsQuery, NewListing};
use crate::state::AppState;

const DEFAULT_PAGE_SIZE: i64 = 20;
const MAX_PAGE_SIZE: i64 = 50;
const MAX_TITLE_LEN: usize = 200;
const MAX_DESCRIPTION_LEN: usize = 5_000;
const MAX_PRICE_CENTS: i32 = 100_000_000; // $1,000,000

/// A cursor is "<created_at as microseconds>_<id>": a bookmark pointing at the
/// last row of the previous page. Microseconds match Postgres' timestamp
/// precision exactly, so no row can be skipped or repeated by rounding.
fn encode_cursor(created_at: DateTime<Utc>, id: Uuid) -> String {
    format!("{}_{}", created_at.timestamp_micros(), id).to_string()
}

fn decode_cursor(cursor: &str) -> Result<(DateTime<Utc>, Uuid), ApiError> {
    let invalid = || ApiError::BadRequest("invalid cursor".into());

    let (micros, id) = cursor.split_once('_').ok_or_else(invalid)?;
    let micros: i64 = micros.parse().map_err(|_| invalid())?;
    let created_at = DateTime::<Utc>::from_timestamp_micros(micros).ok_or_else(invalid)?;
    let id = Uuid::parse_str(id).map_err(|_| invalid())?;

    Ok((created_at, id))
}

/// Public: one page of active listings, newest first.
pub async fn list_listings(
    State(state): State<AppState>,
    Query(params): Query<ListingsQuery>,
) -> Result<Json<ListingsPage>, ApiError> {
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE);

    // Ask for one extra row. If it exists there is a next page; we never show it.
    let mut rows = match params.cursor {
        None => {
            sqlx::query_as::<_, Listing>(
                "SELECT id, title, description, price_cents, status, created_at
                 FROM listings
                 WHERE status = 'active'
                 ORDER BY created_at DESC, id DESC
                 LIMIT $1",
            )
            .bind(limit + 1)
            .fetch_all(&state.pool)
            .await?
        }
        Some(cursor) => {
            let (created_at, id) = decode_cursor(&cursor)?;
            sqlx::query_as::<_, Listing>(
                "SELECT id, title, description, price_cents, status, created_at
                 FROM listings
                 WHERE status = 'active' AND (created_at, id) < ($1, $2)
                 ORDER BY created_at DESC, id DESC
                 LIMIT $3",
            )
            .bind(created_at)
            .bind(id)
            .bind(limit + 1)
            .fetch_all(&state.pool)
            .await?
        }
    };

    let next_cursor = if rows.len() as i64 > limit {
        rows.truncate(limit as usize);
        rows.last().map(|last| encode_cursor(last.created_at, last.id))
    } else {
        None
    };

    Ok(Json(ListingsPage {
        items: rows,
        next_cursor,
    }))
}

/// Public: one listing with its seller's name.
pub async fn get_listing(
    State(state): State<AppState>,
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
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(listing))
}

/// Protected: the seller is whoever is logged in.
pub async fn create_listing(
    State(state): State<AppState>,
    user: AuthUser,
    Json(payload): Json<NewListing>,
) -> Result<(StatusCode, Json<Listing>), ApiError> {
    let title = payload.title.trim();
    if title.is_empty() || title.chars().count() > MAX_TITLE_LEN {
        return Err(ApiError::BadRequest(format!(
            "title must be 1-{MAX_TITLE_LEN} characters"
        )));
    }

    let description = payload
        .description
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty());
    if let Some(description) = description {
        if description.chars().count() > MAX_DESCRIPTION_LEN {
            return Err(ApiError::BadRequest(format!(
                "description must be at most {MAX_DESCRIPTION_LEN} characters"
            )));
        }
    }

    if payload.price_cents < 0 || payload.price_cents > MAX_PRICE_CENTS {
        return Err(ApiError::BadRequest(
            "price_cents is out of the allowed range".into(),
        ));
    }

    let listing = sqlx::query_as::<_, Listing>(
        "INSERT INTO listings (seller_id, title, description, price_cents)
         VALUES ($1, $2, $3, $4)
         RETURNING id, title, description, price_cents, status, created_at",
    )
    .bind(user.user_id)
    .bind(title)
    .bind(description)
    .bind(payload.price_cents)
    .fetch_one(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(listing)))
}