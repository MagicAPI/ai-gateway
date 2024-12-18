pub mod exporters;
pub mod metrics;
pub mod plugins;
pub mod middleware;
pub mod provider_metrics;

pub use self::{
    metrics::MetricsRegistry,
    plugins::ConsolePlugin,
    exporters::prometheus::PrometheusExporter,
    middleware::metrics_middleware,
};

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetrics {
    // Request metadata
    pub provider: String,
    pub model: String,
    pub path: String,
    pub method: String,
    
    // Timing metrics
    pub total_latency: Duration,
    pub provider_latency: Duration,
    
    // Size metrics
    pub request_size: usize,
    pub response_size: usize,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    
    // Status metrics
    pub status_code: u16,
    pub provider_status_code: u16,
    
    // Error metrics
    pub error_count: u32,
    pub error_type: Option<String>,
    pub provider_error_count: u32,
    pub provider_error_type: Option<String>,
    
    // Cost metrics
    pub cost: Option<f64>,
}

impl Default for RequestMetrics {
    fn default() -> Self {
        Self {
            provider: String::new(),
            model: String::new(),
            path: String::new(),
            method: String::new(),
            total_latency: Duration::default(),
            provider_latency: Duration::default(),
            request_size: 0,
            response_size: 0,
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            status_code: 0,
            provider_status_code: 0,
            error_count: 0,
            error_type: None,
            provider_error_count: 0,
            provider_error_type: None,
            cost: None,
        }
    }
} 