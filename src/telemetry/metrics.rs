use super::RequestMetrics;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

#[async_trait]
pub trait MetricsExporter: Send + Sync {
    async fn export_metrics(&self, metrics: RequestMetrics) -> Result<(), Box<dyn std::error::Error>>;
    fn name(&self) -> &str;
}

pub struct MetricsRegistry {
    exporters: Arc<RwLock<Vec<Box<dyn MetricsExporter>>>>,
    debug_mode: bool,
}

impl MetricsRegistry {
    pub fn new(debug_mode: bool) -> Self {
        Self {
            exporters: Arc::new(RwLock::new(Vec::new())),
            debug_mode,
        }
    }

    pub async fn register_exporter(&self, exporter: Box<dyn MetricsExporter>) {
        let mut exporters = self.exporters.write().await;
        info!("Registering metrics exporter: {}", exporter.name());
        exporters.push(exporter);
    }

    pub async fn record_metrics(&self, metrics: RequestMetrics) {
        if self.debug_mode {
            debug!("Request Metrics: {:#?}", metrics);
        }

        let exporters = self.exporters.read().await;
        for exporter in exporters.iter() {
            if let Err(e) = exporter.export_metrics(metrics.clone()).await {
                error!("Failed to export metrics to {}: {}", exporter.name(), e);
            }
        }
    }
} 