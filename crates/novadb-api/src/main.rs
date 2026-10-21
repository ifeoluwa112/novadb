mod account;
mod auth;
mod error;
mod health;
mod models;
mod nova;
mod routes;
mod state;

use std::sync::Arc;
use std::time::Duration;

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::Semaphore;

use crate::nova::NovaClient;
use crate::state::AppState;

fn env_parse<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set, e.g. postgres://youruser@localhost/novadb");
    let nova_addr = std::env::var("NOVADB_ADDR").unwrap_or_else(|_| "127.0.0.1:6379".to_string());
    let port: u16 = env_parse("PORT", 3000);
    let db_max_connections: u32 = env_parse("DB_MAX_CONNECTIONS", 10);

    let pool = PgPoolOptions::new()
        .max_connections(db_max_connections)
        .min_connections(2)
        // Fail fast under overload instead of making users wait 30 seconds.
        .acquire_timeout(Duration::from_secs(3))
        .connect(&database_url)
        .await
        .expect("failed to connect to Postgres");

    let hash_permits = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        .max(2);
    let dummy_hash =
        auth::hash_blocking("not-a-real-password").expect("failed to build dummy hash");

    let state = AppState {
        pool,
        sessions: NovaClient::new(nova_addr),
        hash_limiter: Arc::new(Semaphore::new(hash_permits)),
        dummy_hash: Arc::new(dummy_hash),
    };

    let app = Router::new()
        .route("/healthz", get(health::healthz))
        .route("/readyz", get(health::readyz))
        .route("/signup", post(account::signup))
        .route("/login", post(account::login))
        .route("/logout", post(account::logout))
        .route("/me", get(account::me))
        .route(
            "/listings",
            get(routes::list_listings).post(routes::create_listing),
        )
        .route(
            "/listings/:id",
            get(routes::get_listing).patch(routes::update_listing).delete(routes::delete_listing),
        )
        // Nothing legitimate here is bigger than 64 KB of JSON.
        .layer(DefaultBodyLimit::max(64 * 1024))
        .with_state(state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    println!("novadb-api listening on http://{addr}");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");

    println!("novadb-api stopped cleanly");
}

/// Resolves when the process is asked to stop (Ctrl+C, or SIGTERM from a
/// deploy system). Axum then stops accepting new connections and lets
/// in-flight requests finish before `serve` returns.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("shutdown signal received, draining in-flight requests...");
}
