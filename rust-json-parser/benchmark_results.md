# Benchmark Results

**Started:** 2026-04-25T02:10:25  
**Finished:** 2026-04-25T02:12:50  
**Platform:** Linux-6.6.87.2-microsoft-standard-WSL2-x86_64-with-glibc2.35  
**Python:** 3.14.3  
**Build:** release (requires `maturin develop --release` or `uv sync`)  
**simplejson mode:** pure-Python (C speedups disabled)  

## Methodology

For each (parser, fixture): 3 warmup batches (discarded), then 30 timed batches. Batch size adapts so each batch takes ~100 ms; per-iteration time is `batch_time / batch_size`. Stats reported are computed over the 30 samples. Throughput is `fixture_bytes / median_seconds`.

`json` is the reference for the "vs json" column — it's the parser every Python user has by default.

## Small (synthetic) — 109 B — single object, mixed types

| Parser | Median (ms) | p95 (ms) | Stddev (ms) | Throughput (MB/s) | vs json (C) |
|--------|------------:|---------:|------------:|------------------:|-------------|
| msgspec (C) | 0.000 | 0.000 | 0.000 | 286 | 4.74× faster |
| orjson (Rust+PyO3) | 0.000 | 0.001 | 0.000 | 255 | 4.23× faster |
| **rust_json_parser (us)** | 0.001 | 0.001 | 0.000 | 146 | 2.43× faster |
| ujson (C) | 0.001 | 0.001 | 0.000 | 113 | 1.87× faster |
| json (stdlib C) | 0.002 | 0.002 | 0.000 | 60 | baseline |
| simplejson (pure Python) | 0.009 | 0.012 | 0.002 | 12 | 5.08× slower |

## Medium (synthetic) — 11.5 KB — 75 user records

| Parser | Median (ms) | p95 (ms) | Stddev (ms) | Throughput (MB/s) | vs json (C) |
|--------|------------:|---------:|------------:|------------------:|-------------|
| orjson (Rust+PyO3) | 0.038 | 0.046 | 0.003 | 305 | 2.15× faster |
| msgspec (C) | 0.046 | 0.049 | 0.004 | 251 | 1.77× faster |
| **rust_json_parser (us)** | 0.058 | 0.078 | 0.008 | 199 | 1.40× faster |
| ujson (C) | 0.065 | 0.080 | 0.008 | 177 | 1.25× faster |
| json (stdlib C) | 0.081 | 0.098 | 0.007 | 142 | baseline |
| simplejson (pure Python) | 0.978 | 1.235 | 0.128 | 12 | 12.10× slower |

## Large (synthetic) — 117.7 KB — 750 user records

| Parser | Median (ms) | p95 (ms) | Stddev (ms) | Throughput (MB/s) | vs json (C) |
|--------|------------:|---------:|------------:|------------------:|-------------|
| orjson (Rust+PyO3) | 0.378 | 0.473 | 0.033 | 311 | 2.15× faster |
| msgspec (C) | 0.389 | 0.462 | 0.037 | 302 | 2.09× faster |
| ujson (C) | 0.660 | 0.811 | 0.074 | 178 | 1.23× faster |
| **rust_json_parser (us)** | 0.665 | 0.896 | 0.094 | 177 | 1.22× faster |
| json (stdlib C) | 0.813 | 0.942 | 0.057 | 145 | baseline |
| simplejson (pure Python) | 8.923 | 12.477 | 1.161 | 13 | 10.98× slower |

## XLarge (synthetic) — 560.8 KB — 3500 user records

| Parser | Median (ms) | p95 (ms) | Stddev (ms) | Throughput (MB/s) | vs json (C) |
|--------|------------:|---------:|------------:|------------------:|-------------|
| msgspec (C) | 2.159 | 2.907 | 0.317 | 260 | 2.11× faster |
| orjson (Rust+PyO3) | 2.304 | 3.069 | 0.294 | 243 | 1.98× faster |
| ujson (C) | 3.565 | 5.635 | 0.782 | 157 | 1.28× faster |
| **rust_json_parser (us)** | 4.276 | 5.292 | 0.639 | 131 | 1.07× faster |
| json (stdlib C) | 4.554 | 6.052 | 0.659 | 123 | baseline |
| simplejson (pure Python) | 47.720 | 62.198 | 6.969 | 12 | 10.48× slower |

## Deeply nested (synthetic) — 10.3 KB — 228 levels of nesting

| Parser | Median (ms) | p95 (ms) | Stddev (ms) | Throughput (MB/s) | vs json (C) |
|--------|------------:|---------:|------------:|------------------:|-------------|
| orjson (Rust+PyO3) | 0.036 | 0.045 | 0.004 | 290 | 2.17× faster |
| msgspec (C) | 0.038 | 0.052 | 0.005 | 268 | 2.00× faster |
| **rust_json_parser (us)** | 0.059 | 0.076 | 0.007 | 174 | 1.30× faster |
| ujson (C) | 0.070 | 0.080 | 0.010 | 146 | 1.09× faster |
| json (stdlib C) | 0.077 | 0.100 | 0.010 | 134 | baseline |
| simplejson (pure Python) | 1.306 | 1.685 | 0.222 | 8 | 16.93× slower |

## citm_catalog (real) — 1.7 MB — concert listings, mostly small ints + short strings

| Parser | Median (ms) | p95 (ms) | Stddev (ms) | Throughput (MB/s) | vs json (C) |
|--------|------------:|---------:|------------:|------------------:|-------------|
| orjson (Rust+PyO3) | 3.909 | 4.734 | 0.403 | 442 | 2.33× faster |
| ujson (C) | 4.532 | 9.339 | 1.466 | 381 | 2.01× faster |
| **rust_json_parser (us)** | 5.016 | 8.282 | 1.134 | 344 | 1.82× faster |
| msgspec (C) | 5.119 | 5.965 | 0.528 | 337 | 1.78× faster |
| json (stdlib C) | 9.117 | 10.347 | 0.929 | 189 | baseline |
| simplejson (pure Python) | 78.773 | 96.604 | 15.456 | 22 | 8.64× slower |

## canada (real) — 2.3 MB — geographic coordinates, float-heavy

| Parser | Median (ms) | p95 (ms) | Stddev (ms) | Throughput (MB/s) | vs json (C) |
|--------|------------:|---------:|------------:|------------------:|-------------|
| orjson (Rust+PyO3) | 8.721 | 11.370 | 1.506 | 258 | 3.89× faster |
| msgspec (C) | 10.771 | 13.999 | 1.540 | 209 | 3.15× faster |
| **rust_json_parser (us)** | 11.456 | 15.409 | 1.778 | 196 | 2.96× faster |
| ujson (C) | 18.561 | 20.616 | 2.037 | 121 | 1.83× faster |
| json (stdlib C) | 33.939 | 41.452 | 3.373 | 66 | baseline |
| simplejson (pure Python) | 174.474 | 202.460 | 13.749 | 13 | 5.14× slower |

## twitter (real) — 631.5 KB — Twitter API response, mixed unicode + escapes

| Parser | Median (ms) | p95 (ms) | Stddev (ms) | Throughput (MB/s) | vs json (C) |
|--------|------------:|---------:|------------:|------------------:|-------------|
| orjson (Rust+PyO3) | 1.216 | 1.508 | 0.139 | 519 | 2.97× faster |
| msgspec (C) | 1.494 | 1.797 | 0.126 | 423 | 2.42× faster |
| ujson (C) | 2.233 | 3.033 | 0.349 | 283 | 1.62× faster |
| **rust_json_parser (us)** | 2.553 | 3.020 | 0.270 | 247 | 1.42× faster |
| json (stdlib C) | 3.614 | 4.195 | 0.276 | 175 | baseline |
| simplejson (pure Python) | 25.867 | 30.977 | 3.291 | 24 | 7.16× slower |

