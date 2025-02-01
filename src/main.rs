use aws::persistance::{
    initialize_distributed_filter_system, run_distributed_snapshot_service,
    DistributedFilterPersistence,
};
use axum::{
    extract::{Path, State},
    response::Redirect,
    routing::{get, post},
    Json, Router,
};
use distributed_filter::DistributedFilter;
use errors::AppError;
use models::{CreateUrl, UrlResponse};
use redis::RedisManager;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::trace::TraceLayer;
mod aws;
mod cron;
mod distributed_filter;
mod errors;
mod models;
mod redis;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    distributed_filter: Arc<Mutex<DistributedFilter>>,
    redis: RedisManager,
}

async fn create_short_url(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateUrl>,
) -> Result<Json<UrlResponse>, AppError> {
    let short_code = match payload.custom_short_code {
        Some(custom) => custom,
        None => nanoid::nanoid!(8),
    };
    let current_date = chrono::Utc::now();
    let expiry_date_str = current_date
        .checked_add_months(chrono::Months::new(payload.months_valid.unwrap_or(1)))
        .unwrap()
        .format("%Y-%m-%d")
        .to_string();
    let format = time::macros::format_description!("[year]-[month]-[day]");

    let expiry_date = sqlx::types::time::Date::parse(&expiry_date_str, format).unwrap();

    let expiry_date_str2 = current_date
        .checked_add_months(chrono::Months::new(payload.months_valid.unwrap_or(1) + 1))
        .unwrap()
        .format("%Y-%m-%d")
        .to_string();

    let end_month = sqlx::types::time::Date::parse(&expiry_date_str2, format)
        .unwrap()
        .replace_day(1)
        .unwrap();

    let partition_name = format!(
        "urls_y{}m{:02}",
        expiry_date.year(),
        current_date
            .date_naive()
            .checked_add_months(chrono::Months::new(payload.months_valid.unwrap_or(1)))
            .unwrap()
            .format("%m")
    );

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

    if state
        .distributed_filter
        .lock()
        .await
        .contains(&short_code, current_date.date_naive())
    {
        return Err(AppError::NotFound);
    }

    sqlx::query!(
        "INSERT INTO urls (short_code, long_url, expiry_date)
    VALUES ($1, $2, $3::date)",
        short_code,
        payload.long_url,
        expiry_date,
    )
    .execute(&state.pool)
    .await?;

    let insertion = state.distributed_filter.lock().await.insert(
        &short_code,
        current_date
            .checked_add_months(chrono::Months::new(payload.months_valid.unwrap_or(1)))
            .unwrap()
            .date_naive(),
    );

    tracing::info!("Insertion: {:#?}", insertion);

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
    if !state
        .distributed_filter
        .lock()
        .await
        .contains(&short_code, chrono::Utc::now().date_naive())
    {
        tracing::info!("Short code not found in filter");
        return Err(AppError::NotFound);
    }

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = PgPool::connect(&database_url).await?;

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    // Initialize distributed filter system
    let distributed_filter = initialize_distributed_filter_system(
        &pool,
        "affinitys3".to_string(),
        "distributed-filter-backups".to_string(),
    )
    .await?;

    // Initialize the persistence service
    let filter_persistence = Arc::new(
        DistributedFilterPersistence::new(
            "affinitys3".to_string(),
            "distributed-filter-backups".to_string(),
        )
        .await?,
    );
    tracing::info!("Connecting to redis server...");
    let redis_urls: Vec<String> = vec![
        "redis://localhost:6379".into(), // If using minikube tunnel
                                         // OR use the external IP if you're on a cloud provider
                                         // "redis://<EXTERNAL-IP>:6379".into()
    ];

    let redis_manager = redis::RedisManager::new(redis_urls).await?;

    let app_state = Arc::new(AppState {
        pool: pool.clone(),
        distributed_filter: distributed_filter.clone(),
        redis: redis_manager,
    });

    // Schedule cleanup task
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(chrono::Duration::days(1).to_std().unwrap());
        loop {
            interval.tick().await;
            if let Err(e) = cron::cleanup_expired_partitions(&pool).await {
                eprintln!("Cleanup error: {}", e);
            }
        }
    });

    // tokio::spawn(async move {
    //     run_distributed_snapshot_service(distributed_filter, filter_persistence)
    //         .await;
    // });

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
