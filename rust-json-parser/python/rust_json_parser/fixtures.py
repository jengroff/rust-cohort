"""Benchmark fixtures: synthetic generators + real-world loaders.

Two flavours of input:

* **Synthetic** — built from `_make_user(i)` records, sized in tiers
  (Small/Medium/Large/XLarge/Deeply-nested). Useful because we know
  exactly what's in them, can scale them, and they exercise specific
  parser paths (small ints, short strings, nested objects).

* **Real-world** — `citm_catalog.json`, `canada.json`, `twitter.json`
  pulled from upstream and cached in `benchmarks/fixtures/`. These are
  the canonical files every JSON parser benchmarks against (orjson,
  simdjson, msgspec, yyjson all use them), so our numbers can be
  compared one-for-one with the rest of the ecosystem.

The two flavours stress different things:

| Synthetic                          | Real-world                          |
|------------------------------------|-------------------------------------|
| Uniform shape per record           | Heterogeneous, irregular            |
| Small ints, short strings dominant | Floats (canada), long arrays (citm) |
| Predictable allocation pattern     | Pathological cases at the edges     |

Both belong in the suite. Reporting only one biases the story.
"""

from __future__ import annotations

import json as _stdlib_json
from dataclasses import dataclass
from pathlib import Path

# Real-world fixtures live alongside the repo, two levels up from this file:
# rust-json-parser/benchmarks/fixtures/<name>.json
_REPO_ROOT = Path(__file__).resolve().parents[2]
REAL_FIXTURES_DIR = _REPO_ROOT / "benchmarks" / "fixtures"


@dataclass(frozen=True)
class Fixture:
    """One benchmark input: a labelled JSON string."""

    label: str
    json_str: str
    notes: str = ""

    @property
    def size_bytes(self) -> int:
        # Bytes, not chars — matters for non-ASCII fixtures (canada.json
        # is mostly ASCII so the gap is small, but it's the honest unit
        # for throughput calculations).
        return len(self.json_str.encode("utf-8"))


# ---------------------------------------------------------------------------
# Synthetic fixtures
# ---------------------------------------------------------------------------


def _make_user(i: int) -> dict:
    """A realistic per-record shape: strings, ints, a float, a bool, nested.

    Diversified deliberately. The earliest version of this benchmark
    used records that were just `{"id": int, "name": "itemN"}` —
    every value hit the parser's tuned paths (small-int cache,
    `Cow::Borrowed` on clean ASCII keys, no escapes). A benchmark
    composed entirely of best-cases tells you how the best case
    performs, not how realistic input does. This shape adds a float,
    a bool, and a nested sub-object so the slow paths get exercised
    too.
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
    """Build a deeply-nested JSON document as a string.

    `_stdlib_json.dumps` would hit Python's own recursion limit
    building 228 levels from a Python object, so we concatenate the
    string directly. Depth is chosen to stay below simplejson's
    pure-Python recursion ceiling (~498).
    """
    parts = [
        f'{{"level": {i}, "data": "value_{i}", "child": ' for i in range(depth)
    ]
    parts.append(f'{{"level": {depth}, "data": "value_{depth}", "child": null}}')
    parts.append("}" * depth)
    return "".join(parts)


def synthetic_fixtures() -> list[Fixture]:
    """Return the five synthetic fixtures in size order."""
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

    return [
        Fixture("Small (synthetic)", small, "single object, mixed types"),
        Fixture("Medium (synthetic)", medium, "75 user records"),
        Fixture("Large (synthetic)", large, "750 user records"),
        Fixture("XLarge (synthetic)", xlarge, "3500 user records"),
        Fixture("Deeply nested (synthetic)", nested, "228 levels of nesting"),
    ]


# ---------------------------------------------------------------------------
# Real-world fixtures
# ---------------------------------------------------------------------------


_REAL_FIXTURE_FILES: list[tuple[str, str, str]] = [
    # (label, filename, notes)
    ("citm_catalog (real)", "citm_catalog.json", "concert listings, mostly small ints + short strings"),
    ("canada (real)", "canada.json", "geographic coordinates, float-heavy"),
    ("twitter (real)", "twitter.json", "Twitter API response, mixed unicode + escapes"),
]


def real_fixtures() -> list[Fixture]:
    """Load the canonical real-world JSON benchmark files.

    If the files aren't present (e.g. fresh clone, no
    `benchmarks/download_fixtures.py` run yet), return an empty list
    rather than crashing — the synthetic suite still works on its own.
    """
    out: list[Fixture] = []
    for label, filename, notes in _REAL_FIXTURE_FILES:
        path = REAL_FIXTURES_DIR / filename
        if not path.exists():
            continue
        out.append(Fixture(label, path.read_text(encoding="utf-8"), notes))
    return out


def all_fixtures() -> list[Fixture]:
    """Synthetic followed by real-world, in benchmark display order."""
    return synthetic_fixtures() + real_fixtures()
