use axum::{
    middleware::from_fn_with_state,
    routing::{any, get},
    Router,
    extract::connect_info::ConnectInfo,
    Extension,
};
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod context;
mod error;
mod handlers;
mod providers;
mod proxy;
mod telemetry;

use crate::{
    config::{AppConfig, TelemetryConfig},
    telemetry::{
        exporters::prometheus::PrometheusExporter,
        metrics::MetricsRegistry,
        middleware::{self, metrics_middleware},
        plugins::ConsolePlugin,
    },
};

#[tokio::main]
async fn main() {
    // Initialize tracing
    info!("Initializing tracing system");
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer().compact())
        .init();

    // Load configuration
    info!("Loading application configuration");
    let config = Arc::new(AppConfig::new());
    debug!(
        "Configuration loaded: port={}, host={}, worker_threads={}",
        config.port, config.host, config.worker_threads
    );

    // Optimize tokio runtime
    info!(
        "Configuring tokio runtime with {} worker threads",
        config.worker_threads
    );
    std::env::set_var("TOKIO_WORKER_THREADS", config.worker_threads.to_string());
    std::env::set_var("TOKIO_THREAD_STACK_SIZE", (2 * 1024 * 1024).to_string());

    // Setup CORS
    debug!("Setting up CORS layer with 1-hour max age");
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .max_age(Duration::from_secs(3600));

    let telemetry_config = TelemetryConfig::default();
    debug!(
        "Telemetry configuration: prometheus_enabled={}, debug_mode={}",
        telemetry_config.prometheus_enabled, telemetry_config.debug_mode
    );
    let metrics_registry = Arc::new(MetricsRegistry::new(telemetry_config.debug_mode));

    // Register exporters based on configuration
    if telemetry_config.prometheus_enabled {
        debug!("Registering Prometheus exporter");
        metrics_registry
            .register_exporter(Box::new(PrometheusExporter::new("ai_gateway".to_string())))
            .await;
    }

    if telemetry_config.debug_mode {
        debug!("Registering Console plugin for metrics");
        metrics_registry
            .register_exporter(Box::new(ConsolePlugin::new()))
            .await;
    }

    // Create router with optimized settings
    debug!("Creating application router");
    let app = Router::new()
        .route("/health", get(handlers::health_check))
        .route("/v1/*path", any(handlers::proxy_request))
        .layer(from_fn_with_state(
            metrics_registry.clone(),
            metrics_middleware,
        ))
        .with_state(config.clone())
        .layer(cors);

    // Start server with optimized TCP settings
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("Setting up TCP listener with non-blocking mode");
    let tcp_listener = std::net::TcpListener::bind(addr).expect("Failed to bind address");
    tcp_listener
        .set_nonblocking(true)
        .expect("Failed to set non-blocking");

    debug!("Converting to tokio TCP listener");
    let listener = tokio::net::TcpListener::from_std(tcp_listener)
        .expect("Failed to create Tokio TCP listener");

    info!(
        "AI Gateway listening on {}:{} with {} worker threads",
        config.host, config.port, config.worker_threads
    );

    debug!("Starting server with graceful shutdown");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .unwrap_or_else(|e| {
        error!("Server error: {}", e);
        std::process::exit(1);
    });
}

async fn shutdown_signal() {
    info!("Registering shutdown signal handler");
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler")
    };

    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {
            debug!("CTRL+C signal received");
        },
        _ = terminate => {
            debug!("Terminate signal received");
        },
    }
    info!("Shutdown signal received, starting graceful shutdown");
}