import datetime
import json as _stdlib_json
import os.path
import platform
import sys
from rust_json_parser import parse_json, parse_json_file, dumps, benchmark_performance


def _force_pure_python_simplejson() -> bool:
    """Disable simplejson's C accelerators for the rest of the process.

    simplejson ships a `_speedups` C extension; when it's installed (which
    is the default for binary wheels), `simplejson.loads` is almost as fast
    as CPython's `json.loads`. That makes the simplejson column in our
    benchmark measure Rust-vs-C, not Rust-vs-pure-Python.

    We build a `JSONDecoder` and swap out its scanner/scanstring for the
    pure-Python implementations, then replace `simplejson.loads` at the
    module level. PyO3's `py.import("simplejson")` returns the same module
    object, so the Rust-side benchmark picks up the patched `loads`.
    """
    import simplejson
    try:
        from simplejson.decoder import JSONDecoder, py_scanstring
        from simplejson.scanner import py_make_scanner
    except ImportError:
        return False
    try:
        dec = JSONDecoder()
        dec.parse_string = py_scanstring
        dec.scan_once = py_make_scanner(dec)
        simplejson.loads = dec.decode
    except Exception:
        return False
    return True


def _make_user(i: int) -> dict:
    """Realistic per-item shape: strings, ints, a float, a bool, a nested object.

    Matches 13hulk's fixture shape closely enough for apples-to-apples, and
    exercises every parse path (not just `int` + short ASCII string, which
    is where our optimisations look artificially good).
    """
    return {
        "id": i,
        "name": f"user_{i}",
        "email": f"user_{i}@example.com",
        "age": 25 + (i % 50),
        "score": 10.0 + (i % 90) + 0.5,
        "active": i % 2 == 0,
        "address": {
            "city": f"City_{i}",
            "zip": f"{10000 + i}",
        },
    }


def _build_nested(depth: int) -> str:
    """Build a deeply nested JSON document as a string.

    `json.dumps` would hit Python's own recursion limit building 228 levels
    from a Python object, so we concatenate the string directly. Depth is
    chosen to stay below simplejson's pure-Python recursion ceiling (~498).
    """
    parts = [
        f'{{"level": {i}, "data": "value_{i}", "child": ' for i in range(depth)
    ]
    parts.append(f'{{"level": {depth}, "data": "value_{depth}", "child": null}}')
    parts.append("}" * depth)
    return "".join(parts)


def _ratio_label(other: float, rust: float) -> str:
    if rust <= 0:
        return "n/a"
    ratio = other / rust
    if ratio >= 1:
        return f"{ratio:.2f}× faster"
    return f"{1 / ratio:.2f}× slower"


def run_benchmarks() -> None:
    """Compare Rust parser against json (C) and simplejson (pure Python)."""
    pure_python = _force_pure_python_simplejson()
    simplejson_mode = (
        "pure-Python (C speedups disabled)"
        if pure_python
        else "C speedups ACTIVE — simplejson column is Rust-vs-C, not Rust-vs-pure-Python"
    )

    small = _stdlib_json.dumps({
        "name": "Alice",
        "age": 30,
        "email": "alice@example.com",
        "active": True,
        "score": 95.5,
        "city": "Portland",
    })
    medium = _stdlib_json.dumps([_make_user(i) for i in range(75)])
    large = _stdlib_json.dumps([_make_user(i) for i in range(750)])
    xlarge = _stdlib_json.dumps([_make_user(i) for i in range(3500)])
    nested = _build_nested(228)

    inputs = [
        ("Small", small),
        ("Medium", medium),
        ("Large", large),
        ("XLarge", xlarge),
        ("Deeply nested", nested),
    ]
    iteration_counts = (1000,)

    print(f"simplejson mode: {simplejson_mode}")
    print(f"iterations: {', '.join(str(n) for n in iteration_counts)}")

    header = (
        f"\n{'Input':<26} {'Iters':>6}  "
        f"{'Rust':>11}  {'json (C)':>11}  {'simplejson':>12}  "
        f"{'Rust vs json':<16} {'Rust vs simplejson':<18}"
    )
    print(header)
    print("-" * (len(header) - 1))

    rows = []
    for label, data in inputs:
        label_with_size = f"{label} ({len(data):,} B)"
        for iters in iteration_counts:
            rust_t, json_t, simplejson_t = benchmark_performance(data, iterations=iters)
            rows.append((label, len(data), iters, rust_t, json_t, simplejson_t))
            print(
                f"{label_with_size:<26} {iters:>6,}  "
                f"{rust_t:>10.6f}s  "
                f"{json_t:>10.6f}s  "
                f"{simplejson_t:>11.6f}s  "
                f"{_ratio_label(json_t, rust_t):<16} "
                f"{_ratio_label(simplejson_t, rust_t):<18}"
            )

    out_path = os.path.abspath("benchmark_results.md")
    _write_results_md(out_path, rows, simplejson_mode)
    print(f"\nWrote {out_path}")


def _write_results_md(path: str, rows: list, simplejson_mode: str) -> None:
    now = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    lines = [
        "# Benchmark Results",
        "",
        f"**Date:** {now}  ",
        f"**Platform:** {platform.platform()}  ",
        f"**Python:** {platform.python_version()}  ",
        f"**Build:** release (requires `maturin develop --release`)  ",
        f"**simplejson mode:** {simplejson_mode}  ",
        "",
        "## Results",
        "",
        "| Input | Size | Iterations | Rust (s) | Python json (s) | simplejson (s) | Rust vs json | Rust vs simplejson |",
        "|-------|-----:|-----------:|---------:|----------------:|---------------:|:-------------|:-------------------|",
    ]
    for label, size, iters, rust_t, json_t, simplejson_t in rows:
        lines.append(
            f"| {label} | {size:,} B | {iters:,} | "
            f"{rust_t:.6f} | {json_t:.6f} | {simplejson_t:.6f} | "
            f"{_ratio_label(json_t, rust_t)} | {_ratio_label(simplejson_t, rust_t)} |"
        )

    lines += ["", "## Per-iteration totals", ""]
    lines.append("| Iterations | Rust total (s) | Python json total (s) | simplejson total (s) |")
    lines.append("|-----------:|---------------:|----------------------:|---------------------:|")
    iter_counts = sorted({r[2] for r in rows})
    for iters in iter_counts:
        rust_sum = sum(r[3] for r in rows if r[2] == iters)
        json_sum = sum(r[4] for r in rows if r[2] == iters)
        sj_sum = sum(r[5] for r in rows if r[2] == iters)
        lines.append(f"| {iters:,} | {rust_sum:.6f} | {json_sum:.6f} | {sj_sum:.6f} |")

    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    if "--benchmark" in sys.argv:
        run_benchmarks()
        sys.exit(0)

    if len(sys.argv) < 2:
        print("Usage: python -m rust_json_parser <file_or_json_string>")
        print("       python -m rust_json_parser --benchmark")
        sys.exit(1)

    arg = sys.argv[1]
    try:
        if os.path.exists(arg):
            result = parse_json_file(arg)
        else:
            result = parse_json(arg)
        print(dumps(result, indent=2))
    except ValueError as e:
        print(f"Parse error: {e}", file=sys.stderr)
        sys.exit(1)
    except IOError as e:
        print(f"File error: {e}", file=sys.stderr)
        sys.exit(1)
