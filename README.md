# Ritcher

## Open-Source Live SSAI Stitcher

Ritcher is a high-performance HLS stitcher built in Rust for Server-Side Ad Insertion (SSAI). It sits between the origin CDN and the player, dynamically inserting ads into live and VOD HLS streams by manipulating manifests and proxying segments.

Designed to fill the gap in the [Eyevinn Open Source Cloud](https://www.osaas.io) ecosystem as a standalone live SSAI service downstream of [Channel Engine](https://github.com/Eyevinn/channel-engine).

---

## Features

- **SCTE-35 CUE tag detection** — Detects `EXT-X-CUE-OUT`, `EXT-X-CUE-IN`, and `EXT-X-CUE-OUT-CONT` markers in HLS playlists
- **VAST ad provider** — Fetches and parses VAST 2.0/3.0/4.0 XML from any ad server, with wrapper chain support
- **Static ad provider** — Built-in provider for testing with pre-configured ad segments
- **Ad interleaving** — Replaces content segments in ad break windows with ad segments, including proper `EXT-X-DISCONTINUITY` tags
- **Segment proxying** — High-performance proxying for both content and ad segments
- **Session management** — Per-session stitching with automatic TTL-based cleanup
- **Demo endpoint** — Synthetic HLS playlist with real Mux test segments and CUE markers for testing
- **JSON health check** — Structured diagnostics with version, session count, and uptime
- **CORS support** — Permissive in dev mode, restrictive in production

---

## Architecture

```
                    +------------------+
  Player  -------->|     Ritcher      |
                    |                  |
                    |  1. Fetch playlist from origin
                    |  2. Detect SCTE-35 CUE breaks
                    |  3. Fetch ads from VAST endpoint
                    |  4. Interleave ad segments
                    |  5. Rewrite URLs through proxy
                    |  6. Serve modified playlist
                    +--------+---------+
                             |
              +--------------+--------------+
              |                             |
        Origin CDN                    Ad Server
     (content segments)            (VAST endpoint)
```

---

## Quick Start

### Prerequisites

- Rust stable (edition 2024)

### Development Mode

```bash
# Start with built-in demo and static ad provider
DEV_MODE=true cargo run

# Demo playlist (raw, no stitching):
# http://localhost:3000/demo/playlist.m3u8

# Stitched demo (with ad insertion):
# http://localhost:3000/stitch/demo/playlist.m3u8?origin=http://localhost:3000/demo/playlist.m3u8
```

### With VAST Ad Server

```bash
# Using Eyevinn test-adserver (or any VAST-compatible ad server)
DEV_MODE=true \
VAST_ENDPOINT="http://localhost:8080/api/v1/vast?dur=[DURATION]" \
cargo run
```

### Production

```bash
PORT=3000 \
BASE_URL=https://stitcher.example.com \
ORIGIN_URL=https://cdn.example.com/stream/playlist.m3u8 \
VAST_ENDPOINT=https://ads.example.com/vast \
cargo run --release
```

---

## API Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /health` | JSON health check (`{ status, version, active_sessions, uptime_seconds }`) |
| `GET /demo/playlist.m3u8` | Demo HLS playlist with CUE markers |
| `GET /stitch/{session_id}/playlist.m3u8?origin={url}` | Stitched playlist with ad insertion |
| `GET /stitch/{session_id}/segment/{*path}?origin={base}` | Proxied content segment |
| `GET /stitch/{session_id}/ad/{ad_name}` | Proxied ad segment |

---

## Configuration

| Variable | Description | Required | Default |
|----------|-------------|----------|---------|
| `DEV_MODE` | Enable dev mode with defaults | No | `false` |
| `PORT` | Server port | Prod only | `3000` |
| `BASE_URL` | Stitcher's public URL | Prod only | `http://localhost:3000` |
| `ORIGIN_URL` | Default origin playlist URL | Prod only | — |
| `AD_PROVIDER_TYPE` | `vast`, `static`, or `auto` | No | `auto` |
| `VAST_ENDPOINT` | VAST ad server URL | For VAST mode | — |
| `AD_SOURCE_URL` | Static ad segment source | For static mode | tedm.io test stream |
| `AD_SEGMENT_DURATION` | Static ad segment duration (seconds) | No | `1.0` |

**Auto-detection**: When `AD_PROVIDER_TYPE=auto` (default), Ritcher uses VAST if `VAST_ENDPOINT` is set, otherwise falls back to static.

---

## Tech Stack

- **Rust** (Edition 2024) — Zero-cost abstractions for manifest-per-viewer scalability
- **Axum 0.8** — Async HTTP server
- **Tokio** — Async runtime
- **m3u8-rs 6.0** — HLS playlist parsing
- **quick-xml** — VAST XML parsing
- **reqwest** — HTTP client with connection pooling
- **DashMap** — Lock-free concurrent session storage
- **tower-http** — CORS middleware
- **tracing** — Structured logging

---

## Testing

```bash
# Run all tests (30 tests)
cargo test

# Run with logging
RUST_LOG=debug cargo test

# Clippy
cargo clippy
```

---

## Roadmap

### Phase 1: Production-Ready HLS SSAI

- [x] HLS playlist parsing and URL rewriting
- [x] SCTE-35 CUE-OUT/CUE-IN/CUE-OUT-CONT detection
- [x] Ad interleaving with DISCONTINUITY tags
- [x] Static ad provider (testing)
- [x] VAST ad provider (VAST 2.0/3.0/4.0, wrapper chains)
- [x] Session management with background cleanup
- [x] Demo endpoint with real test segments
- [x] JSON health check with diagnostics
- [x] CORS middleware (dev/prod)
- [ ] Slate management (fallback when no ads available)
- [ ] Master playlist support
- [ ] Prometheus metrics
- [ ] Error recovery with retry logic
- [ ] Docker + Eyevinn OSC deployment

### Phase 2: DASH Support

- [ ] DASH MPD parsing and URL rewriting
- [ ] Period-based ad insertion
- [ ] CMAF segment support

### Phase 3: Advanced

- [ ] Low-latency HLS (LL-HLS)
- [ ] Server-Guided Ad Insertion (SGAI)
- [ ] Ad tracking and beaconing
- [ ] Per-viewer manifest personalization

---

## Why Ritcher?

The SSAI market is growing at 20.3% CAGR toward $14.5B by 2033, yet **no production-ready open-source live SSAI stitcher exists**. Eyevinn's OSC ecosystem has Channel Engine for FAST channel creation but lacks a standalone ad stitching service. Ritcher fills that gap with Rust performance for the CPU-bound work of generating unique manifests per viewer.

---

## Author

**Joel del Pilar** ([@JoeldelPilar](https://github.com/JoeldelPilar))

---

## Acknowledgments

Built on the shoulders of [Eyevinn Technology](https://www.eyevinntechnology.se/)'s open-source streaming ecosystem and designed for deployment on [Eyevinn Open Source Cloud](https://www.osaas.io).

---

## License

MIT License — see [LICENSE](LICENCE) file for details.
