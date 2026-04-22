# Benchmark Results

**Date:** 2026-04-22 09:49:23  
**Platform:** Linux-6.6.87.2-microsoft-standard-WSL2-x86_64-with-glibc2.35  
**Python:** 3.14.3  
**Build:** release (requires `maturin develop --release`)  
**simplejson mode:** pure-Python (C speedups disabled)  

## Results

| Input | Size | Iterations | Rust (s) | Python json (s) | simplejson (s) | Rust vs json | Rust vs simplejson |
|-------|-----:|-----------:|---------:|----------------:|---------------:|:-------------|:-------------------|
| Small | 109 B | 1,000 | 0.000350 | 0.001335 | 0.009439 | 3.82× faster | 26.98× faster |
| Medium | 11,472 B | 1,000 | 0.076262 | 0.092064 | 0.908889 | 1.21× faster | 11.92× faster |
| Large | 117,685 B | 1,000 | 0.598332 | 0.869570 | 7.885628 | 1.45× faster | 13.18× faster |
| XLarge | 560,810 B | 1,000 | 3.219725 | 3.797877 | 36.240227 | 1.18× faster | 11.26× faster |
| Deeply nested | 10,318 B | 1,000 | 0.055264 | 0.080852 | 0.869275 | 1.46× faster | 15.73× faster |

## Per-iteration totals

| Iterations | Rust total (s) | Python json total (s) | simplejson total (s) |
|-----------:|---------------:|----------------------:|---------------------:|
| 1,000 | 3.949933 | 4.841698 | 45.913457 |
