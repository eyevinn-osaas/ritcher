use m3u8_rs::{MediaPlaylist, MediaSegment};
use tracing::{debug, info};

/// Represents an ad break detected from CUE tags in the playlist
#[derive(Debug, Clone, PartialEq)]
pub struct AdBreak {
    /// Starting segment index (inclusive)
    pub start_index: usize,
    /// Ending segment index (exclusive)
    pub end_index: usize,
    /// Duration of the ad break in seconds
    pub duration: f32,
}

/// Detect ad breaks from SCTE-35 CUE tags in HLS playlists
///
/// Scans the `unknown_tags` field of each MediaSegment for industry-standard
/// CUE markers:
/// - `#EXT-X-CUE-OUT:{duration}` — ad break start
/// - `#EXT-X-CUE-OUT-CONT:{elapsed}/{duration}` — mid-break continuation
/// - `#EXT-X-CUE-IN` — ad break end
///
/// Returns a vector of AdBreak structs with start/end indices and duration.
pub fn detect_ad_breaks(playlist: &MediaPlaylist) -> Vec<AdBreak> {
    let mut ad_breaks = Vec::new();
    let mut current_break: Option<(usize, f32)> = None; // (start_index, duration)

    for (index, segment) in playlist.segments.iter().enumerate() {
        // Check unknown_tags for CUE markers
        for tag in &segment.unknown_tags {
            let tag_str = format!("{}:{}", tag.tag, tag.rest.as_deref().unwrap_or(""));

            if let Some(cue_out_duration) = parse_cue_out(&tag_str) {
                // CUE-OUT detected - start of ad break
                info!(
                    "Detected CUE-OUT at segment #{}: duration {}s",
                    index, cue_out_duration
                );

                if current_break.is_none() {
                    current_break = Some((index, cue_out_duration));
                }
            } else if tag_str.contains("EXT-X-CUE-IN") || tag_str.contains("EXT-CUE-IN") {
                // CUE-IN detected - end of ad break
                if let Some((start_idx, duration)) = current_break.take() {
                    info!("Detected CUE-IN at segment #{}", index);
                    ad_breaks.push(AdBreak {
                        start_index: start_idx,
                        end_index: index,
                        duration,
                    });
                }
            } else if tag_str.contains("EXT-X-CUE-OUT-CONT") {
                // CUE-OUT-CONT detected - continuation of ad break
                debug!("Detected CUE-OUT-CONT at segment #{}", index);
                // Don't need to do anything special, just indicates we're still in the break
            }
        }
    }

    // If we reached the end with an open ad break, close it
    if let Some((start_idx, duration)) = current_break {
        info!(
            "Ad break started at segment #{} not closed, ending at playlist end",
            start_idx
        );
        ad_breaks.push(AdBreak {
            start_index: start_idx,
            end_index: playlist.segments.len(),
            duration,
        });
    }

    ad_breaks
}

/// Parse CUE-OUT tag to extract duration
///
/// Supports formats:
/// - `#EXT-X-CUE-OUT:30` → 30.0
/// - `#EXT-X-CUE-OUT:DURATION=30` → 30.0
/// - `#EXT-CUE-OUT:30` → 30.0 (legacy format)
fn parse_cue_out(tag: &str) -> Option<f32> {
    if !tag.contains("CUE-OUT") || tag.contains("CUE-OUT-CONT") {
        return None;
    }

    // Try to extract duration from various formats
    if let Some(colon_pos) = tag.rfind(':') {
        let after_colon = &tag[colon_pos + 1..];

        // Handle "DURATION=30" format
        if after_colon.contains("DURATION=")
            && let Some(eq_pos) = after_colon.find('=')
        {
            let duration_str = &after_colon[eq_pos + 1..];
            if let Ok(duration) = duration_str.trim().parse::<f32>() {
                return Some(duration);
            }
        }

        // Handle simple "30" format
        if let Ok(duration) = after_colon.trim().parse::<f32>() {
            return Some(duration);
        }
    }

    None
}

/// Helper to check if a segment is within an ad break
pub fn is_in_ad_break(segment_index: usize, ad_breaks: &[AdBreak]) -> bool {
    ad_breaks
        .iter()
        .any(|ab| segment_index >= ab.start_index && segment_index < ab.end_index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use m3u8_rs::ExtTag;

    fn create_segment_with_tag(tag: &str, rest: Option<&str>) -> MediaSegment {
        MediaSegment {
            uri: "segment.ts".to_string(),
            duration: 10.0,
            title: None,
            byte_range: None,
            discontinuity: false,
            key: None,
            map: None,
            program_date_time: None,
            daterange: None,
            unknown_tags: vec![ExtTag {
                tag: tag.to_string(),
                rest: rest.map(|s| s.to_string()),
            }],
        }
    }

    #[test]
    fn test_parse_cue_out_simple() {
        assert_eq!(parse_cue_out("#EXT-X-CUE-OUT:30"), Some(30.0));
        assert_eq!(parse_cue_out("#EXT-X-CUE-OUT:60.5"), Some(60.5));
    }

    #[test]
    fn test_parse_cue_out_with_duration_key() {
        assert_eq!(parse_cue_out("#EXT-X-CUE-OUT:DURATION=30"), Some(30.0));
        assert_eq!(parse_cue_out("#EXT-X-CUE-OUT:DURATION=45.5"), Some(45.5));
    }

    #[test]
    fn test_parse_cue_out_legacy() {
        assert_eq!(parse_cue_out("#EXT-CUE-OUT:30"), Some(30.0));
    }

    #[test]
    fn test_parse_cue_out_invalid() {
        assert_eq!(parse_cue_out("#EXT-X-CUE-OUT-CONT"), None);
        assert_eq!(parse_cue_out("#EXT-X-CUE-IN"), None);
        assert_eq!(parse_cue_out("#EXT-X-CUE-OUT:invalid"), None);
    }

    #[test]
    fn test_detect_ad_breaks_simple() {
        let mut playlist = MediaPlaylist::default();
        playlist.segments = vec![
            create_segment_with_tag("SOMETHING", None),
            create_segment_with_tag("EXT-X-CUE-OUT", Some("30")),
            create_segment_with_tag("SOMETHING", None),
            create_segment_with_tag("SOMETHING", None),
            create_segment_with_tag("EXT-X-CUE-IN", None),
            create_segment_with_tag("SOMETHING", None),
        ];

        let ad_breaks = detect_ad_breaks(&playlist);

        assert_eq!(ad_breaks.len(), 1);
        assert_eq!(
            ad_breaks[0],
            AdBreak {
                start_index: 1,
                end_index: 4,
                duration: 30.0
            }
        );
    }

    #[test]
    fn test_detect_multiple_ad_breaks() {
        let mut playlist = MediaPlaylist::default();
        playlist.segments = vec![
            create_segment_with_tag("SOMETHING", None),
            create_segment_with_tag("EXT-X-CUE-OUT", Some("30")),
            create_segment_with_tag("SOMETHING", None),
            create_segment_with_tag("EXT-X-CUE-IN", None),
            create_segment_with_tag("SOMETHING", None),
            create_segment_with_tag("EXT-X-CUE-OUT", Some("60")),
            create_segment_with_tag("SOMETHING", None),
            create_segment_with_tag("EXT-X-CUE-IN", None),
        ];

        let ad_breaks = detect_ad_breaks(&playlist);

        assert_eq!(ad_breaks.len(), 2);
        assert_eq!(ad_breaks[0].start_index, 1);
        assert_eq!(ad_breaks[0].end_index, 3);
        assert_eq!(ad_breaks[0].duration, 30.0);
        assert_eq!(ad_breaks[1].start_index, 5);
        assert_eq!(ad_breaks[1].end_index, 7);
        assert_eq!(ad_breaks[1].duration, 60.0);
    }

    #[test]
    fn test_detect_unclosed_ad_break() {
        let mut playlist = MediaPlaylist::default();
        playlist.segments = vec![
            create_segment_with_tag("SOMETHING", None),
            create_segment_with_tag("EXT-X-CUE-OUT", Some("30")),
            create_segment_with_tag("SOMETHING", None),
        ];

        let ad_breaks = detect_ad_breaks(&playlist);

        assert_eq!(ad_breaks.len(), 1);
        assert_eq!(ad_breaks[0].start_index, 1);
        assert_eq!(ad_breaks[0].end_index, 3);
    }

    #[test]
    fn test_is_in_ad_break() {
        let ad_breaks = vec![AdBreak {
            start_index: 2,
            end_index: 5,
            duration: 30.0,
        }];

        assert!(!is_in_ad_break(0, &ad_breaks));
        assert!(!is_in_ad_break(1, &ad_breaks));
        assert!(is_in_ad_break(2, &ad_breaks));
        assert!(is_in_ad_break(3, &ad_breaks));
        assert!(is_in_ad_break(4, &ad_breaks));
        assert!(!is_in_ad_break(5, &ad_breaks));
    }
}
