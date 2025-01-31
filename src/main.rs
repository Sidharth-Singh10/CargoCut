use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Redirect,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{types::chrono::Utc, PgPool};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

#[derive(Deserialize)]
struct CreateUrl {
    long_url: String,
    days_valid: Option<u32>, // Optional expiry days
}

#[derive(Serialize)]
struct UrlResponse {
    short_code: String,
    long_url: String,
    expiry_date: String,
}

#[derive(thiserror::Error, Debug)]
enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("URL not found or expired")]
    NotFound,
}

impl axum::response::IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        match self {
            AppError::Database(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database error: {}", err),
            )
                .into_response(),
            AppError::NotFound => StatusCode::NOT_FOUND.into_response(),
        }
    }
}

async fn create_short_url(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateUrl>,
) -> Result<Json<UrlResponse>, AppError> {
    let short_code = nanoid::nanoid!(8);
    let current_date = chrono::Utc::now();
    let expiry_date_str = current_date
        .checked_add_months(chrono::Months::new(payload.days_valid.unwrap_or(1)))
        .unwrap()
        .format("%Y-%m-%d")
        .to_string();
    let format = time::macros::format_description!("[year]-[month]-[day]");

    let expiry_date = sqlx::types::time::Date::parse(&expiry_date_str, format).unwrap();

    let expiry_date_str2 = current_date
        .checked_add_months(chrono::Months::new(payload.days_valid.unwrap_or(1)+1))
        .unwrap()
        .format("%Y-%m-%d")
        .to_string();

    let end_month = sqlx::types::time::Date::parse(&expiry_date_str2, format).unwrap().replace_day(1).unwrap();

    let partition_name = format!("urls_y{}m{:02}", expiry_date.year(), current_date.date_naive().checked_add_months(chrono::Months::new(payload.days_valid.unwrap_or(1))).unwrap().format("%m"));

    tracing::info!("Partition name: {}", partition_name);
    tracing::info!("End date: {}", end_month);
    tracing::info!("{}-{}-01", expiry_date.year(), expiry_date.month());

    let query = format!(
        "CREATE TABLE IF NOT EXISTS {} 
         PARTITION OF urls 
         FOR VALUES FROM ('{}') TO ('{}');",
        partition_name,
        format!("{}-{}-01", expiry_date.year(), expiry_date.month()),
        end_month
    );

    sqlx::query(&query).execute(&state.pool).await?;

    sqlx::query!(
        "INSERT INTO urls (short_code, long_url, expiry_date)
    VALUES ($1, $2, $3::date)",
        short_code,
        payload.long_url,
        expiry_date,
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(UrlResponse {
        short_code,
        long_url: payload.long_url,
        expiry_date: expiry_date.to_string(),
    }))
}

async fn redirect_to_long_url(
    State(state): State<Arc<AppState>>,
    Path(short_code): Path<String>,
) -> Result<Redirect, AppError> {
    let url = sqlx::query!(
        "SELECT long_url FROM urls 
         WHERE short_code = $1 
         AND expiry_date >= CURRENT_DATE",
        short_code
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Redirect::permanent(&url.long_url))
}

// Function to drop expired partitions
async fn cleanup_expired_partitions(pool: &PgPool) -> Result<(), sqlx::Error> {
    let expired_month = Utc::now()
        .date_naive()
        .checked_sub_months(chrono::Months::new(1))
        .unwrap();

    let partition_name = format!(
        "urls_y{}m{}",
        expired_month.format("%Y"),
        expired_month.format("%m")
    );

    sqlx::query(&format!("DROP TABLE IF EXISTS {}", partition_name))
        .execute(pool)
        .await?;

    Ok(())
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = PgPool::connect(&database_url).await?;

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    let app_state = Arc::new(AppState { pool: pool.clone() });

    // Schedule cleanup task
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(chrono::Duration::days(1).to_std().unwrap());
        loop {
            interval.tick().await;
            if let Err(e) = cleanup_expired_partitions(&pool).await {
                eprintln!("Cleanup error: {}", e);
            }
        }
    });

    let app = Router::new()
        .route("/api/urls", post(create_short_url))
        .route("/{short_code}", get(redirect_to_long_url))
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await?;
    println!("Server running on http://localhost:3001");
    axum::serve(listener, app).await?;

    Ok(())
}
