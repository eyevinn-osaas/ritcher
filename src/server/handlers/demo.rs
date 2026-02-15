use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use tracing::info;

/// Demo HLS playlist endpoint for testing the ad insertion pipeline
///
/// Serves a synthetic HLS media playlist that uses segments from a real
/// test stream (Mux public test stream) with SCTE-35 CUE-OUT/CUE-IN
/// markers injected to simulate ad break opportunities.
///
/// Usage:
///   1. Start Ritcher: `DEV_MODE=true cargo run`
///   2. Open VLC or any HLS player
///   3. Point to: http://localhost:3000/stitch/demo/playlist.m3u8?origin=http://localhost:3000/demo/playlist.m3u8
///
/// The stitcher will fetch this demo playlist, detect the CUE markers,
/// and replace the marked segments with ad content.
pub async fn serve_demo_playlist() -> Response {
    info!("Serving demo HLS playlist with CUE markers");

    // Build a synthetic HLS media playlist with real, reachable test segments
    // from the Mux public test stream. Each segment uses a different sub-path
    // (url_462, url_463, etc.) matching the actual Mux stream layout.
    //
    // The CUE markers create a 30-second ad break window at segments 5-7.
    // During stitching, those 3 segments will be replaced by ad content.
    let playlist = r#"#EXTM3U
#EXT-X-VERSION:3
#EXT-X-TARGETDURATION:10
#EXT-X-MEDIA-SEQUENCE:0

#EXTINF:10.0,
https://test-streams.mux.dev/x36xhzz/url_0/url_462/193039199_mp4_h264_aac_hd_7.ts
#EXTINF:10.0,
https://test-streams.mux.dev/x36xhzz/url_0/url_463/193039199_mp4_h264_aac_hd_7.ts
#EXTINF:10.0,
https://test-streams.mux.dev/x36xhzz/url_0/url_464/193039199_mp4_h264_aac_hd_7.ts
#EXTINF:10.0,
https://test-streams.mux.dev/x36xhzz/url_0/url_465/193039199_mp4_h264_aac_hd_7.ts
#EXTINF:10.0,
https://test-streams.mux.dev/x36xhzz/url_0/url_466/193039199_mp4_h264_aac_hd_7.ts

#EXT-X-CUE-OUT:30
#EXTINF:10.0,
https://test-streams.mux.dev/x36xhzz/url_0/url_467/193039199_mp4_h264_aac_hd_7.ts
#EXT-X-CUE-OUT-CONT:10/30
#EXTINF:10.0,
https://test-streams.mux.dev/x36xhzz/url_0/url_468/193039199_mp4_h264_aac_hd_7.ts
#EXT-X-CUE-OUT-CONT:20/30
#EXTINF:10.0,
https://test-streams.mux.dev/x36xhzz/url_0/url_469/193039199_mp4_h264_aac_hd_7.ts
#EXT-X-CUE-IN

#EXTINF:10.0,
https://test-streams.mux.dev/x36xhzz/url_0/url_470/193039199_mp4_h264_aac_hd_7.ts
#EXTINF:10.0,
https://test-streams.mux.dev/x36xhzz/url_0/url_471/193039199_mp4_h264_aac_hd_7.ts
#EXTINF:10.0,
https://test-streams.mux.dev/x36xhzz/url_0/url_472/193039199_mp4_h264_aac_hd_7.ts

#EXT-X-ENDLIST
"#;

    info!("Demo playlist: 11 segments, 1 ad break (30s) at segments 5-7");

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
        playlist,
    )
        .into_response()
}
