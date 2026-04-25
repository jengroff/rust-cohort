"""Benchmark harness: timing, statistics, console + markdown output.

Why we built this instead of using `pyperf`:

* `pyperf` is the textbook-correct tool. It runs each benchmark in a
  subprocess for isolation, does CPU pinning, and outputs in a
  standard format. It also adds 30+ seconds per run, which makes
  iteration painful for a benchmark suite this size.
* For our use case — in-process timing of pure functions, six parsers
  × eight fixtures, no JIT to warm up — `time.perf_counter_ns()` plus
  Python's `statistics` module is plenty rigorous. We get median,
  p95, stddev, and min/max from the same sample set.

Methodology per (parser, fixture):

1. **Calibration.** Time 10 iterations to estimate single-iteration
   cost. From that, pick `batch_size` so each batch takes ~100 ms.
2. **Warmup.** Run 3 untimed batches. Discarded.
3. **Measurement.** Run 30 timed batches. Each batch produces one
   sample (its mean per-iteration time).
4. **Stats.** Median, p95, stddev, min computed over the 30 samples.
5. **Throughput.** `bytes(input_utf8) / median_seconds`, reported as
   MB/s.

Each parser × fixture has a hard 30-second budget. If calibration or
measurement exceeds it, the cell is reported as "skipped (too slow)"
and the run continues. (Looking at you, simplejson on the XLarge
fixture.)
"""

from __future__ import annotations

import datetime
import platform
import statistics
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable

from . import fixtures as _fixtures

# ---------------------------------------------------------------------------
# Parser registry
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Parser:
    """One competitor in the benchmark."""

    name: str            # e.g. "rust_json_parser"
    label: str           # human-readable, e.g. "rust_json_parser (us)"
    is_us: bool = False  # marks our parser for output highlighting


PARSERS: list[Parser] = [
    Parser("rust_json_parser", "rust_json_parser (us)", is_us=True),
    Parser("json", "json (stdlib C)"),
    Parser("orjson", "orjson (Rust+PyO3)"),
    Parser("msgspec", "msgspec (C)"),
    Parser("ujson", "ujson (C)"),
    Parser("simplejson", "simplejson (pure Python)"),
]

# Reference parser used for "vs X" columns in the output. json (the
# CPython stdlib C implementation) is the right reference because
# it's the parser everyone has by default — beating it is the
# floor of what counts as "faster than the standard answer".
REFERENCE_PARSER = "json"


def _resolve_loads(name: str) -> Callable[[str], object]:
    """Return the `loads`-equivalent callable for the named parser.

    Imports lazily so a missing optional dep doesn't crash the suite —
    callers handle the ImportError and report "not available".
    """
    if name == "rust_json_parser":
        from rust_json_parser import parse_json
        return parse_json
    if name == "json":
        import json
        return json.loads
    if name == "orjson":
        import orjson
        return orjson.loads
    if name == "msgspec":
        import msgspec.json
        return msgspec.json.decode
    if name == "ujson":
        import ujson
        return ujson.loads
    if name == "simplejson":
        # simplejson.loads should already be patched to the pure-Python
        # path by run() before we time it; see _force_pure_python_simplejson.
        import simplejson
        return simplejson.loads
    raise ValueError(f"unknown parser: {name!r}")


def _force_pure_python_simplejson() -> bool:
    """Disable simplejson's C accelerator for the rest of the process.

    simplejson ships a `_speedups` C extension that `simplejson.loads`
    picks up automatically when present. Without disabling it, the
    simplejson column measures Rust-vs-C, not Rust-vs-pure-Python —
    which defeats the point of having simplejson in the comparison.

    Mirrors the approach that lived in the old __main__.py.
    """
    try:
        import simplejson
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


# ---------------------------------------------------------------------------
# Timing primitives
# ---------------------------------------------------------------------------

# Tuning knobs. Exposed as module constants so tests / experiments can
# adjust without touching the timing code.
TARGET_BATCH_NS = 100_000_000      # 100 ms per batch
WARMUP_BATCHES = 3
MEASUREMENT_BATCHES = 30
PER_CELL_BUDGET_S = 30.0           # max wall-clock for one (parser, fixture)


@dataclass(frozen=True)
class Result:
    """One (parser, fixture) measurement."""

    parser: str
    fixture: str
    size_bytes: int
    # Per-iteration stats, all in seconds.
    median_s: float
    p95_s: float
    stddev_s: float
    min_s: float
    samples: int
    iterations_per_sample: int
    # Errors / skips. If `error` is set the rest of the fields above
    # are placeholders and shouldn't be displayed as numbers.
    error: str = ""

    @property
    def throughput_mb_s(self) -> float:
        if self.median_s <= 0:
            return 0.0
        return (self.size_bytes / self.median_s) / 1_000_000

    @property
    def ok(self) -> bool:
        return not self.error


def _calibrate_batch_size(loads: Callable, json_str: str) -> int:
    """Pick a batch size that should take ~TARGET_BATCH_NS per batch.

    Times 10 iterations once, scales up. Floored at 1 (in case the
    operation is so slow that even one iteration overruns the
    target — XLarge × simplejson, looking at you).
    """
    start = time.perf_counter_ns()
    for _ in range(10):
        loads(json_str)
    elapsed = time.perf_counter_ns() - start
    per_iter_ns = max(1, elapsed // 10)
    batch_size = TARGET_BATCH_NS // per_iter_ns
    return max(1, batch_size)


def _time_one(loads: Callable, json_str: str) -> Result:
    """Run the full warmup+measurement protocol for one (parser, fixture).

    Returns a Result with stats filled in, OR a Result with an error
    field set if anything went sideways (slow timeout, parser raised,
    parser produced wrong output type — though we don't validate
    output here, only timing).
    """
    fixture_size = len(json_str.encode("utf-8"))
    deadline = time.perf_counter() + PER_CELL_BUDGET_S

    # Calibration.
    try:
        batch_size = _calibrate_batch_size(loads, json_str)
    except Exception as exc:
        return Result(
            parser="?", fixture="?", size_bytes=fixture_size,
            median_s=0, p95_s=0, stddev_s=0, min_s=0,
            samples=0, iterations_per_sample=0,
            error=f"calibration failed: {type(exc).__name__}: {exc}",
        )

    if time.perf_counter() > deadline:
        return Result(
            parser="?", fixture="?", size_bytes=fixture_size,
            median_s=0, p95_s=0, stddev_s=0, min_s=0,
            samples=0, iterations_per_sample=0,
            error=f"too slow (calibration alone exceeded {PER_CELL_BUDGET_S}s)",
        )

    # Warmup.
    for _ in range(WARMUP_BATCHES):
        for _ in range(batch_size):
            loads(json_str)
        if time.perf_counter() > deadline:
            return Result(
                parser="?", fixture="?", size_bytes=fixture_size,
                median_s=0, p95_s=0, stddev_s=0, min_s=0,
                samples=0, iterations_per_sample=batch_size,
                error=f"too slow (warmup exceeded {PER_CELL_BUDGET_S}s)",
            )

    # Measurement. One sample per batch = mean per-iteration time
    # over batch_size iterations.
    samples_s: list[float] = []
    for _ in range(MEASUREMENT_BATCHES):
        start = time.perf_counter_ns()
        for _ in range(batch_size):
            loads(json_str)
        elapsed_ns = time.perf_counter_ns() - start
        samples_s.append((elapsed_ns / batch_size) / 1e9)
        if time.perf_counter() > deadline:
            break  # use what we have

    if len(samples_s) < 2:
        return Result(
            parser="?", fixture="?", size_bytes=fixture_size,
            median_s=0, p95_s=0, stddev_s=0, min_s=0,
            samples=len(samples_s), iterations_per_sample=batch_size,
            error="not enough samples to compute stats",
        )

    # Stats from the samples we collected.
    median_s = statistics.median(samples_s)
    stddev_s = statistics.stdev(samples_s)
    min_s = min(samples_s)
    # statistics.quantiles requires n>=2 samples. p95 = top of the
    # 19th of 20 evenly-spaced quantiles.
    quantiles = statistics.quantiles(samples_s, n=20)
    p95_s = quantiles[18]

    return Result(
        parser="?", fixture="?", size_bytes=fixture_size,
        median_s=median_s, p95_s=p95_s, stddev_s=stddev_s, min_s=min_s,
        samples=len(samples_s), iterations_per_sample=batch_size,
    )


# ---------------------------------------------------------------------------
# Suite runner
# ---------------------------------------------------------------------------


@dataclass
class SuiteResults:
    """All measurements + run metadata."""

    started_at: str = ""
    finished_at: str = ""
    platform: str = ""
    python_version: str = ""
    simplejson_mode: str = ""
    # Indexed by (fixture_label, parser_name).
    cells: dict[tuple[str, str], Result] = field(default_factory=dict)
    fixtures: list[_fixtures.Fixture] = field(default_factory=list)
    parsers: list[Parser] = field(default_factory=list)


def run(
    fixtures: list[_fixtures.Fixture] | None = None,
    parsers: list[Parser] | None = None,
    progress: Callable[[str], None] | None = None,
) -> SuiteResults:
    """Run the full suite and return all results.

    `progress` is an optional callback for "we're now running parser X
    on fixture Y" updates — useful so the user knows the run isn't
    stuck. Defaults to silent.
    """
    fixtures = fixtures or _fixtures.all_fixtures()
    parsers = parsers or PARSERS
    progress = progress or (lambda _msg: None)

    pure_python = _force_pure_python_simplejson()
    sj_mode = (
        "pure-Python (C speedups disabled)"
        if pure_python
        else "C speedups ACTIVE — simplejson row is Rust-vs-C, not Rust-vs-pure-Python"
    )

    out = SuiteResults(
        started_at=datetime.datetime.now().isoformat(timespec="seconds"),
        platform=platform.platform(),
        python_version=platform.python_version(),
        simplejson_mode=sj_mode,
        fixtures=list(fixtures),
        parsers=list(parsers),
    )

    # Pre-resolve callables. Skip parsers that can't be imported.
    resolved: dict[str, Callable] = {}
    for p in parsers:
        try:
            resolved[p.name] = _resolve_loads(p.name)
        except ImportError as exc:
            progress(f"  ! {p.label}: not installed ({exc})")
            for fx in fixtures:
                out.cells[(fx.label, p.name)] = Result(
                    parser=p.name, fixture=fx.label, size_bytes=fx.size_bytes,
                    median_s=0, p95_s=0, stddev_s=0, min_s=0,
                    samples=0, iterations_per_sample=0,
                    error="parser not installed",
                )

    for fx in fixtures:
        progress(f"\n{fx.label} ({_format_size(fx.size_bytes)})")
        for p in parsers:
            if p.name not in resolved:
                continue  # already recorded as not-installed above
            progress(f"  … {p.label}")
            try:
                r = _time_one(resolved[p.name], fx.json_str)
            except Exception as exc:
                r = Result(
                    parser=p.name, fixture=fx.label, size_bytes=fx.size_bytes,
                    median_s=0, p95_s=0, stddev_s=0, min_s=0,
                    samples=0, iterations_per_sample=0,
                    error=f"{type(exc).__name__}: {exc}",
                )
            # _time_one doesn't know parser/fixture names; fill in here.
            r = Result(
                parser=p.name, fixture=fx.label, size_bytes=r.size_bytes,
                median_s=r.median_s, p95_s=r.p95_s, stddev_s=r.stddev_s,
                min_s=r.min_s, samples=r.samples,
                iterations_per_sample=r.iterations_per_sample,
                error=r.error,
            )
            out.cells[(fx.label, p.name)] = r

    out.finished_at = datetime.datetime.now().isoformat(timespec="seconds")
    return out


# ---------------------------------------------------------------------------
# Output formatting
# ---------------------------------------------------------------------------


def _format_size(n: int) -> str:
    if n >= 1_000_000:
        return f"{n / 1_000_000:.1f} MB"
    if n >= 1_000:
        return f"{n / 1_000:.1f} KB"
    return f"{n} B"


def _ratio_label(other_s: float, ref_s: float) -> str:
    """X.YZ× faster / slower / baseline."""
    if other_s <= 0 or ref_s <= 0:
        return "n/a"
    if other_s == ref_s:
        return "baseline"
    ratio = ref_s / other_s
    if ratio >= 1:
        return f"{ratio:.2f}× faster"
    return f"{1 / ratio:.2f}× slower"


def _sorted_for_fixture(
    results: SuiteResults, fixture_label: str
) -> list[tuple[Parser, Result]]:
    """Parser+Result pairs for one fixture, fastest-first.

    Failures sort to the bottom regardless of where they fell in
    measurement.
    """
    rows: list[tuple[Parser, Result]] = []
    for p in results.parsers:
        r = results.cells.get((fixture_label, p.name))
        if r is None:
            continue
        rows.append((p, r))
    # ok cells sorted by median (fastest-first); failures grouped at end.
    rows.sort(key=lambda pr: (not pr[1].ok, pr[1].median_s if pr[1].ok else 0))
    return rows


def format_console(results: SuiteResults) -> str:
    """Human-readable text output for the CLI."""
    lines: list[str] = []
    lines.append(f"started:  {results.started_at}")
    lines.append(f"platform: {results.platform}")
    lines.append(f"python:   {results.python_version}")
    lines.append(f"simplejson mode: {results.simplejson_mode}")
    lines.append(
        f"methodology: {MEASUREMENT_BATCHES} batches × adaptive batch size "
        f"(target ~{TARGET_BATCH_NS // 1_000_000} ms each), "
        f"{WARMUP_BATCHES} warmup batches discarded"
    )

    for fx in results.fixtures:
        size_str = _format_size(fx.size_bytes)
        lines.append("")
        lines.append(f"{fx.label} — {size_str}" + (f" — {fx.notes}" if fx.notes else ""))
        ref = results.cells.get((fx.label, REFERENCE_PARSER))
        ref_med = ref.median_s if (ref and ref.ok) else 0
        for p, r in _sorted_for_fixture(results, fx.label):
            marker = " ★" if p.is_us else "  "
            if not r.ok:
                lines.append(f" {marker} {p.label:<28} — {r.error}")
                continue
            lines.append(
                f" {marker} {p.label:<28} "
                f"{r.median_s * 1000:>8.3f} ms ± {r.stddev_s * 1000:.3f}  "
                f"p95 {r.p95_s * 1000:>7.3f}  "
                f"{r.throughput_mb_s:>7.0f} MB/s   "
                f"vs {REFERENCE_PARSER}: {_ratio_label(r.median_s, ref_med)}"
            )

    return "\n".join(lines)


def format_markdown(results: SuiteResults) -> str:
    """Timestamped report for benchmark_results.md."""
    lines: list[str] = []
    lines.append("# Benchmark Results")
    lines.append("")
    lines.append(f"**Started:** {results.started_at}  ")
    lines.append(f"**Finished:** {results.finished_at}  ")
    lines.append(f"**Platform:** {results.platform}  ")
    lines.append(f"**Python:** {results.python_version}  ")
    lines.append(f"**Build:** release (requires `maturin develop --release` or `uv sync`)  ")
    lines.append(f"**simplejson mode:** {results.simplejson_mode}  ")
    lines.append("")
    lines.append("## Methodology")
    lines.append("")
    lines.append(
        f"For each (parser, fixture): {WARMUP_BATCHES} warmup batches "
        f"(discarded), then {MEASUREMENT_BATCHES} timed batches. Batch "
        f"size adapts so each batch takes ~{TARGET_BATCH_NS // 1_000_000} ms; "
        f"per-iteration time is `batch_time / batch_size`. Stats reported are "
        f"computed over the {MEASUREMENT_BATCHES} samples. Throughput is "
        f"`fixture_bytes / median_seconds`."
    )
    lines.append("")
    lines.append(f"`{REFERENCE_PARSER}` is the reference for the "
                 f"\"vs {REFERENCE_PARSER}\" column — it's the parser every Python user "
                 f"has by default.")
    lines.append("")

    for fx in results.fixtures:
        size_str = _format_size(fx.size_bytes)
        title_extra = f" — {fx.notes}" if fx.notes else ""
        lines.append(f"## {fx.label} — {size_str}{title_extra}")
        lines.append("")
        lines.append("| Parser | Median (ms) | p95 (ms) | Stddev (ms) | Throughput (MB/s) | vs json (C) |")
        lines.append("|--------|------------:|---------:|------------:|------------------:|-------------|")
        ref = results.cells.get((fx.label, REFERENCE_PARSER))
        ref_med = ref.median_s if (ref and ref.ok) else 0
        for p, r in _sorted_for_fixture(results, fx.label):
            label = f"**{p.label}**" if p.is_us else p.label
            if not r.ok:
                lines.append(f"| {label} | — | — | — | — | {r.error} |")
                continue
            # Bold parser-name only on the "us" row — enough highlighting in
            # a sea of plain rows; bolding numbers too clutters the table.
            lines.append(
                f"| {label} | "
                f"{r.median_s * 1000:.3f} | "
                f"{r.p95_s * 1000:.3f} | "
                f"{r.stddev_s * 1000:.3f} | "
                f"{r.throughput_mb_s:.0f} | "
                f"{_ratio_label(r.median_s, ref_med)} |"
            )
        lines.append("")

    return "\n".join(lines) + "\n"


def write_markdown(results: SuiteResults, out_path: str | Path) -> Path:
    """Save the markdown report to `out_path` (suggest `benchmark_results.md`)."""
    out_path = Path(out_path)
    out_path.write_text(format_markdown(results), encoding="utf-8")
    return out_path
