mod error;
mod models;
mod routes;

use axum::routing::get;
use axum::Router;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set, e.g. postgres://user@localhost/novadb");

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("failed to connect to Postgres");

    let app = Router::new()
        .route("/listings", get(routes::list_listings))
        .route("/listings/:id", get(routes::get_listing))
        .with_state(pool);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("novadb-api listening on http://{addr}");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("server error");
}