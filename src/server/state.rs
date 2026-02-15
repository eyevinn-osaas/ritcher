use crate::{
    ad::{AdProvider, StaticAdProvider, VastAdProvider},
    config::{AdProviderType, Config},
    session::SessionManager,
};
use reqwest::Client;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::info;

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    /// Application configuration
    pub config: Arc<Config>,
    /// Shared HTTP client for connection pooling
    pub http_client: Client,
    /// Session manager for tracking active sessions
    pub sessions: SessionManager,
    /// Ad provider for serving ad content (trait object for runtime flexibility)
    pub ad_provider: Arc<dyn AdProvider>,
    /// Server start time for uptime tracking
    pub started_at: Instant,
}

impl AppState {
    /// Create a new AppState with the given configuration
    pub fn new(config: Config) -> Self {
        let http_client = Client::builder()
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .build()
            .expect("Failed to create HTTP client");

        // Session TTL: 5 minutes
        let sessions = SessionManager::new(Duration::from_secs(300));

        // Create ad provider based on config
        let ad_provider: Arc<dyn AdProvider> = match config.ad_provider_type {
            AdProviderType::Vast => {
                let endpoint = config
                    .vast_endpoint
                    .as_deref()
                    .expect("VAST_ENDPOINT is required when AD_PROVIDER_TYPE=vast");
                info!("Ad provider: VAST (endpoint: {})", endpoint);
                Arc::new(VastAdProvider::new(
                    endpoint.to_string(),
                    http_client.clone(),
                ))
            }
            AdProviderType::Static => {
                info!(
                    "Ad provider: Static (source: {}, segment duration: {}s)",
                    config.ad_source_url, config.ad_segment_duration
                );
                Arc::new(StaticAdProvider::new(
                    config.ad_source_url.clone(),
                    config.ad_segment_duration,
                ))
            }
        };

        Self {
            config: Arc::new(config),
            http_client,
            sessions,
            ad_provider,
            started_at: Instant::now(),
        }
    }
}
