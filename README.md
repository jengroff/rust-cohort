# rust-json-parser

A JSON tokenizer built in Rust, written as part of rust-cohort-2.

## What it does

Takes a JSON string as input and breaks it down into a flat list of tokens — the first step in parsing JSON.

For example, `{"name": "Alice"}` becomes:

```
LeftBrace, String("name"), Colon, String("Alice"), RightBrace
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

## Usage

```rust
mod tokenizer;
mod enums;

fn main() {
    let tokens = tokenizer::tokenize(r#"{"name": "Alice", "age": 30}"#);
    println!("{:?}", tokens);
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
  main.rs       # entry point
  tokenizer.rs  # tokenize() function
  enums.rs      # Token enum
  lib.rs        # library crate root
```
