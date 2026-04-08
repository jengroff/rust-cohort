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

```rust
use rust_json_parser::parser::JsonParser;

fn main() {
    // Parse a JSON primitive
    let mut parser = JsonParser::new(r#""hello world""#).unwrap();
    let value = parser.parse().unwrap();
    println!("{:?}", value); // Text("hello world")
}
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
  lib.rs        # library crate root (re-exports JsonValue, JsonError)
  main.rs       # binary entry point with usage examples
  tokenizer.rs  # Tokenizer struct and Token enum
  value.rs      # JsonValue enum with accessor methods
  error.rs      # JsonError enum with Display impl
  parser.rs     # JsonParser struct (tokenizes then parses)
```
