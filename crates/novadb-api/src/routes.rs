use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::models::{
    Listing, ListingDetail, ListingsPage, ListingsQuery, NewListing, UpdateListing,
};
use crate::state::AppState;
use sqlx::PgPool;

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
    let limit = params
        .limit
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);

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
        rows.last()
            .map(|last| encode_cursor(last.created_at, last.id))
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

fn clean_title(raw: &str) -> Result<String, ApiError> {
    let title = raw.trim();
    if title.is_empty() || title.chars().count() > MAX_TITLE_LEN {
        return Err(ApiError::BadRequest(format!(
            "title must be 1-{MAX_TITLE_LEN} characters"
        )));
    }
    Ok(title.to_string())
}

/// Returns None when the description is blank after trimming.
fn clean_description(raw: &str) -> Result<Option<String>, ApiError> {
    let description = raw.trim();
    if description.is_empty() {
        return Ok(None);
    }
    if description.chars().count() > MAX_DESCRIPTION_LEN {
        return Err(ApiError::BadRequest(format!(
            "description must be at most {MAX_DESCRIPTION_LEN} characters"
        )));
    }
    Ok(Some(description.to_string()))
}

fn check_price(price_cents: i32) -> Result<i32, ApiError> {
    if !(0..=MAX_PRICE_CENTS).contains(&price_cents) {
        return Err(ApiError::BadRequest(
            "price_cents is out of the allowed range".into(),
        ));
    }
    Ok(price_cents)
}

/// Protected: the seller is whoever is logged in.
pub async fn create_listing(
    State(state): State<AppState>,
    user: AuthUser,
    Json(payload): Json<NewListing>,
) -> Result<(StatusCode, Json<Listing>), ApiError> {
    let title = clean_title(&payload.title)?;
    let description = match payload.description.as_deref() {
        Some(raw) => clean_description(raw)?,
        None => None,
    };
    let price_cents = check_price(payload.price_cents)?;

    let listing = sqlx::query_as::<_, Listing>(
        "INSERT INTO listings (seller_id, title, description, price_cents)
         VALUES ($1, $2, $3, $4)
         RETURNING id, title, description, price_cents, status, created_at",
    )
    .bind(user.user_id)
    .bind(title.as_str())
    .bind(description.as_deref())
    .bind(price_cents)
    .fetch_one(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(listing)))
}

/// Protected, owner only: change any subset of title, description, price.
pub async fn update_listing(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateListing>,
) -> Result<Json<Listing>, ApiError> {
    if payload.title.is_none() && payload.description.is_none() && payload.price_cents.is_none() {
        return Err(ApiError::BadRequest("nothing to update".into()));
    }

    let title = payload.title.as_deref().map(clean_title).transpose()?;
    let description = match payload.description.as_deref() {
        Some(raw) => Some(
            clean_description(raw)?
                .ok_or_else(|| ApiError::BadRequest("description must not be empty".into()))?,
        ),
        None => None,
    };
    let price_cents = payload.price_cents.map(check_price).transpose()?;

    // The ownership and status checks live inside the UPDATE itself, so
    // "check, then change" happens as ONE atomic statement. A separate
    // SELECT-then-UPDATE would leave a gap where the listing could be sold
    // between the two steps: the same read-then-write gap as the lost update.
    let updated = sqlx::query_as::<_, Listing>(
        "UPDATE listings
         SET title       = COALESCE($3::text, title),
             description = COALESCE($4::text, description),
             price_cents = COALESCE($5::int4, price_cents)
         WHERE id = $1 AND seller_id = $2 AND status = 'active'
         RETURNING id, title, description, price_cents, status, created_at",
    )
    .bind(id)
    .bind(user.user_id)
    .bind(title.as_deref())
    .bind(description.as_deref())
    .bind(price_cents)
    .fetch_optional(&state.pool)
    .await?;

    match updated {
        Some(listing) => Ok(Json(listing)),
        None => Err(why_not_changed(&state.pool, id, user.user_id).await),
    }
}

// Delete a listing that is still active. The row must exist, be owned by the
// caller, and not be sold yet. The DELETE returns the row if it was actually
// deleted, or nothing if it was not. We then check why it was not deleted,
// so the client gets an honest answer. This is a single atomic statement, so
// there is no race condition where the listing could be sold between a
// separate SELECT and DELETE.

pub async fn delete_listing(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let deleted = sqlx::query(
        "DELETE FROM listings
         WHERE id = $1 AND seller_id = $2 AND status = 'active'
         RETURNING id",
    )
    .bind(id)
    .bind(user.user_id)
    .fetch_optional(&state.pool)
    .await?;

    match deleted {
        Some(_) => Ok(StatusCode::NO_CONTENT),
        None => Err(why_not_changed(&state.pool, id, user.user_id).await),
    }
}

/// Zero rows changed. Work out why, so the client gets an honest answer.
/// This lookup only chooses the error message. It can't affect correctness,
/// because the UPDATE above already refused to touch the row.
async fn why_not_changed(pool: &PgPool, id: Uuid, user_id: Uuid) -> ApiError {
    let row =
        sqlx::query_as::<_, (Uuid, String)>("SELECT seller_id, status FROM listings WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await;

    match row {
        Ok(None) => ApiError::NotFound,
        Ok(Some((seller_id, _))) if seller_id != user_id => ApiError::Forbidden,
        Ok(Some(_)) => ApiError::Conflict("listing can no longer be changed".into()),
        Err(error) => error.into(),
    }
}
