use crate::error::{Result, RitcherError};
use m3u8_rs::{parse_playlist_res, Playlist};
use tracing::info;

/// Parse HLS playlist from string content
pub fn parse_hls_playlist(content: &str) -> Result<Playlist> {
    info!("Parsing HLS playlist");

    match parse_playlist_res(content.as_bytes()) {
        Ok(playlist) => {
            info!("Successfully parsed playlist");
            Ok(playlist)
        }
        Err(e) => {
            let error_msg = format!("Failed to parse playlist: {:?}", e);
            Err(RitcherError::PlaylistParseError(error_msg))
        }
    }
}

/// Rewrite content segment URLs to route through stitcher's proxy
///
/// This function ONLY handles URL rewriting for content segments.
/// Ad insertion is handled separately by the ad interleaver.
///
/// For segments with absolute URLs (starting with http), the origin is
/// derived from the segment's own URL. For relative URLs, the provided
/// origin_base is used as the origin.
///
/// Note: URLs with query parameters are currently not handled specially.
/// The query string will be included in the segment name passed to the
/// segment handler.
pub fn rewrite_content_urls(
    mut playlist: Playlist,
    session_id: &str,
    base_url: &str,
    origin_base: &str,
) -> Result<Playlist> {
    info!("Rewriting content URLs for session: {}", session_id);

    if let Playlist::MediaPlaylist(ref mut media_playlist) = playlist {
        for segment in media_playlist.segments.iter_mut() {
            // Skip segments that are already routed through stitcher (ads)
            if segment.uri.contains("/stitch/") {
                continue;
            }

            info!("Rewriting segment URL: {}", segment.uri);

            if segment.uri.starts_with("http") {
                // Absolute URL: derive origin from the segment's own URL
                let (seg_origin, segment_name) = segment
                    .uri
                    .rsplit_once('/')
                    .unwrap_or(("", &segment.uri));

                segment.uri = format!(
                    "{}/stitch/{}/segment/{}?origin={}",
                    base_url, session_id, segment_name, seg_origin
                );
            } else {
                // Relative URL: use the provided origin base
                segment.uri = format!(
                    "{}/stitch/{}/segment/{}?origin={}",
                    base_url, session_id, segment.uri, origin_base
                );
            }
        }
    }

    Ok(playlist)
}

/// Serialize playlist to string
pub fn serialize_playlist(playlist: Playlist) -> Result<String> {
    let mut output = Vec::new();
    playlist
        .write_to(&mut output)
        .map_err(|e| RitcherError::PlaylistModifyError(format!("Failed to write playlist: {}", e)))?;

    String::from_utf8(output).map_err(|e| {
        RitcherError::ConversionError(format!("Failed to convert playlist to UTF-8: {}", e))
    })
}
