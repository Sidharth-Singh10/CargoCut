use serde::Deserialize;
use serde::Serialize;

#[derive(Deserialize)]
pub struct CreateUrl {
    pub long_url: String,
    pub months_valid: Option<u32>, // Optional expiry days
    pub custom_short_code: Option<String>,
}

#[derive(Serialize)]
pub struct UrlResponse {
    pub short_code: String,
    pub long_url: String,
    pub expiry_date: String,
}
