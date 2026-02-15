pub mod handlers;
pub mod state;

use crate::config::Config;
use axum::{routing::get, Router};
use state::AppState;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};

/// Start the Axum HTTP server
pub async fn start(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("0.0.0.0:{}", config.port);
    let port = config.port;
    let is_dev = config.is_dev;

    // Create shared application state
    let state = AppState::new(config);

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

    info!("🚀 Server listening on http://{}", addr);
    info!("📺 Demo playlist: http://{}/demo/playlist.m3u8", addr);
    info!(
        "🔗 Stitched demo: http://{}/stitch/demo/playlist.m3u8?origin=http://localhost:{}/demo/playlist.m3u8",
        addr, port
    );

    // Start serving
    if let Err(e) = axum::serve(listener, app).await {
        error!("Server error: {}", e);
        return Err(e.into());
    }

    Ok(())
}
