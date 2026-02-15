use crate::ad::provider::{AdProvider, AdSegment};
use crate::ad::vast::{self, VastAdType};
use dashmap::DashMap;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

/// Ad creative resolved from VAST, cached per session
#[derive(Debug, Clone)]
struct ResolvedCreative {
    /// URL to the ad creative (HLS playlist or MP4)
    url: String,
    /// Duration in seconds
    duration: f32,
    /// Whether this is an HLS stream (vs progressive MP4)
    is_hls: bool,
}

/// VAST-based ad provider that fetches ads from a VAST endpoint
///
/// Implements the AdProvider trait by:
/// 1. Fetching VAST XML from configured endpoint on each ad break
/// 2. Parsing the response to extract media file URLs and durations
/// 3. Caching resolved creatives per session for segment URL resolution
#[derive(Clone)]
pub struct VastAdProvider {
    /// VAST endpoint URL (with optional macros like [DURATION])
    vast_endpoint: String,
    /// HTTP client for VAST requests
    http_client: Client,
    /// Per-session ad cache: maps "session_id:break-N-seg-M" to creative URL
    ad_cache: Arc<DashMap<String, ResolvedCreative>>,
    /// Maximum number of VAST wrapper redirects to follow
    max_wrapper_depth: u32,
    /// VAST request timeout
    timeout: Duration,
}

impl VastAdProvider {
    /// Create a new VastAdProvider
    ///
    /// # Arguments
    /// * `vast_endpoint` - VAST endpoint URL (supports [DURATION] and [CACHEBUSTING] macros)
    /// * `http_client` - Shared HTTP client for VAST requests
    pub fn new(vast_endpoint: String, http_client: Client) -> Self {
        Self {
            vast_endpoint,
            http_client,
            ad_cache: Arc::new(DashMap::new()),
            max_wrapper_depth: 5,
            timeout: Duration::from_millis(2000),
        }
    }

    /// Replace VAST macros in the endpoint URL
    fn resolve_endpoint(&self, duration: f32) -> String {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        self.vast_endpoint
            .replace("[DURATION]", &format!("{}", duration as u32))
            .replace("[CACHEBUSTING]", &format!("{}", timestamp))
    }

    /// Fetch and parse VAST XML, following wrapper chains
    ///
    /// Uses `block_in_place` to run async HTTP requests within the sync
    /// AdProvider trait methods. This is safe because Axum uses a multi-threaded
    /// runtime and `block_in_place` only blocks the current thread.
    fn fetch_vast(
        &self,
        url: &str,
        depth: u32,
    ) -> Option<Vec<(String, f32, bool)>> {
        if depth > self.max_wrapper_depth {
            warn!(
                "VAST wrapper chain exceeded max depth ({})",
                self.max_wrapper_depth
            );
            return None;
        }

        let client = self.http_client.clone();
        let url = url.to_string();
        let timeout = self.timeout;

        // Run async reqwest within sync context using block_in_place
        let xml = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let response = client
                    .get(&url)
                    .timeout(timeout)
                    .send()
                    .await;

                match response {
                    Ok(resp) if resp.status().is_success() => resp.text().await.ok(),
                    Ok(resp) => {
                        error!("VAST endpoint returned status {}", resp.status());
                        None
                    }
                    Err(e) => {
                        error!("VAST request failed: {}", e);
                        None
                    }
                }
            })
        })?;

        let vast_response = match vast::parse_vast(&xml) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to parse VAST XML: {}", e);
                return None;
            }
        };

        let mut creatives = Vec::new();

        for ad in &vast_response.ads {
            match &ad.ad_type {
                VastAdType::InLine(inline) => {
                    for creative in &inline.creatives {
                        if let Some(linear) = &creative.linear {
                            if let Some(media_file) =
                                vast::select_best_media_file(&linear.media_files)
                            {
                                let is_hls =
                                    media_file.mime_type == "application/x-mpegURL";
                                creatives.push((
                                    media_file.url.clone(),
                                    linear.duration,
                                    is_hls,
                                ));
                            }
                        }
                    }
                }
                VastAdType::Wrapper(wrapper) => {
                    // Follow wrapper chain recursively
                    if let Some(mut wrapped_creatives) =
                        self.fetch_vast(&wrapper.ad_tag_uri, depth + 1)
                    {
                        creatives.append(&mut wrapped_creatives);
                    }
                }
            }
        }

        Some(creatives)
    }

    /// Build cache key for ad segment lookup
    fn cache_key(session_id: &str, ad_name: &str) -> String {
        format!("{}:{}", session_id, ad_name)
    }
}

impl std::fmt::Debug for VastAdProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VastAdProvider")
            .field("vast_endpoint", &self.vast_endpoint)
            .field("max_wrapper_depth", &self.max_wrapper_depth)
            .field("timeout", &self.timeout)
            .field("cached_entries", &self.ad_cache.len())
            .finish()
    }
}

impl AdProvider for VastAdProvider {
    fn get_ad_segments(&self, duration: f32, session_id: &str) -> Vec<AdSegment> {
        let url = self.resolve_endpoint(duration);
        info!(
            "VastAdProvider: Fetching VAST for session {} (duration: {}s) from {}",
            session_id, duration, url
        );

        let creatives = match self.fetch_vast(&url, 0) {
            Some(c) if !c.is_empty() => c,
            _ => {
                warn!(
                    "VastAdProvider: No creatives resolved for session {} — returning empty",
                    session_id
                );
                return Vec::new();
            }
        };

        // Build ad segments and cache them for resolve_segment_url
        let mut segments = Vec::new();
        let break_idx = 0; // TODO: track break index per session

        for (seg_idx, (url, creative_duration, is_hls)) in creatives.iter().enumerate() {
            let ad_name = format!("break-{}-seg-{}.ts", break_idx, seg_idx);

            // Cache the resolved creative for later URL resolution
            self.ad_cache.insert(
                Self::cache_key(session_id, &ad_name),
                ResolvedCreative {
                    url: url.clone(),
                    duration: *creative_duration,
                    is_hls: *is_hls,
                },
            );

            segments.push(AdSegment {
                uri: ad_name,
                duration: *creative_duration,
            });
        }

        info!(
            "VastAdProvider: Resolved {} ad segment(s) for session {}",
            segments.len(),
            session_id
        );

        segments
    }

    fn resolve_segment_url(&self, ad_name: &str) -> Option<String> {
        // Search across all sessions for this ad_name.
        // Ad names include break and segment indices, making them unique enough.
        for entry in self.ad_cache.iter() {
            if entry.key().ends_with(&format!(":{}", ad_name)) {
                return Some(entry.value().url.clone());
            }
        }

        warn!("VastAdProvider: No cached creative found for {}", ad_name);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_endpoint_macros() {
        let client = Client::new();
        let provider = VastAdProvider::new(
            "http://ads.example.com/vast?dur=[DURATION]&cb=[CACHEBUSTING]".to_string(),
            client,
        );

        let resolved = provider.resolve_endpoint(30.0);
        assert!(resolved.contains("dur=30"));
        assert!(!resolved.contains("[CACHEBUSTING]"));
        assert!(!resolved.contains("[DURATION]"));
    }

    #[test]
    fn test_cache_key() {
        assert_eq!(
            VastAdProvider::cache_key("session-1", "break-0-seg-0.ts"),
            "session-1:break-0-seg-0.ts"
        );
    }
}
