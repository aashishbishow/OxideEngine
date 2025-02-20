use axum::{
    routing::{get, Router},
    response::IntoResponse,
    http::StatusCode,
    Server,
};
use tower_http::{
    trace::TraceLayer,
    cors::CorsLayer,
    limit::RequestBodyLimitLayer,
};
use tower::ServiceBuilder;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Clone)]
struct AppState {
    db_pool: sqlx::PgPool,
    cache: redis::Client,
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "Healthy")
}

async fn root_handler() -> impl IntoResponse {
    (StatusCode::OK, "OxideEngine Running")
}

async fn error_handler() -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Server Error")
}

pub async fn start_server(
    database_url: &str, 
    redis_url: &str,
    server_addr: &str
) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging and tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Database connection
    let db_pool = sqlx::PgPool::connect(database_url).await?;

    // Redis connection
    let redis_client = redis::Client::open(redis_url)?;

    // Application state
    let app_state = AppState {
        db_pool,
        cache: redis_client,
    };

    // Middleware
    let middleware_stack = ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .layer(RequestBodyLimitLayer::new(1024 * 1024)) // 1MB limit
        .timeout(Duration::from_secs(10));

    // Routes
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/health", get(health_check))
        .route("/error", get(error_handler))
        .with_state(app_state)
        .layer(middleware_stack);

    // Parse socket address
    let addr: SocketAddr = server_addr.parse()?;

    // Start server
    tracing::info!("Server listening on {}", addr);
    Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok(); // Load .env file
    
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let redis_url = std::env::var("REDIS_URL")
        .expect("REDIS_URL must be set");
    
    start_server(&database_url, &redis_url, "0.0.0.0:3000").await
}