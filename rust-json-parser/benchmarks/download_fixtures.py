"""Download standard JSON benchmark fixtures.

These three files are the de facto standard for JSON parser
benchmarking across the ecosystem (orjson, msgspec, simdjson, yyjson
all benchmark against them). Bundled in our repo so the benchmark
runs offline and against fixed, well-known inputs.

Run once after cloning:

    python benchmarks/download_fixtures.py

The files land in benchmarks/fixtures/ and should be committed.

Sources differ because canada.json is no longer in the simdjson repo
but lives on at serde-rs/json-benchmark. The hashes below pin specific
versions so future moves don't silently change what we benchmark on.
"""

from __future__ import annotations

import hashlib
import sys
import urllib.request
from pathlib import Path

FIXTURES_DIR = Path(__file__).parent / "fixtures"

# (filename, source URL, expected sha256). Hashes filled in on first
# successful download; if upstream moves, we'll catch it.
FIXTURES: list[tuple[str, str, str]] = [
    (
        "citm_catalog.json",
        "https://raw.githubusercontent.com/simdjson/simdjson/master/jsonexamples/citm_catalog.json",
        "a73e7a883f6ea8de113dff59702975e60119b4b58d451d518a929f31c92e2059",
    ),
    (
        "canada.json",
        "https://raw.githubusercontent.com/serde-rs/json-benchmark/master/data/canada.json",
        "f83b3b354030d5dd58740c68ac4fecef64cb730a0d12a90362a7f23077f50d78",
    ),
    (
        "twitter.json",
        "https://raw.githubusercontent.com/simdjson/simdjson/master/jsonexamples/twitter.json",
        "30721e496a8d73cfc50658923c34eb2c0fbe15ee6835005e43ee624d8dedf200",
    ),
]


def download(name: str, url: str, expected_sha: str) -> None:
    target = FIXTURES_DIR / name
    if target.exists():
        actual = hashlib.sha256(target.read_bytes()).hexdigest()
        if expected_sha and actual != expected_sha:
            print(f"  ! {name} exists but sha256 mismatch (got {actual[:12]}, "
                  f"expected {expected_sha[:12]}); re-downloading")
        else:
            print(f"  ✓ {name} already present ({target.stat().st_size:,} bytes)")
            return

    print(f"  ↓ {name} from {url}", flush=True)
    urllib.request.urlretrieve(url, target)
    size = target.stat().st_size
    sha = hashlib.sha256(target.read_bytes()).hexdigest()
    print(f"    {size:,} bytes, sha256 {sha[:12]}…")


def main() -> int:
    FIXTURES_DIR.mkdir(exist_ok=True, parents=True)
    print(f"Downloading benchmark fixtures into {FIXTURES_DIR}/")
    for name, url, expected_sha in FIXTURES:
        try:
            download(name, url, expected_sha)
        except Exception as exc:
            print(f"  × {name} failed: {exc}", file=sys.stderr)
            return 1
    print("Done.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
