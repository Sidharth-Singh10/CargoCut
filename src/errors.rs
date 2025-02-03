use axum::http::StatusCode;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("URL not found or expired")]
    NotFound,
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("Promethues error: {0}")]
    Prometheus(#[from] prometheus::Error),
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
            AppError::Redis(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Redis error: {}", err),
            )
                .into_response(),
            AppError::Prometheus(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Prometheus error: {}", err),
            )
                .into_response(),
        }
    }
}
