# rust-json-parser

A JSON parser built in Rust, written as part of rust-cohort-2.

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
JsonValue::Object(HashMap<String, JsonValue>)
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
# {'name': 'Alice', 'age': 30.0}

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

`benchmark_performance(json_str, iterations=1000)` times three parsers on the same input and returns the total elapsed seconds for each: this crate's `parse_json`, Python's built-in `json.loads` (C implementation), and `simplejson.loads` (pure Python).

Results from `python -m rust_json_parser --benchmark` on a recent run (1000 iterations per size):

| Input size | Rust | `json` (C) | `simplejson` | Rust vs `json` |
|---|---|---|---|---|
| Small (25 B) | 0.000559s | 0.000833s | 0.000810s | **1.49× faster** |
| Medium (2,991 B) | 0.079735s | 0.022997s | 0.021630s | 0.29× (3.5× slower) |
| Large (65,791 B) | 0.954715s | 0.437346s | 0.511625s | 0.46× (2.2× slower) |

The Rust parser wins on tiny inputs where FFI overhead dominates, but loses on real-world payloads — CPython's `json` module is a heavily-tuned C parser, and this implementation still pays the cost of a `Vec<Token>` materialisation pass plus a full Rust→Python conversion of every value. The numbers are a useful starting point for profiling, not a claim of production-readiness.

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
    tokenizer.rs                    # Tokenizer struct and Token enum
    value.rs                        # JsonValue enum + accessors + Display
    error.rs                        # JsonError enum with Display impl
    parser.rs                       # JsonParser (recursive descent)
    python_bindings.rs              # PyO3 bridge (IntoPyObject, From<JsonError>)
  python/
    rust_json_parser/
      __init__.py                   # re-exports from the compiled extension
      __main__.py                   # `python -m rust_json_parser` CLI
  tests/
    test_python_integration.py      # pytest suite for the FFI layer
```
