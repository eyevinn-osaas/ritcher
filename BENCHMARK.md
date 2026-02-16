# Ritcher Benchmark Results

Detailed performance analysis of Ritcher's manifest stitching pipeline and VAST parsing.

For a summary, see the [Performance section in README](README.md#performance).

## Why Performance Matters in SSAI

In Server-Side Ad Insertion, every concurrent viewer receives a **personalized manifest** — the stitcher rewrites each playlist with viewer-specific ad segment URLs. Unlike content segments (which are CDN-cacheable), manifests must be generated per request.

The fundamental equation:

```
required_manifest_RPS = concurrent_viewers / segment_duration
```

| Concurrent Viewers | Segment Duration | Required RPS |
|---|---|---|
| 1,000 | 6s | ~167 |
| 10,000 | 6s | ~1,667 |
| 100,000 | 6s | ~16,667 |
| 1,000,000 | 6s | ~166,667 |

This makes the manifest pipeline the critical scalability bottleneck. Every microsecond saved translates directly to more viewers per instance.

---

## Test Environment

- **CPU**: Apple M-series (ARM64)
- **Rust**: Edition 2024, release profile (optimized)
- **Tool**: [Criterion.rs](https://github.com/bheisler/criterion.rs) v0.5
- **Methodology**: 100 samples per benchmark, statistical analysis with outlier detection

Reproduce these results:

```bash
# All benchmarks
cargo bench

# Manifest pipeline only
cargo bench --bench manifest_pipeline

# VAST parsing only
cargo bench --bench vast_parsing
```

---

## Full Pipeline Benchmark

**The critical benchmark.** Measures the complete manifest processing path that each viewer incurs on every playlist request:

```
parse HLS → detect CUE breaks → interleave ad segments → rewrite URLs → serialize
```

| Scenario | Segments | Ad Breaks | Time (p50) | Throughput |
|---|---|---|---|---|
| Typical live | 6 | 1 | **6.4 µs** | **156K ops/sec** |
| Medium window | 15 | 1 | 11.9 µs | 84K ops/sec |
| DVR / catchup | 60 | 3 | 44.3 µs | 23K ops/sec |
| Pass-through (no ads) | 12 | 0 | 7.3 µs | 137K ops/sec |

### Scaling Estimates

Based on the typical live scenario (~6 µs per manifest, 6-second segments):

| Cores | Manifest RPS | Concurrent Viewers |
|---|---|---|
| 1 | ~156,000 | ~936,000 |
| 2 | ~312,000 | ~1,870,000 |
| 4 | ~624,000 | ~3,740,000 |
| 8 | ~1,248,000 | ~7,490,000 |

> **Note:** These are theoretical maximums from CPU-only benchmarks. Real-world throughput will be lower due to network I/O, VAST fetches, memory allocation, and OS scheduling. However, VAST responses are cached per ad break (not per viewer), so the CPU-bound manifest pipeline remains the dominant factor during ad break storms.

---

## Pipeline Stage Breakdown

### Parse HLS Playlist

Parsing raw M3U8 text into structured data using `m3u8-rs`:

| Playlist Size | Segments | Time |
|---|---|---|
| Small (live window) | 6 | 3.5 µs |
| Medium | 12 | 5.2 µs |
| Large | 30 | 10.6 µs |
| Very large (DVR) | 60 | 19.7 µs |

Scales linearly with segment count (~330 ns per segment).

### Detect CUE Ad Breaks

Scanning `EXT-X-CUE-OUT` / `EXT-X-CUE-IN` / `EXT-X-CUE-OUT-CONT` tags:

| Ad Breaks | Time |
|---|---|
| 1 | 53 ns |
| 3 | 95 ns |
| 5 | 164 ns |

Effectively free — single-pass scan over segment tags with no allocations.

### Interleave Ad Segments

Replacing content segments within ad break windows with ad segments and adding `EXT-X-DISCONTINUITY` markers:

| Ad Breaks | Ad Segments | Time |
|---|---|---|
| 1 | 5 | 4.1 µs |
| 3 | 15 | 7.2 µs |

Dominated by segment cloning and URL string formatting.

### Rewrite Content URLs

Rewriting segment URIs to route through the stitcher proxy:

| Segments | Time |
|---|---|
| 6 | 1.3 µs |
| 30 | 6.2 µs |
| 60 | 12.6 µs |

Linear scaling (~210 ns per segment) — string formatting with `format!()`.

### Rewrite Master Playlist URLs

Rewriting variant-stream URIs for multi-quality stitching:

| Variants | Time |
|---|---|
| 3 | 848 ns |
| 5 | 1.4 µs |
| 7 | 1.9 µs |

~270 ns per variant. Master playlist rewriting happens once per session refresh, not per media playlist request.

### Serialize Playlist

Converting structured playlist back to M3U8 text:

| Segments | Time |
|---|---|
| 6 | 1.4 µs |
| 30 | 3.9 µs |
| 60 | 7.3 µs |

~120 ns per segment — the fastest pipeline stage.

---

## VAST XML Parsing

VAST parsing happens when a viewer enters an ad break. In production, responses are cached per ad break, so this cost is amortized across all concurrent viewers.

### By Ad Count

| Ads | Media Files/Ad | XML Size | Time |
|---|---|---|---|
| 1 | 3 | ~1.8 KB | 6.3 µs |
| 3 | 3 | ~5.3 KB | 18.0 µs |
| 5 | 3 | ~8.7 KB | 32.1 µs |
| 10 | 3 | ~17 KB | 64.5 µs |

Scales linearly with ad count (~6 µs per ad).

### By Media File Count

| Media Files | Time |
|---|---|
| 1 | 3.9 µs |
| 3 | 6.2 µs |
| 5 | 8.6 µs |

~1.2 µs per additional media file.

### Special Cases

| Scenario | Time |
|---|---|
| Empty VAST (no-fill) | 195 ns |
| Wrapper (redirect) | 1.6 µs |
| Media file selection (5 candidates) | 3.4 ns |

Empty VAST responses are nearly free, which matters because no-fill rates can be 30-50% in programmatic advertising.

---

## Ad Break Storm Analysis

The worst-case scenario in live SSAI is when **all viewers hit an ad break simultaneously** — every manifest request requires both VAST parsing and ad interleaving.

For a typical live scenario (6 segments, 1 ad break, 3-ad VAST pod):

```
Manifest pipeline:  ~6 µs
VAST parsing:       ~18 µs (cached after first viewer)
Total (first):      ~24 µs
Total (subsequent): ~6 µs  (VAST cached)
```

Even during an ad break storm, the per-viewer cost remains ~6 µs since VAST is cached. The single-viewer VAST parse adds only ~18 µs of latency to the first request.

---

## Comparison Context

No direct apples-to-apples benchmarks exist for other open-source stitchers, but for context:

- **Node.js manifest manipulation** (e.g., `@eyevinn/hls-splice`): Typically measured in milliseconds due to V8 JIT compilation, garbage collection pauses, and single-threaded execution
- **Ritcher (Rust)**: Measured in microseconds with zero GC pauses and multi-core parallelism via Tokio

The ~1000x difference in overhead is expected for CPU-bound string processing workloads comparing native compiled code to interpreted/JIT runtimes.

> We welcome benchmark contributions from other SSAI implementations for fair comparison. Open an issue or PR with your results.

---

## Running Benchmarks

```bash
# Full benchmark suite with HTML reports
cargo bench

# Specific benchmark
cargo bench --bench manifest_pipeline
cargo bench --bench vast_parsing

# View HTML reports (generated by Criterion)
open target/criterion/report/index.html
```

Criterion generates detailed HTML reports with plots in `target/criterion/`. These include iteration times, throughput curves, and regression detection against previous runs.
