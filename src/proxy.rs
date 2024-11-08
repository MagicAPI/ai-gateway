use axum::{
    body::{self, Body, Bytes},
    http::{Request, Response, HeaderMap, StatusCode}
};
use std::sync::Arc;
use http::HeaderValue;
use std::str::FromStr;
use crate::config::AppConfig;
use crate::error::AppError;
use tracing::{info, error};
use std::time::Duration;
use once_cell::sync::Lazy;
use futures_util::StreamExt;

/// Static HTTP client with optimized connection pooling and timeout settings
static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .pool_idle_timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(32)
        .tcp_keepalive(Duration::from_secs(60))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
});

/// Proxies incoming requests to the specified provider while maintaining optimal performance
/// through connection pooling and efficient streaming.
pub async fn proxy_request_to_provider(
    _config: Arc<AppConfig>,
    provider: &str,
    original_request: Request<Body>,
) -> Result<Response<Body>, AppError> {
    info!(
        provider = provider,
        method = %original_request.method(),
        path = %original_request.uri().path(),
        "Incoming request"
    );

    let base_url = match provider {
        "openai" => "https://api.openai.com",
        "anthropic" => "https://api.anthropic.com",
        "groq" => "https://api.groq.com/openai",
        _ => {
            error!(provider = provider, "Unsupported provider");
            return Err(AppError::UnsupportedProvider);
        }
    };

    let path = original_request.uri().path();
    let query = original_request
        .uri()
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();

    let url = format!("{}{}{}", base_url, path, query);
    info!(
        provider = provider,
        url = %url,
        method = %original_request.method(),
        "Preparing proxy request"
    );

    let method = reqwest::Method::from_str(original_request.method().as_str())
        .map_err(|_| AppError::InvalidMethod)?;
    
    // Optimize headers handling with pre-allocated capacity
    let mut reqwest_headers = reqwest::header::HeaderMap::with_capacity(8);
    reqwest_headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/json"),
    );

    // Header handling for different providers
    match provider {
        "openai" => {
            tracing::debug!("Processing OpenAI request headers");
            if let Some(api_key) = original_request.headers().get("x-magicapi-api-key")
                .and_then(|h| h.to_str().ok()) {
                tracing::debug!("Using x-magicapi-api-key for authentication");
                reqwest_headers.insert(
                    reqwest::header::AUTHORIZATION,
                    reqwest::header::HeaderValue::from_str(&format!("Bearer {}", api_key))
                        .map_err(|_| {
                            tracing::error!("Failed to create authorization header from x-magicapi-api-key");
                            AppError::InvalidHeader
                        })?
                );
            } else if let Some(auth) = original_request.headers().get("authorization")
                .and_then(|h| h.to_str().ok()) {
                tracing::debug!("Using provided authorization header");
                reqwest_headers.insert(
                    reqwest::header::AUTHORIZATION,
                    reqwest::header::HeaderValue::from_str(auth)
                        .map_err(|_| {
                            tracing::error!("Failed to process authorization header");
                            AppError::InvalidHeader
                        })?
                );
            } else {
                tracing::error!("No authorization header found for OpenAI request");
                return Err(AppError::MissingApiKey);
            }
        },
        "groq" => {
            tracing::debug!("Processing GROQ request headers");
            if let Some(auth) = original_request.headers().get("authorization")
                .and_then(|h| h.to_str().ok()) {
                tracing::debug!("Using provided authorization header for GROQ");
                reqwest_headers.insert(
                    reqwest::header::AUTHORIZATION,
                    reqwest::header::HeaderValue::from_str(auth)
                        .map_err(|_| {
                            tracing::error!("Failed to process GROQ authorization header");
                            AppError::InvalidHeader
                        })?
                );
            } else {
                tracing::error!("No authorization header found for GROQ request");
                return Err(AppError::MissingApiKey);
            }
        },
        _ => return Err(AppError::UnsupportedProvider),
    }

    tracing::info!("Proxying request to {}", url);
    // Efficiently handle request body
    let body_bytes = body::to_bytes(original_request.into_body(), usize::MAX).await?;
    tracing::debug!("Request body size: {} bytes", body_bytes.len());
    
    let proxy_request = CLIENT
        .request(method, url)
        .headers(reqwest_headers)
        .body(body_bytes.to_vec());

    tracing::debug!("Sending request to provider");
    let response = proxy_request.send().await.map_err(|e| {
        tracing::error!("Provider request failed: {}", e);
        e
    })?;
    let status = StatusCode::from_u16(response.status().as_u16())?;
    tracing::info!("Provider response status: {}", status);

    // Optimize streaming response handling
    if response.headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map_or(false, |ct| ct.contains("text/event-stream")) 
    {
        tracing::info!("Processing streaming response");
        // Efficient headers copying with proper type conversion
        let mut response_headers = HeaderMap::new();
        for (name, value) in response.headers() {
            if let Ok(v) = HeaderValue::from_bytes(value.as_bytes()) {
                if let Ok(header_name) = http::HeaderName::from_bytes(name.as_ref()) {
                    response_headers.insert(header_name, v);
                } else {
                    tracing::warn!("Failed to convert header name: {:?}", name);
                }
            } else {
                tracing::warn!("Failed to convert header value for: {:?}", name);
            }
        }

        tracing::debug!("Setting up streaming response");
        // Efficient stream handling with proper error mapping
        let stream = response.bytes_stream()
            .map(|result| {
                match result {
                    Ok(bytes) => {
                        tracing::trace!("Streaming chunk: {} bytes", bytes.len());
                        Ok(Bytes::from(bytes))
                    },
                    Err(e) => {
                        tracing::error!("Stream error: {}", e);
                        Err(std::io::Error::new(std::io::ErrorKind::Other, e))
                    }
                }
            });

        tracing::debug!("Returning streaming response");
        Ok(Response::builder()
            .status(status)
            .header("content-type", "text/event-stream")
            .header("cache-control", "no-cache")
            .header("connection", "keep-alive")
            .extension(response_headers)
            .body(Body::from_stream(stream))
            .unwrap())
    } else {
        // Extract headers before consuming the response body
        let mut response_headers = HeaderMap::new();
        for (name, value) in response.headers() {
            if let Ok(v) = HeaderValue::from_bytes(value.as_bytes()) {
                if let Ok(header_name) = http::HeaderName::from_bytes(name.as_ref()) {
                    response_headers.insert(header_name, v);
                } else {
                    tracing::warn!("Failed to convert header name: {:?}", name);
                }
            } else {
                tracing::warn!("Failed to convert header value for: {:?}", name);
            }
        }

        // Now consume the response body
        let body = response.bytes().await?;

        let mut builder = Response::builder().status(status);
        
        // Add headers individually to the builder
        for (name, value) in response_headers.iter() {
            builder = builder.header(name, value);
        }

        Ok(builder
            .body(Body::from(body))
            .unwrap())
    }
} 