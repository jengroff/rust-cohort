# rust-json-parser

A JSON parser built in Rust, written as part of [rust-cohort-2](https://refactorcoach.com/coaching/).

## What it does

Takes a JSON string, tokenizes it, and parses it into a `JsonValue` — supporting all six JSON types (strings, numbers, booleans, null, arrays, and objects). Ships as both a Rust crate and a Python package via [PyO3](https://pyo3.rs) + [maturin](https://www.maturin.rs).

For example, `{"name": "Alice"}` tokenizes to:

```
LeftBrace, String("name"), Colon, String("Alice"), RightBrace
```

And `"hello"` parses to:

```rust
JsonValue::Text("hello".to_string())
```

## Tokens

| Token | JSON |
|---|---|
| `LeftBrace` | `{` |
| `RightBrace` | `}` |
| `LeftBracket` | `[` |
| `RightBracket` | `]` |
| `Comma` | `,` |
| `Colon` | `:` |
| `String(s)` | `"hello"` |
| `Number(f)` | `42`, `-1.5` |
| `Boolean(b)` | `true`, `false` |
| `Null` | `null` |

## JsonValue

Parsed JSON is represented as:

```rust
JsonValue::Null
JsonValue::Boolean(bool)
JsonValue::Number(f64)
JsonValue::Text(String)
JsonValue::Array(Vec<JsonValue>)
JsonValue::Object(JsonObject)  // = FxHashMap<String, JsonValue>; see wiki for why
```

Accessor methods (`is_null`, `as_bool`, `as_f64`, `as_str`, `as_array`, `as_object`, `get`, `get_index`) return `Option<&T>` for safe, pattern-free access. `Display` produces compact JSON.

## Escape sequences

The tokenizer handles standard JSON escape sequences:

| Escape | Character |
|---|---|
| `\"` | Quote |
| `\\` | Backslash |
| `\/` | Forward slash |
| `\n` | Newline |
| `\t` | Tab |
| `\r` | Carriage return |
| `\b` | Backspace |
| `\f` | Form feed |
| `\uXXXX` | Unicode (4-digit hex) |

## Error types

All errors include a `position` field for locating issues in the input:

```rust
JsonError::UnexpectedToken { expected, found, position }
JsonError::UnexpectedEndOfInput { expected, position }
JsonError::InvalidNumber { value, position }
JsonError::InvalidEscape { ch, position }
JsonError::InvalidUnicode { sequence, position }
```

## Usage

The top-level `parse_json` function is the convenience entry point:

```rust
use rust_json_parser::{parse_json, JsonValue};

fn main() {
    let value = parse_json(r#"{"name": "Alice", "age": 30}"#).unwrap();
    assert_eq!(value.get("name"), Some(&JsonValue::Text("Alice".to_string())));
}
```

For multi-step use (inspecting the token stream, reusing parser state), construct `JsonParser` directly:

```rust
use rust_json_parser::parser::JsonParser;

let mut parser = JsonParser::new(r#""hello world""#).unwrap();
let value = parser.parse().unwrap();
```

You can also use the `Tokenizer` directly:

```rust
use rust_json_parser::tokenizer::Tokenizer;

fn main() {
    let mut tokenizer = Tokenizer::new(r#"{"name": "Alice"}"#);
    let tokens = tokenizer.tokenize().unwrap();
    println!("{:?}", tokens);
    // [LeftBrace, String("name"), Colon, String("Alice"), RightBrace]
}
```

## Python bindings

The same parser is also exposed as a Python package. Rust values become native Python types (`dict`, `list`, `float`, `bool`, `None`), and `JsonError` maps to `ValueError`; `std::io::Error` maps to `IOError`.

```python
import rust_json_parser as rjp

rjp.parse_json('{"name": "Alice", "age": 30}')
# {'name': 'Alice', 'age': 30}

rjp.parse_json_file("data.json")
# same shape, read from disk

rjp.dumps({"key": "value"})
# '{"key":"value"}'

rjp.benchmark_performance('{"key": "value"}', iterations=1000)
# (rust_seconds, json_seconds, simplejson_seconds)
```

A `__main__` entry point lets you pretty-run from the shell:

```bash
python -m rust_json_parser '{"hello": "world"}'
python -m rust_json_parser path/to/file.json
python -m rust_json_parser --benchmark
```

## Benchmarks

`python -m rust_json_parser --benchmark` runs an 8-fixture × 6-parser suite and writes a timestamped `benchmark_results.md` next to the invocation. Methodology: per (parser, fixture), 3 warmup batches discarded then 30 timed batches, batch size adapts so each batch takes ~100 ms; per-iteration time is `batch_time / batch_size`. Stats reported are computed over the 30 samples.

The competition:

- **`json`** — CPython's stdlib JSON module (C implementation). Reference baseline.
- **`orjson`** — Rust+PyO3, by ijl. The default "faster than json" answer in the Python ecosystem.
- **`msgspec`** — C, by Jim Crist-Harif. Schema-aware decoder; very fast even without a schema.
- **`ujson`** — long-running C library; used to dominate, now closer to stdlib `json` on Python 3.11+.
- **`simplejson`** — pure-Python (with the pip-installed wheel's `_speedups` C accelerator monkey-patched out before timing, so this column measures the genuine Rust-vs-pure-Python gap rather than a second Rust-vs-C comparison).

Throughput in MB/s (input size ÷ median time per parse), measured on Linux x86-64, Python 3.14:

| Fixture | Size | **rust_json_parser** | orjson | msgspec | ujson | json (stdlib) | simplejson |
|---|---|---:|---:|---:|---:|---:|---:|
| Small (synthetic) | 109 B | **146** | 255 | 286 | 113 | 60 | 12 |
| Medium (synthetic) | 11.5 KB | **199** | 305 | 251 | 177 | 142 | 12 |
| Large (synthetic) | 117.7 KB | **177** | 311 | 302 | 178 | 145 | 13 |
| XLarge (synthetic) | 560.8 KB | **131** | 243 | 260 | 157 | 123 | 12 |
| Deeply nested (228 levels) | 10.3 KB | **174** | 290 | 268 | 146 | 134 | 8 |
| `citm_catalog.json` (real) | 1.7 MB | **344** | 442 | 337 | 381 | 189 | 22 |
| `canada.json` (real, float-heavy) | 2.3 MB | **196** | 258 | 209 | 121 | 66 | 13 |
| `twitter.json` (real, unicode + escapes) | 631.5 KB | **247** | 519 | 423 | 283 | 175 | 24 |

Same data as "× faster than stdlib `json`":

| Fixture | rust_json_parser vs json (stdlib) |
|---|---|
| Small | **2.43× faster** |
| Medium | **1.40× faster** |
| Large | **1.22× faster** |
| XLarge | **1.07× faster** |
| Deeply nested | **1.30× faster** |
| `citm_catalog.json` | **1.82× faster** |
| `canada.json` | **2.96× faster** |
| `twitter.json` | **1.42× faster** |

The honest landscape: this parser sits **mid-pack of the well-known faster-than-stdlib club**. It beats CPython's stdlib `json` on every fixture in the suite, including all three real-world files (`citm_catalog`, `canada`, `twitter`) that are the canonical inputs every JSON library benchmarks against. It trades places with `ujson` (a long-established C library), and on `citm_catalog` specifically it edges past `msgspec` — that one's a genuine surprise, the workload (small ints + short escape-free strings) lands in this parser's tuned paths. `orjson` wins on every fixture; that's expected, orjson has years of full-time engineering behind it.

The real-world fixtures (`citm_catalog.json`, `canada.json`, `twitter.json`) come from the simdjson and serde-json benchmark suites — the same files orjson, msgspec, simdjson, and yyjson all benchmark against. Pulling them in puts the numbers above in the same vocabulary as the rest of the ecosystem rather than only on synthetic shapes that might inadvertently flatter the implementation. Re-pull with `python benchmarks/download_fixtures.py`; the SHA hashes are pinned so upstream churn doesn't silently change what we measure on.

Three changes drove the gap over stdlib `json`:

1. **Single-pass streaming parser.** The original tokenizer materialised a `Vec<Token>` before the parser started. `src/stream.rs` merges lex and parse into one recursive descent over the raw `&[u8]`, so thousands of `Token::String(String)` allocations disappear on big inputs.
2. **`memchr` + `FxHashMap`.** Inner loops that scan string bodies for `"` or `\` now use `memchr::memchr2` (SIMD-accelerated). Object parsing switched from `std::HashMap` (SipHash) to `rustc_hash::FxHashMap`, which is ≈3–5× faster for the short, trusted keys we're inserting.
3. **Direct Rust → Python builder.** The Python-facing `parse_json` used to parse to `JsonValue` and then walk that tree a second time to produce a `dict`. `src/python_bindings.rs::py_stream` collapses that into one pass: `PyDict::new`/`PyList::append`/`PyString::new` are called inline during parsing, so the entire `JsonValue` allocation + traversal drops out. Integer literals are returned as `int` (not `float`) to match `json.loads` and hit CPython's small-int cache.

The pure-Rust `crate::parser::parse_json` (returning `JsonValue`) and the `tokenizer`/`JsonParser` two-pass classroom version are kept for pedagogy and are still tested — they're just no longer on the hot path.

For a page-by-page deep dive on each of these decisions, plus a concept primer covering strings/bytes/UTF-8, ownership, lifetimes, and the PyO3 mental model for Python developers landing on Rust, see the [project wiki](https://github.com/jengroff/rust-cohort/wiki).

## Building

```bash
# Rust-only: compile and run pure-Rust tests
cargo test --no-default-features --lib

# Python: build the extension module and install into the active env
maturin develop

# Python integration tests (needs maturin develop first)
pytest tests/test_python_integration.py
```

`maturin develop` is the Python equivalent of `cargo build` + `pip install -e .` — it compiles the Rust cdylib, packages it with the pure-Python wrapper in `python/rust_json_parser/`, and installs the result so `import rust_json_parser` just works.

The `python` Cargo feature is on by default. Turn it off (`--no-default-features`) when running pure-Rust tests so you don't need Python headers on the build host.

API docs are inline rustdoc; browse them with `cargo doc --open --no-default-features`.

## Project structure

```
rust-json-parser/
  Cargo.toml                        # rlib + cdylib; pyo3 behind "python" feature
  pyproject.toml                    # maturin build config
  src/
    lib.rs                          # library root; gates python_bindings
    main.rs                         # rust binary entry point
    tokenizer.rs                    # Tokenizer struct and Token enum (classroom pass)
    value.rs                        # JsonValue + JsonObject (FxHashMap) + Display
    error.rs                        # JsonError enum with Display impl
    parser.rs                       # JsonParser two-pass (classroom); parse_json → stream
    stream.rs                       # single-pass streaming parser over &[u8] — the fast path
    python_bindings.rs              # PyO3 bridge + py_stream direct Rust→Python parser
  python/
    rust_json_parser/
      __init__.py                   # re-exports from the compiled extension
      __main__.py                   # `python -m rust_json_parser` CLI
      benchmark.py                  # timing harness, statistics, output formatters
      fixtures.py                   # synthetic generators + real-world fixture loaders
  benchmarks/
    download_fixtures.py            # one-shot script to fetch citm_catalog/canada/twitter
    fixtures/                       # canonical real-world JSON files (committed, ~5 MB)
  tests/
    test_python_integration.py      # pytest suite for the FFI layer
```

## Credits

I built this parser during the 6-week Rust-for-Python-developers course taught by [Jim Hodapp](https://refactorcoach.com/coaching/) and [Bob Belderbos](https://belderbos.dev/coaching/rust/). The performance work documented above came after the main coursework wrapped, but it stands directly on the Rust foundation they laid: ownership, borrowing, lifetimes, idiomatic recursive descent, the discipline of letting the compiler check the things humans get wrong. Any speed in the explanations here is theirs; any remaining naïveté is mine.