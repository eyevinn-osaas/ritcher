use tracing::info;

/// Represents a single ad segment
#[derive(Debug, Clone, PartialEq)]
pub struct AdSegment {
    /// URI of the ad segment
    pub uri: String,
    /// Duration of the segment in seconds
    pub duration: f32,
}

/// Trait for ad content providers
///
/// Implementations provide ad segments to fill ad breaks of a given duration.
/// This abstraction allows for different ad decision strategies (static, VAST, VMAP, etc.)
pub trait AdProvider: Send + Sync {
    /// Get ad segments to fill an ad break of the given duration
    ///
    /// # Arguments
    /// * `duration` - Duration of the ad break in seconds
    /// * `session_id` - Session ID for tracking and personalization
    ///
    /// # Returns
    /// A vector of AdSegment structs. The total duration may be less than, equal to,
    /// or slightly greater than the requested duration.
    fn get_ad_segments(&self, duration: f32, session_id: &str) -> Vec<AdSegment>;
}

/// Static ad provider that returns a fixed set of ad segments
///
/// This is the MVP implementation that uses a configured ad source URL
/// and segment duration to generate ad segments.
#[derive(Clone, Debug)]
pub struct StaticAdProvider {
    /// Base URL for ad segments
    ad_source_url: String,
    /// Duration of each ad segment
    segment_duration: f32,
}

impl StaticAdProvider {
    /// Create a new StaticAdProvider
    ///
    /// # Arguments
    /// * `ad_source_url` - Base URL where ad segments are hosted
    /// * `segment_duration` - Duration of each ad segment in seconds
    pub fn new(ad_source_url: String, segment_duration: f32) -> Self {
        Self {
            ad_source_url,
            segment_duration,
        }
    }
}

impl AdProvider for StaticAdProvider {
    fn get_ad_segments(&self, duration: f32, session_id: &str) -> Vec<AdSegment> {
        info!(
            "StaticAdProvider: Generating ad segments for session {} with duration {}s",
            session_id, duration
        );

        // Calculate how many segments we need to fill the duration
        let num_segments = (duration / self.segment_duration).ceil() as usize;
        let num_segments = num_segments.max(1); // At least one segment

        // Generate ad segments
        let segments: Vec<AdSegment> = (0..num_segments)
            .map(|i| AdSegment {
                // For MVP, all segments point to the same ad source
                // In production, this would be different ad creatives
                uri: format!("{}/ad-segment-{}.ts", self.ad_source_url, i),
                duration: self.segment_duration,
            })
            .collect();

        info!(
            "StaticAdProvider: Generated {} ad segments (total duration: {}s)",
            segments.len(),
            segments.len() as f32 * self.segment_duration
        );

        segments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_ad_provider_exact_duration() {
        let provider = StaticAdProvider::new("https://ads.example.com".to_string(), 10.0);
        let segments = provider.get_ad_segments(30.0, "test-session");

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].duration, 10.0);
        assert_eq!(segments[0].uri, "https://ads.example.com/ad-segment-0.ts");
        assert_eq!(segments[1].uri, "https://ads.example.com/ad-segment-1.ts");
        assert_eq!(segments[2].uri, "https://ads.example.com/ad-segment-2.ts");
    }

    #[test]
    fn test_static_ad_provider_partial_duration() {
        let provider = StaticAdProvider::new("https://ads.example.com".to_string(), 10.0);
        let segments = provider.get_ad_segments(25.0, "test-session");

        // 25 / 10 = 2.5, ceiling = 3 segments
        assert_eq!(segments.len(), 3);
    }

    #[test]
    fn test_static_ad_provider_min_one_segment() {
        let provider = StaticAdProvider::new("https://ads.example.com".to_string(), 10.0);
        let segments = provider.get_ad_segments(2.0, "test-session");

        // Even for very short duration, return at least 1 segment
        assert_eq!(segments.len(), 1);
    }

    #[test]
    fn test_static_ad_provider_zero_duration() {
        let provider = StaticAdProvider::new("https://ads.example.com".to_string(), 10.0);
        let segments = provider.get_ad_segments(0.0, "test-session");

        // Should return at least 1 segment
        assert_eq!(segments.len(), 1);
    }
}
