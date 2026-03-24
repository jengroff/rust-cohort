# rust-json-parser

A JSON parser built in Rust, written as part of rust-cohort-2.

## What it does

Takes a JSON string, tokenizes it, and parses it into a `JsonValue` — currently supporting primitive types (strings, numbers, booleans, and null).

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
```

## Usage

```rust
use rust_json_parser::parse_json;

fn main() {
    let value = parse_json(r#""hello world""#).unwrap();
    println!("{:?}", value); // Text("hello world")
}
```

## Running

```bash
cargo run
```

## Tests

```bash
cargo test
```

## Project structure

```
src/
  lib.rs        # library crate root
  tokenizer.rs  # Token enum and tokenize() function
  value.rs      # JsonValue enum
  error.rs      # JsonError enum
  parser.rs     # parse_json() function
```
