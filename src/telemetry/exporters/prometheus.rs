use crate::telemetry::{metrics::MetricsExporter, RequestMetrics};
use async_trait::async_trait;
use metrics::{counter, gauge, histogram};

pub struct PrometheusExporter {
    namespace: String,
}

impl PrometheusExporter {
    pub fn new(namespace: String) -> Self {
        Self { namespace }
    }
}

#[async_trait]
impl MetricsExporter for PrometheusExporter {
    async fn export_metrics(&self, metrics: RequestMetrics) -> Result<(), Box<dyn std::error::Error>> {
        let labels = [
            ("provider", metrics.provider.clone()),
            ("model", metrics.model.clone()),
            ("path", metrics.path.clone()),
        ];

        // Record latency metrics
        let name = format!("{}_request_latency", self.namespace);
        histogram!(name, &labels).record(metrics.total_latency.as_secs_f64());
        
        let name = format!("{}_provider_latency", self.namespace);
        histogram!(name, &labels).record(metrics.provider_latency.as_secs_f64());

        // Record size metrics
        let name = format!("{}_request_size", self.namespace);
        gauge!(name, &labels).set(metrics.request_size as f64);
        
        let name = format!("{}_response_size", self.namespace);
        gauge!(name, &labels).set(metrics.response_size as f64);

        // Record token metrics
        if let Some(tokens) = metrics.total_tokens {
            let name = format!("{}_total_tokens", self.namespace);
            gauge!(name, &labels).set(tokens as f64);
        }

        // Record error metrics
        let name = format!("{}_error_count", self.namespace);
        counter!(name, &labels).increment(metrics.error_count as u64);

        Ok(())
    }

    fn name(&self) -> &str {
        "prometheus"
    }
}