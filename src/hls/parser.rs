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
pub fn rewrite_content_urls(
    mut playlist: Playlist,
    session_id: &str,
    base_url: &str,
    origin_url: &str,
) -> Result<Playlist> {
    info!("Rewriting content URLs for session: {}", session_id);

    if let Playlist::MediaPlaylist(ref mut media_playlist) = playlist {
        for segment in media_playlist.segments.iter_mut() {
            // Skip segments that are already routed through stitcher (ads)
            if segment.uri.contains("/stitch/") {
                continue;
            }

            info!("Rewriting segment URL: {}", segment.uri);

            // Extract segment name from URL
            let segment_name = if segment.uri.starts_with("http") {
                segment.uri.split('/').next_back().unwrap_or(&segment.uri)
            } else {
                &segment.uri
            };

            // Rewrite to proxy through stitcher
            segment.uri = format!(
                "{}/stitch/{}/segment/{}?origin={}",
                base_url, session_id, segment_name, origin_url
            );
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
