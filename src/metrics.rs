use lazy_static::lazy_static;
use prometheus::{
    register_counter, register_gauge, register_histogram_vec, Counter, Gauge, HistogramVec,
};

lazy_static! {
    // For p99 latency and request timing
    pub static ref REQUEST_DURATION: HistogramVec = register_histogram_vec!(
        "request_duration_seconds",
        "Request duration in seconds by endpoint",
        &["endpoint"],
        vec![
            0.001, 0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0
        ]
    ).unwrap();

    // For requests per second
    pub static ref REQUEST_COUNTER: Counter = register_counter!(
        "requests_total",
        "Total number of requests"
    ).unwrap();

    // For CPU and memory
    pub static ref CPU_USAGE: Gauge = register_gauge!(
        "cpu_usage_percent",
        "CPU usage percentage"
    ).unwrap();

    pub static ref MEMORY_USAGE: Gauge = register_gauge!(
        "memory_usage_bytes",
        "Memory usage in bytes"
    ).unwrap();
}
// Helper function to get endpoint name from path
pub fn get_endpoint_name(path: &str) -> &str {
    if path.starts_with("/api/urls") {
        "create_url"
    } else if path == "/metrics" {
        "metrics"
    } else {
        "redirect"
    }
}
