pub mod handlers;
pub mod state;

use crate::config::Config;
use axum::{routing::get, Router};
use metrics_exporter_prometheus::PrometheusBuilder;
use state::AppState;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};

/// Start the Axum HTTP server
pub async fn start(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("0.0.0.0:{}", config.port);
    let port = config.port;
    let is_dev = config.is_dev;

    // Install Prometheus metrics recorder
    let prometheus_handle = PrometheusBuilder::new()
        .install_recorder()
        .expect("Failed to install Prometheus recorder");
    info!("Prometheus metrics recorder installed");

    // Create shared application state
    let state = AppState::new(config);

    // Spawn background task for session cleanup (prevents memory leaks)
    let cleanup_sessions = state.sessions.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            let before = cleanup_sessions.session_count();
            cleanup_sessions.cleanup_expired();
            let after = cleanup_sessions.session_count();
            if before != after {
                info!(
                    "Session cleanup: removed {} expired sessions ({} active)",
                    before - after,
                    after
                );
            }
            // Update session gauge metric
            crate::metrics::set_active_sessions(after);
        }
    });

    // CORS layer: permissive in dev mode for testing with external players
    let cors = if is_dev {
        info!("CORS: Permissive mode (dev)");
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        info!("CORS: Restrictive mode (prod)");
        // Default: no CORS headers — origins must be configured for production
        CorsLayer::new()
    };

    // Build router with all routes
    let app = Router::new()
        .route("/", get(handlers::health::health_check))
        .route("/health", get(handlers::health::health_check))
        // Prometheus metrics endpoint
        .route(
            "/metrics",
            get({
                let handle = prometheus_handle.clone();
                move || handlers::metrics::serve_metrics(handle)
            }),
        )
        // Demo endpoint: synthetic playlist with CUE markers for testing
        .route(
            "/demo/playlist.m3u8",
            get(handlers::demo::serve_demo_playlist),
        )
        // Stitcher endpoints
        .route(
            "/stitch/{session_id}/playlist.m3u8",
            get(handlers::playlist::serve_playlist),
        )
        .route(
            "/stitch/{session_id}/segment/{*segment_path}",
            get(handlers::segment::serve_segment),
        )
        .route(
            "/stitch/{session_id}/ad/{ad_name}",
            get(handlers::ad::serve_ad),
        )
        .layer(cors)
        .with_state(state);

    // Bind TCP listener
    let listener = match tokio::net::TcpListener::bind(addr.as_str()).await {
        Ok(listener) => listener,
        Err(e) => {
            error!("Failed to bind to address {}: {}", addr, e);
            return Err(e.into());
        }
    };

    info!("Server listening on http://{}", addr);
    info!("Demo playlist: http://{}/demo/playlist.m3u8", addr);
    info!(
        "Stitched demo: http://{}/stitch/demo/playlist.m3u8?origin=http://localhost:{}/demo/playlist.m3u8",
        addr, port
    );
    info!("Metrics: http://{}/metrics", addr);

    // Start serving
    if let Err(e) = axum::serve(listener, app).await {
        error!("Server error: {}", e);
        return Err(e.into());
    }

    Ok(())
}
