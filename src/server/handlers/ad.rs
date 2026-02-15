use crate::{ad::AdProvider, error::Result, server::state::AppState};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use tracing::info;

/// Serve ad segments by proxying from the configured ad source
///
/// The ad_name encodes the break and segment index (e.g. "break-0-seg-3.ts").
/// We delegate URL resolution to the AdProvider, keeping this handler decoupled
/// from ad source implementation details.
pub async fn serve_ad(
    Path((session_id, ad_name)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Response> {
    info!("Serving ad: {} for session: {}", ad_name, session_id);

    // Resolve ad segment identifier to actual source URL via the provider
    let ad_url = state
        .ad_provider
        .resolve_segment_url(&ad_name)
        .ok_or_else(|| {
            crate::error::RitcherError::InternalError(format!(
                "Failed to resolve ad segment URL for: {}",
                ad_name
            ))
        })?;

    info!("Fetching ad segment from: {}", ad_url);

    // Fetch ad segment from ad source using shared HTTP client
    let response = state.http_client.get(&ad_url).send().await?;

    if !response.status().is_success() {
        return Err(crate::error::RitcherError::OriginFetchError(
            response.error_for_status().unwrap_err(),
        ));
    }

    let bytes = response.bytes().await?;

    info!("Ad segment {} fetched: {} bytes", ad_name, bytes.len());

    // Return ad segment with proper Content-Type header
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "video/MP2T")],
        Body::from(bytes.to_vec()),
    )
        .into_response())
}
