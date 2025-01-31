use redis::cluster_async::ClusterConnection;
use redis::AsyncCommands;
use redis::{cluster::ClusterClient, RedisResult};
use std::sync::Arc;
use tokio::sync::Mutex;
#[derive(Clone)]
pub struct RedisManager {
    pub conn: Arc<Mutex<ClusterConnection>>,
}

impl RedisManager {
    /// Initialize a Redis Cluster connection
    pub async fn new(redis_urls: Vec<String>) -> RedisResult<Self> {
        let client = ClusterClient::new(redis_urls)?;
        let conn = client.get_async_connection().await?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Store short URL → long URL mapping
    pub async fn set_short_url(&self, short_code: &str, long_url: &str) -> RedisResult<()> {
        let mut conn = self.conn.lock().await;
        conn.set::<&str, &str, ()>(short_code, long_url).await?;
        Ok(())
    }

    /// Retrieve long URL from short code
    pub async fn get_long_url(&self, short_code: &str) -> RedisResult<Option<String>> {
        let mut conn = self.conn.lock().await;
        let result = conn.get(short_code).await?;
        Ok(result)
    }
}
