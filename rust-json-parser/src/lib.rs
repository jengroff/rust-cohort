//! # Rust JSON Parser
//!
//! A recursive-descent JSON parser that doubles as a Python extension
//! module via PyO3.
//!
//! ## Features
//!
//! - Parses all six JSON types (null, boolean, number, string, array, object)
//! - Arbitrary nesting depth
//! - Position-aware error messages via [`JsonError`]
//! - Native Python bindings (dict/list/float/bool/None) behind the
//!   `python` Cargo feature
//!
//! ## Quick Start
//!
//! ```
//! use rust_json_parser::{parse_json, JsonValue};
//!
//! let value = parse_json(r#"{"name": "Alice", "age": 30}"#)?;
//! assert_eq!(value.get("name"), Some(&JsonValue::Text("Alice".to_string())));
//! # Ok::<(), rust_json_parser::JsonError>(())
//! ```
//!
//! ## Python Usage
//!
//! After `maturin develop`:
//!
//! ```python
//! import rust_json_parser as rjp
//! rjp.parse_json('{"key": "value"}')  # -> {'key': 'value'}
//! ```

#![warn(missing_docs)]

/// Error types returned by the tokenizer and parser.
pub mod error;
/// Recursive-descent parser that turns tokens into a [`JsonValue`].
pub mod parser;
/// Lexer that turns a JSON string into a stream of tokens.
pub mod tokenizer;
/// The [`JsonValue`] enum and its accessor / Display implementations.
pub mod value;

pub use error::JsonError;
pub use parser::{JsonParser, parse_json};
pub use value::JsonValue;

#[cfg(feature = "python")]
mod python_bindings;
