use crate::{
    ad::{interleaver, AdProvider},
    error::Result,
    hls::{cue, parser},
    server::state::AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use m3u8_rs::Playlist;
use std::collections::HashMap;
use tracing::info;

/// Serve modified HLS playlist with stitched ad markers
pub async fn serve_playlist(
    Path(session_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Result<Response> {
    info!("Serving playlist for session: {}", session_id);

    // Get origin URL from query params or fallback to config
    let origin_url = params
        .get("origin")
        .map(|s| s.as_str())
        .unwrap_or(&state.config.origin_url);

    info!("Fetching playlist from origin: {}", origin_url);

    // Fetch playlist from origin using shared HTTP client
    let response = state.http_client.get(origin_url).send().await?;

    if !response.status().is_success() {
        return Err(crate::error::RitcherError::OriginFetchError(
            response.error_for_status().unwrap_err(),
        ));
    }

    let content = response.text().await?;

    // Parse HLS playlist
    let playlist = parser::parse_hls_playlist(&content)?;

    // Extract base URL from origin
    let origin_base = origin_url
        .rsplit_once('/')
        .map(|(base, _)| base)
        .unwrap_or(origin_url);

    // Process playlist through the ad insertion pipeline
    let modified_playlist = process_playlist(
        playlist,
        &session_id,
        &state.config.base_url,
        origin_base,
        state.ad_provider.as_ref(),
    )?;

    // Serialize to string
    let playlist_str = parser::serialize_playlist(modified_playlist)?;

    // Return playlist with proper Content-Type header
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
        playlist_str,
    )
        .into_response())
}

/// Process playlist through the ad insertion pipeline
fn process_playlist(
    playlist: Playlist,
    session_id: &str,
    base_url: &str,
    origin_base: &str,
    ad_provider: &dyn AdProvider,
) -> Result<Playlist> {
    // Only process MediaPlaylist (not MasterPlaylist)
    let Playlist::MediaPlaylist(mut media_playlist) = playlist else {
        return Ok(playlist);
    };

    // Step 1: Detect ad breaks from CUE tags
    let ad_breaks = cue::detect_ad_breaks(&media_playlist);

    if !ad_breaks.is_empty() {
        info!("Detected {} ad break(s)", ad_breaks.len());

        // Step 2: Get ad segments for each break
        let ad_segments_per_break: Vec<_> = ad_breaks
            .iter()
            .map(|ad_break| ad_provider.get_ad_segments(ad_break.duration, session_id))
            .collect();

        // Step 3: Interleave ads into playlist
        media_playlist = interleaver::interleave_ads(
            media_playlist,
            &ad_breaks,
            &ad_segments_per_break,
            session_id,
            base_url,
        );
    } else {
        info!("No ad breaks detected in playlist");
    }

    // Step 4: Rewrite content URLs to proxy through stitcher
    let playlist = Playlist::MediaPlaylist(media_playlist);
    parser::rewrite_content_urls(playlist, session_id, base_url, origin_base)
}
