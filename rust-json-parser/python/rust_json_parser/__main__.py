"""CLI for `python -m rust_json_parser`.

Two modes:

    python -m rust_json_parser '<json string>'      # parse + pretty-print
    python -m rust_json_parser path/to/file.json    # parse a file
    python -m rust_json_parser --benchmark          # run the benchmark suite

The benchmark machinery (timing, statistics, formatting) lives in
`rust_json_parser.benchmark`; this file is just the entrypoint.
"""

from __future__ import annotations

import os.path
import sys

from rust_json_parser import parse_json, parse_json_file, dumps


def _run_benchmark() -> int:
    from rust_json_parser import benchmark

    print("Running benchmark suite. This typically takes 1-3 minutes.")
    print()

    results = benchmark.run(progress=lambda msg: print(msg, flush=True))

    # Console summary.
    print()
    print("=" * 72)
    print()
    print(benchmark.format_console(results))

    # Markdown report.
    out_path = os.path.abspath("benchmark_results.md")
    benchmark.write_markdown(results, out_path)
    print()
    print(f"Wrote {out_path}")
    return 0


def _parse_arg(arg: str) -> int:
    try:
        if os.path.exists(arg):
            result = parse_json_file(arg)
        else:
            result = parse_json(arg)
        print(dumps(result, indent=2))
        return 0
    except ValueError as e:
        print(f"Parse error: {e}", file=sys.stderr)
        return 1
    except IOError as e:
        print(f"File error: {e}", file=sys.stderr)
        return 1


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)

    if "--benchmark" in argv:
        return _run_benchmark()

    if not argv:
        print("Usage: python -m rust_json_parser <file_or_json_string>")
        print("       python -m rust_json_parser --benchmark")
        return 1

    return _parse_arg(argv[0])


if __name__ == "__main__":
    sys.exit(main())
