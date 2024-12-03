use super::TelemetryPlugin;
use crate::telemetry::RequestMetrics;
use async_trait::async_trait;
use elasticsearch::{
    auth::Credentials,
    http::transport::{Transport, TransportBuilder, SingleNodeConnectionPool},
    Elasticsearch, IndexParts,
};
use serde_json::json;
use std::error::Error;
use tracing::{debug, error};

pub struct ElasticsearchPlugin {
    client: Elasticsearch,
    index: String,
}

impl ElasticsearchPlugin {
    pub fn new(url: String, username: Option<String>, password: Option<String>, index: String) -> Result<Self, Box<dyn Error>> {
        let transport = match (username, password) {
            (Some(u), Some(p)) => {
                let credentials = Credentials::Basic(u, p);
                let conn_pool = SingleNodeConnectionPool::new(url.parse()?);
                let transport_builder = TransportBuilder::new(conn_pool)
                    .auth(credentials);
                transport_builder.build()?
            }
            _ => {
                let conn_pool = SingleNodeConnectionPool::new(url.parse()?);
                TransportBuilder::new(conn_pool).build()?
            }
        };

        Ok(Self {
            client: Elasticsearch::new(transport),
            index,
        })
    }
}

#[async_trait]
impl TelemetryPlugin for ElasticsearchPlugin {
    async fn export(&self, metrics: &RequestMetrics) -> Result<(), Box<dyn Error>> {
        let document = json!({
            "timestamp": chrono::Utc::now(),
            "provider": metrics.provider,
            "model": metrics.model,
            "path": metrics.path,
            "method": metrics.method,
            "total_latency_ms": metrics.total_latency.as_millis(),
            "provider_latency_ms": metrics.provider_latency.as_millis(),
            "request_size": metrics.request_size,
            "response_size": metrics.response_size,
            "input_tokens": metrics.input_tokens,
            "output_tokens": metrics.output_tokens,
            "total_tokens": metrics.total_tokens,
            "status_code": metrics.status_code,
            "provider_status_code": metrics.provider_status_code,
            "error_count": metrics.error_count,
            "error_type": metrics.error_type,
            "provider_error_count": metrics.provider_error_count,
            "provider_error_type": metrics.provider_error_type,
            "cost": metrics.cost,
        });

        debug!("Sending metrics to Elasticsearch index: {}", self.index);
        
        let response = self.client
            .index(IndexParts::Index(&self.index))
            .body(document)
            .send()
            .await?;

        if !response.status_code().is_success() {
            error!(
                "Failed to index metrics: status={}, response={:?}",
                response.status_code(),
                response.text().await?
            );
            return Err("Failed to index metrics".into());
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "elasticsearch"
    }
} 