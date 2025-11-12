use axum::{
    routing::get,
    Router,
};
use tracing::{info, error};

pub async fn start() -> Result<(), Box<dyn std::error::Error>> {
  let addr = "0.0.0.0:3000";

  let app = Router::new()
    .route("/", get(health_check))
    .route("/health", get(health_check));

  let listener = match tokio::net::TcpListener::bind(addr).await {
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