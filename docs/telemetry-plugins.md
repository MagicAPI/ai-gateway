# Telemetry Plugins Guide

This guide explains how to create and integrate new telemetry exporters into the MagicAPI Gateway.

## Overview

The MagicAPI Gateway uses a plugin-based architecture for telemetry exporters, allowing easy integration of new exporters like Elasticsearch, DataDog, New Relic, etc.

## Creating a New Exporter Plugin

### 1. Create a New Exporter Module

Create a new file in `src/telemetry/exporters/elasticsearch.rs`:

```rust
use async_trait::async_trait;
use opentelemetry::trace::TraceError;
use serde_json::json;
use std::sync::Arc;
use reqwest::Client;
use tracing::info;

use crate::telemetry::{TelemetryConfig, TelemetryExporter};

pub struct ElasticsearchExporter {
    client: Client,
    url: String,
    index: String,
}

impl ElasticsearchExporter {
    pub fn new(url: String, index: String) -> Self {
        Self {
            client: Client::new(),
            url,
            index,
        }
    }

    async fn send_metrics(&self, metrics: serde_json::Value) -> Result<(), TraceError> {
        let url = format!("{}/{}/metrics", self.url, self.index);
        
        self.client
            .post(&url)
            .json(&metrics)
            .send()
            .await
            .map_err(|e| TraceError::from(e.to_string()))?;
            
        Ok(())
    }
}

#[async_trait]
impl TelemetryExporter for ElasticsearchExporter {
    fn name(&self) -> &str {
        "elasticsearch"
    }

    async fn init(&self, config: &TelemetryConfig) -> Result<(), TraceError> {
        info!("Initializing Elasticsearch exporter");
        
        // Test connection
        let health_url = format!("{}/_cluster/health", self.url);
        self.client
            .get(&health_url)
            .send()
            .await
            .map_err(|e| TraceError::from(e.to_string()))?;
            
        Ok(())
    }

    async fn shutdown(&self) -> Result<(), TraceError> {
        info!("Shutting down Elasticsearch exporter");
        Ok(())
    }
}
```

### 2. Create a Plugin Implementation

Create a new file in `src/telemetry/plugins/elasticsearch.rs`:

```rust
use inventory::submit;
use crate::telemetry::{plugins::TelemetryPlugin, TelemetryExporter};
use crate::telemetry::exporters::elasticsearch::ElasticsearchExporter;

pub struct ElasticsearchPlugin;

#[async_trait::async_trait]
impl TelemetryPlugin for ElasticsearchPlugin {
    fn name(&self) -> &str {
        "elasticsearch"
    }

    fn create_exporter(&self) -> Box<dyn TelemetryExporter> {
        let url = std::env::var("ELASTICSEARCH_URL")
            .unwrap_or_else(|_| "http://localhost:9200".to_string());
        let index = std::env::var("ELASTICSEARCH_INDEX")
            .unwrap_or_else(|_| "magicapi-metrics".to_string());
            
        Box::new(ElasticsearchExporter::new(url, index))
    }
}

// Register the plugin
inventory::submit! {
    Box::new(ElasticsearchPlugin) as Box<dyn TelemetryPlugin>
}
```

### 3. Update Module Declarations

Add to `src/telemetry/exporters/mod.rs`:
```rust
pub mod elasticsearch;
pub use elasticsearch::ElasticsearchExporter;
```

## Configuration

### Environment Variables

```bash
# Enable Elasticsearch exporter
ENABLED_EXPORTERS=prometheus,elasticsearch

# Elasticsearch configuration
ELASTICSEARCH_URL=http://localhost:9200
ELASTICSEARCH_INDEX=magicapi-metrics
```

### Docker Compose Example

```yaml
version: '3.8'
services:
  gateway:
    image: magicapi1/magicapi-ai-gateway:latest
    environment:
      - ENABLED_EXPORTERS=prometheus,elasticsearch
      - ELASTICSEARCH_URL=http://elasticsearch:9200
      - ELASTICSEARCH_INDEX=magicapi-metrics
    ports:
      - "3000:3000"
    depends_on:
      - elasticsearch

  elasticsearch:
    image: docker.elastic.co/elasticsearch/elasticsearch:8.12.0
    environment:
      - discovery.type=single-node
      - xpack.security.enabled=false
    ports:
      - "9200:9200"
```

## Custom Metrics

You can extend the metrics collection by adding new metrics to `GatewayMetrics`:

```rust
// In src/telemetry/metrics.rs
pub struct GatewayMetrics {
    // Existing metrics...
    
    // Add custom metrics
    pub custom_metric: Counter<u64>,
    pub custom_histogram: Histogram<f64>,
}

impl GatewayMetrics {
    pub fn new(meter: Meter) -> Self {
        Self {
            // Existing metrics...
            
            custom_metric: meter
                .u64_counter("custom_metric")
                .with_description("Description of custom metric")
                .with_unit(Unit::new("units"))
                .init(),
                
            custom_histogram: meter
                .f64_histogram("custom_histogram")
                .with_description("Description of custom histogram")
                .with_unit(Unit::new("units"))
                .init(),
        }
    }
}
```

## Best Practices

1. **Error Handling**
   - Implement proper error handling and logging
   - Use appropriate backoff strategies for failed exports
   - Consider circuit breakers for external services

2. **Performance**
   - Batch metrics when possible
   - Use appropriate buffer sizes
   - Implement rate limiting if needed

3. **Security**
   - Support TLS/SSL connections
   - Implement authentication
   - Sanitize sensitive data

4. **Configuration**
   - Make all parameters configurable
   - Provide sensible defaults
   - Document all configuration options

## Example: Adding DataDog Exporter

Here's a quick example of adding a DataDog exporter:

```rust
// src/telemetry/exporters/datadog.rs
pub struct DatadogExporter {
    api_key: String,
    site: String,
    client: Client,
}

#[async_trait]
impl TelemetryExporter for DatadogExporter {
    fn name(&self) -> &str {
        "datadog"
    }

    async fn init(&self, config: &TelemetryConfig) -> Result<(), TraceError> {
        // Initialize DataDog connection
        Ok(())
    }

    async fn shutdown(&self) -> Result<(), TraceError> {
        // Cleanup
        Ok(())
    }
}

// src/telemetry/plugins/datadog.rs
pub struct DatadogPlugin;

#[async_trait]
impl TelemetryPlugin for DatadogPlugin {
    fn name(&self) -> &str {
        "datadog"
    }

    fn create_exporter(&self) -> Box<dyn TelemetryExporter> {
        let api_key = std::env::var("DD_API_KEY").expect("DD_API_KEY must be set");
        let site = std::env::var("DD_SITE").unwrap_or_else(|_| "datadoghq.com".to_string());
        
        Box::new(DatadogExporter::new(api_key, site))
    }
}

inventory::submit! {
    Box::new(DatadogPlugin) as Box<dyn TelemetryPlugin>
}
```

## Testing Plugins

Create tests for your exporter:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_elasticsearch_exporter() {
        let exporter = ElasticsearchExporter::new(
            "http://localhost:9200".to_string(),
            "test-metrics".to_string(),
        );
        
        let config = TelemetryConfig::default();
        assert!(exporter.init(&config).await.is_ok());
        // Add more tests...
    }
}
```

## Troubleshooting

Common issues and solutions:

1. **Connection Issues**
   ```bash
   # Check connectivity
   curl -X GET "localhost:9200/_cluster/health"
   ```

2. **Authentication Errors**
   - Verify credentials
   - Check environment variables
   - Ensure proper permissions

3. **Performance Issues**
   - Monitor memory usage
   - Check batch sizes
   - Verify network latency

## Resources

- [OpenTelemetry Documentation](https://opentelemetry.io/docs/)
- [Elasticsearch API Reference](https://www.elastic.co/guide/en/elasticsearch/reference/current/index.html)
- [DataDog API Documentation](https://docs.datadoghq.com/api/) 