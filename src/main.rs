use aws::persistance::initialize_distributed_filter_system;
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use distributed_filter::DistributedFilter;
use errors::AppError;
use metrics::{CPU_USAGE, MEMORY_USAGE, REQUEST_COUNTER, REQUEST_DURATION};
use models::{CreateUrl, UrlResponse};
use prometheus::{Encoder, TextEncoder};
use redis::RedisManager;
use sqlx::{migrate::Migrator, PgPool};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::trace::TraceLayer;
mod aws;
mod cron;
mod distributed_filter;
mod errors;
mod metrics;
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
    // metrics
    let start_metric = tokio::time::Instant::now();
    REQUEST_COUNTER.inc();
    //

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
    /////////////////////////
    // state
    //     .redis
    //     .set_short_url(&short_code, &payload.long_url)
    //     .await?;

    ///////////////////////
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

    // metrics:
    let duration = start_metric.elapsed().as_secs_f64();
    REQUEST_DURATION
        .with_label_values(&["create_url"])
        .observe(duration);
    //q

    Ok(Json(UrlResponse {
        short_code,
        long_url: payload.long_url,
        expiry_date: expiry_date.to_string(),
    }))
}

async fn redirect_to_long_url(
    State(state): State<Arc<AppState>>,
    Path(short_code): Path<String>,
) -> Response {
    // metric
    let start = tokio::time::Instant::now();
    REQUEST_COUNTER.inc();
    //
    if !state
        .distributed_filter
        .lock()
        .await
        .contains(&short_code, chrono::Utc::now().date_naive())
    {
        tracing::info!("Short code not found in filter");
        return AppError::NotFound.into_response();
    }
    // metric
    let duration = start.elapsed().as_secs_f64();
    REQUEST_DURATION
        .with_label_values(&["redirect"])
        .observe(duration);
    //
    match state.redis.get_long_url(&short_code).await {
        Ok(Some(long_url)) => {
            tracing::info!("Got it from redis");
            Redirect::permanent(&long_url).into_response()
        }
        Ok(None) => {
            match sqlx::query!(
                "SELECT long_url FROM urls
                 WHERE short_code = $1
                 AND expiry_date >= CURRENT_DATE",
                short_code
            )
            .fetch_optional(&state.pool)
            .await
            {
                Ok(Some(url)) => Redirect::permanent(&url.long_url).into_response(),
                _ => AppError::NotFound.into_response(),
            }
        }
        Err(_) => AppError::NotFound.into_response(),
    }
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");

    let pool = PgPool::connect(&database_url).await?;

    // Run migrations
    // sqlx::migrate!("./migrations").run(&pool).await?;
    let migrator = Migrator::new(std::path::Path::new("./migrations")).await?;

    // Run migrations
    migrator.run(&pool).await?;

    // Initialize distributed filter system
    let distributed_filter = initialize_distributed_filter_system(
        &pool,
        "affinitys3".to_string(),
        "distributed-filter-backups".to_string(),
    )
    .await?;

    let redis_manager = redis::RedisManager::new(&redis_url).await?;

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

    tokio::spawn(collect_system_metrics());

    let app = Router::new()
        .route("/api/urls", post(create_short_url))
        .route("/{short_code}", get(redirect_to_long_url))
        .route("/metrics", get(metrics_handler2))
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await?;
    println!("Server running on http://localhost:3001");
    axum::serve(listener, app).await?;

    Ok(())
}

// async fn metrics_handler() -> Result<String, AppError> {
//     let encoder = TextEncoder::new();
//     let mut buffer = vec![];
//     encoder
//         .encode(&prometheus::gather(), &mut buffer)
//         .map_err(|e| AppError::Prometheus(e))?;
//     String::from_utf8(buffer)
//         .map_err(|e| AppError::Prometheus(prometheus::Error::Msg(e.to_string())))
// }

async fn metrics_handler2() -> impl axum::response::IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    // Create response with correct content type
    (
        axum::http::StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        buffer,
    )
}
// System metrics collection task
async fn collect_system_metrics() {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
    loop {
        interval.tick().await;

        // Update CPU usage
        if let Ok(cpu) = sys_info::loadavg() {
            CPU_USAGE.set(cpu.one * 100.0);
        }

        // Update memory usage
        if let Ok(mem) = sys_info::mem_info() {
            let used_mem = mem.total - mem.free - mem.buffers - mem.cached;
            MEMORY_USAGE.set(used_mem as f64);
        }
    }
}
