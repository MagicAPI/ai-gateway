use crate::telemetry::{RequestMetrics, metrics::MetricsExporter};
use async_trait::async_trait;
use std::error::Error;

pub struct ConsolePlugin;

impl ConsolePlugin {
    pub fn new() -> Self {
        ConsolePlugin
    }
}

#[async_trait]
impl MetricsExporter for ConsolePlugin {
    async fn export_metrics(&self, metrics: RequestMetrics) -> Result<(), Box<dyn Error>> {
        println!("Request Metrics:\n{:#?}", metrics);
        Ok(())
    }

    fn name(&self) -> &str {
        "console"
    }
}
