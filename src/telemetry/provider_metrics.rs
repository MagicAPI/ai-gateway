use serde_json::Value;
use std::time::Duration;
use tracing::debug;

#[derive(Debug, Clone, Default)]
pub struct ProviderMetrics {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub cost: Option<f64>,
    pub model: String,
    pub provider_latency: Duration,
}

pub trait MetricsExtractor: Send + Sync {
    fn extract_metrics(&self, response_body: &Value) -> ProviderMetrics;
    fn extract_streaming_metrics(&self, chunk: &str) -> Option<ProviderMetrics>;
}

// OpenAI-compatible metrics extractor
pub struct OpenAIMetricsExtractor;

impl MetricsExtractor for OpenAIMetricsExtractor {
    fn extract_metrics(&self, response_body: &Value) -> ProviderMetrics {
        debug!("Extracting OpenAI metrics from response: {}", response_body);
        let mut metrics = ProviderMetrics::default();
        
        if let Some(usage) = response_body.get("usage") {
            debug!("Found usage data: {:?}", usage);
            metrics.input_tokens = usage.get("prompt_tokens").and_then(|v| v.as_u64()).map(|v| v as u32);
            metrics.output_tokens = usage.get("completion_tokens").and_then(|v| v.as_u64()).map(|v| v as u32);
            metrics.total_tokens = usage.get("total_tokens").and_then(|v| v.as_u64()).map(|v| v as u32);
            debug!("Extracted tokens - input: {:?}, output: {:?}, total: {:?}", 
                metrics.input_tokens, metrics.output_tokens, metrics.total_tokens);
        }

        if let Some(model) = response_body.get("model").and_then(|v| v.as_str()) {
            debug!("Found model: {}", model);
            metrics.model = model.to_string();
        }

        if let (Some(total_tokens), Some(model)) = (metrics.total_tokens, response_body.get("model")) {
            metrics.cost = Some(calculate_cost(model.as_str().unwrap_or(""), total_tokens));
            debug!("Calculated cost: {:?} for model {} and {} tokens", 
                metrics.cost, metrics.model, total_tokens);
        }

        debug!("Final extracted metrics: {:?}", metrics);
        metrics
    }

    fn extract_streaming_metrics(&self, chunk: &str) -> Option<ProviderMetrics> {
        debug!("Attempting to extract metrics from streaming chunk: {}", chunk);
        if let Ok(json) = serde_json::from_str::<Value>(chunk) {
            if json.get("usage").is_some() {
                debug!("Found usage in streaming chunk, extracting metrics");
                return Some(self.extract_metrics(&json));
            }
        }
        debug!("No usage data found in streaming chunk");
        None
    }
}

// Helper function to calculate cost based on model and tokens
fn calculate_cost(model: &str, total_tokens: u32) -> f64 {
    match model {
        m if m.contains("gpt-4") => (total_tokens as f64) * 0.00003,
        m if m.contains("gpt-3.5") => (total_tokens as f64) * 0.000002,
        _ => 0.0,
    }
}

// Anthropic-specific metrics extractor
pub struct AnthropicMetricsExtractor;

impl MetricsExtractor for AnthropicMetricsExtractor {
    fn extract_metrics(&self, response_body: &Value) -> ProviderMetrics {
        let mut metrics = ProviderMetrics::default();
        
        if let Some(usage) = response_body.get("usage") {
            metrics.input_tokens = usage.get("input_tokens").and_then(|v| v.as_u64()).map(|v| v as u32);
            metrics.output_tokens = usage.get("output_tokens").and_then(|v| v.as_u64()).map(|v| v as u32);
            metrics.total_tokens = Some(
                metrics.input_tokens.unwrap_or(0) + metrics.output_tokens.unwrap_or(0)
            );
        }

        if let Some(model) = response_body.get("model").and_then(|v| v.as_str()) {
            metrics.model = model.to_string();
        }

        metrics
    }

    fn extract_streaming_metrics(&self, chunk: &str) -> Option<ProviderMetrics> {
        None // Anthropic handles metrics differently in streaming mode
    }
}

// Add a new metrics extractor for Bedrock
pub struct BedrockMetricsExtractor;

impl MetricsExtractor for BedrockMetricsExtractor {
    fn extract_metrics(&self, response_body: &Value) -> ProviderMetrics {
        debug!("Extracting Bedrock metrics from response: {}", response_body);
        let mut metrics = ProviderMetrics::default();
        
        if let Some(usage) = response_body.get("usage") {
            debug!("Found Bedrock usage data: {:?}", usage);
            metrics.input_tokens = usage.get("inputTokens").and_then(|v| v.as_u64()).map(|v| v as u32);
            metrics.output_tokens = usage.get("outputTokens").and_then(|v| v.as_u64()).map(|v| v as u32);
            metrics.total_tokens = usage.get("totalTokens").and_then(|v| v.as_u64()).map(|v| v as u32);
            debug!("Extracted Bedrock tokens - input: {:?}, output: {:?}, total: {:?}", 
                metrics.input_tokens, metrics.output_tokens, metrics.total_tokens);
        }

        if let Some(model) = response_body.get("model").and_then(|v| v.as_str()) {
            debug!("Found Bedrock model: {}", model);
            metrics.model = model.to_string();
        }

        if let (Some(total_tokens), Some(model)) = (metrics.total_tokens, response_body.get("model")) {
            metrics.cost = Some(calculate_bedrock_cost(model.as_str().unwrap_or(""), total_tokens));
            debug!("Calculated Bedrock cost: {:?} for model {} and {} tokens", 
                metrics.cost, metrics.model, total_tokens);
        }

        debug!("Final extracted Bedrock metrics: {:?}", metrics);
        metrics
    }

    fn extract_streaming_metrics(&self, chunk: &str) -> Option<ProviderMetrics> {
        debug!("Attempting to extract metrics from Bedrock streaming chunk: {}", chunk);
        if let Ok(json) = serde_json::from_str::<Value>(chunk) {
            if json.get("usage").is_some() {
                debug!("Found usage in Bedrock streaming chunk");
                return Some(self.extract_metrics(&json));
            }
        }
        None
    }
}

// Helper function for Bedrock-specific cost calculation
fn calculate_bedrock_cost(model: &str, total_tokens: u32) -> f64 {
    match model {
        m if m.contains("claude") => (total_tokens as f64) * 0.00001102,
        m if m.contains("titan") => (total_tokens as f64) * 0.00001,
        m if m.contains("llama2") => (total_tokens as f64) * 0.00001,
        _ => 0.0,
    }
}

// Factory function to get the appropriate metrics extractor
pub fn get_metrics_extractor(provider: &str) -> Box<dyn MetricsExtractor> {
    match provider {
        "anthropic" => Box::new(AnthropicMetricsExtractor),
        "bedrock" => Box::new(BedrockMetricsExtractor), // Use Bedrock-specific extractor
        "groq" => Box::new(OpenAIMetricsExtractor),
        "fireworks" => Box::new(OpenAIMetricsExtractor),
        "together" => Box::new(OpenAIMetricsExtractor),
        _ => Box::new(OpenAIMetricsExtractor),
    }
} 