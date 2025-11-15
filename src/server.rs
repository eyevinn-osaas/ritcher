use crate::config::Config;
use axum::{
    routing::get,
    Router,
    extract::{Path, Query, State},
    response::IntoResponse,
    http::StatusCode,
};
use tracing::{info, error};
use crate::stitcher::parser;
use std::collections::HashMap;
use reqwest;

pub async fn start(config: Config) -> Result<(), Box<dyn std::error::Error>> {
  let addr = format!("0.0.0.0:{}", config.port);

  let app = Router::new()
    .route("/", get(health_check))
    .route("/health", get(health_check))
    .route("/stitch/{session_id}/playlist.m3u8", get(serve_playlist)).with_state(config);

  let listener = match tokio::net::TcpListener::bind(addr.as_str()).await {
    Ok(listener) => listener,
    Err(e) => {
      error!("Failed to bind to address {}: {}", addr, e);
      return Err(e.into());
    }
  };

  info!("🚀 Server listening on http://{}", addr);

  if let Err(e) = axum::serve(listener, app).await {
    error!("Server error: {}", e);
    return Err(e.into());
  }

  Ok(())
}

async fn health_check() -> &'static str {
    "🦀 Ritcher is running!"
}

async fn serve_playlist(
  Path(session_id): Path<String>,
  Query(params): Query<HashMap<String, String>>,
  State(config): State<Config>
) -> impl IntoResponse {
  info!("Serving playlist for session: {}", session_id);

  let origin_url = params.get("origin").map(|s| s.as_str()).unwrap_or(&config.origin_url);

  info!("fetching playlist from origin: {}", origin_url);

  let content = match reqwest::get(origin_url).await {
    Ok(response) => {
      if response.status().is_success() {
        match response.text().await {
          Ok(text) => text,
          Err(e) => {
            error!("Failed to read response text: {:?}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read response text".to_string());
          }
        }
      } else {
        error!("origin server returned error: {}", response.status());
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch playlist".to_string());
      }
    }
    Err(e) => {
      error!("Failed to fetch playlist from origin: {:?}", e);
      return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch playlist from origin".to_string());
    }
  };

  let playlist = match parser::parse_hls_playlist(&content) {
    Ok(p) => p,
    Err(e) => {
      error!("Failed to parse playlist: {:?}", e);
      return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to parse playlist".to_string());
    }
  };

  let modified_playlist = match parser::modify_playlist(playlist, &session_id, &config.base_url) {
    Ok(p) => p,
    Err(e) => {
      error!("Failed to modify playlist: {:?}", e);
      return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to modify playlist".to_string());
    }
  };

  (StatusCode::OK, modified_playlist)
}