import sys
import os.path
from rust_json_parser import parse_json, parse_json_file, dumps, benchmark_performance


def run_benchmarks() -> None:
    """Compare Rust parser against json (C) and simplejson (pure Python)."""
    inputs = [
        ("Small JSON", '{"key": "value", "n": 42}'),
        ("Medium JSON", '{"items": ' + str([{"id": i, "name": f"item{i}"} for i in range(100)]).replace("'", '"') + '}'),
        ("Large JSON", '{"items": ' + str([{"id": i, "name": f"item{i}"} for i in range(2000)]).replace("'", '"') + '}'),
    ]

    for label, data in inputs:
        rust_t, json_t, simplejson_t = benchmark_performance(data)
        json_ratio = json_t / rust_t
        simplejson_ratio = simplejson_t / rust_t
        json_word = "faster" if json_ratio > 1 else "slower"
        simplejson_word = "faster" if simplejson_ratio > 1 else "slower"
        print(f"\n{label} ({len(data)} bytes):")
        print(f"  Rust:            {rust_t:.6f}s")
        print(f"  Python json (C): {json_t:.6f}s  (Rust is {json_ratio:.2f}x {json_word})")
        print(f"  simplejson:      {simplejson_t:.6f}s  (Rust is {simplejson_ratio:.2f}x {simplejson_word})")


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