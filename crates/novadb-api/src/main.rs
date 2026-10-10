mod account;
mod auth;
mod error;
mod models;
mod nova;
mod routes;
mod state;

use axum::routing::{get, post};
use axum::Router;
use sqlx::postgres::PgPoolOptions;

use crate::nova::NovaClient;
use crate::state::AppState;

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://youruser@localhost/novadb".to_string());
    let nova_addr = std::env::var("NOVADB_ADDR").unwrap_or_else(|_| "127.0.0.1:6379".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("failed to connect to Postgres");

    let state = AppState {
        pool,
        sessions: NovaClient::new(nova_addr),
    };

    let app = Router::new()
        .route("/signup", post(account::signup))
        .route("/login", post(account::login))
        .route("/logout", post(account::logout))
        .route("/me", get(account::me))
        .route(
            "/listings",
            get(routes::list_listings).post(routes::create_listing),
        )
        .route("/listings/:id", get(routes::get_listing))
        .with_state(state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("novadb-api listening on http://{addr}");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("server error");
}
