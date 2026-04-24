# Benchmark Results

**Date:** 2026-04-23 16:25:33  
**Platform:** Linux-6.6.87.2-microsoft-standard-WSL2-x86_64-with-glibc2.35  
**Python:** 3.14.3  
**Build:** release (requires `maturin develop --release`)  
**simplejson mode:** pure-Python (C speedups disabled)  

## Results

| Input | Size | Iterations | Rust (s) | Python json (s) | simplejson (s) | Rust vs json | Rust vs simplejson |
|-------|-----:|-----------:|---------:|----------------:|---------------:|:-------------|:-------------------|
| Small | 109 B | 1,000 | 0.000495 | 0.001710 | 0.009240 | 3.45× faster | 18.65× faster |
| Medium | 11,472 B | 1,000 | 0.063605 | 0.094425 | 0.935464 | 1.48× faster | 14.71× faster |
| Large | 117,685 B | 1,000 | 0.676518 | 1.015580 | 9.225854 | 1.50× faster | 13.64× faster |
| XLarge | 560,810 B | 1,000 | 4.549820 | 4.586158 | 45.650102 | 1.01× faster | 10.03× faster |
| Deeply nested | 10,318 B | 1,000 | 0.065342 | 0.088519 | 1.294285 | 1.35× faster | 19.81× faster |

## Per-iteration totals

| Iterations | Rust total (s) | Python json total (s) | simplejson total (s) |
|-----------:|---------------:|----------------------:|---------------------:|
| 1,000 | 5.355780 | 5.786392 | 57.114945 |
