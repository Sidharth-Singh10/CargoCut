use redis::RedisResult;
use redis::{Client, Commands, Connection};
use std::sync::Arc;
use tokio::sync::Mutex;
#[derive(Clone)]
pub struct RedisManager {
    pub conn: Arc<Mutex<Connection>>,
}

impl RedisManager {
    /// Initialize a Redis Cluster connection
    pub async fn new(redis_urls: &str) -> RedisResult<Self> {
        let client = Client::open(redis_urls)?;
        let conn = client.get_connection()?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Store short URL → long URL mapping
    pub async fn set_short_url(&self, short_code: &str, long_url: &str) -> RedisResult<()> {
        let mut conn = self.conn.lock().await;
        conn.set::<&str, &str, ()>(short_code, long_url)?;
        Ok(())
    }

    /// Retrieve long URL from short code
    pub async fn get_long_url(&self, short_code: &str) -> RedisResult<Option<String>> {
        let mut conn = self.conn.lock().await;
        let result = conn.get(short_code)?;
        Ok(result)
    }
}
