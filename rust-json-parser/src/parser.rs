use crate::error::JsonError;
use crate::tokenizer::{Token, Tokenizer};
use crate::value::{JsonObject, JsonValue};

/// Parses a JSON string and returns a [`JsonValue`].
///
/// This is the convenience entry point — it constructs a [`JsonParser`]
/// internally and runs it to completion. For streaming or multi-pass use
/// cases, construct [`JsonParser`] yourself.
///
/// # Examples
///
/// ```
/// use rust_json_parser::{parse_json, JsonValue};
///
/// let value = parse_json(r#"{"arr": [1, 2, 3]}"#)?;
/// assert_eq!(
///     value.get("arr").and_then(|v| v.as_array()).map(|a| a.len()),
///     Some(3)
/// );
/// # Ok::<(), rust_json_parser::JsonError>(())
/// ```
///
/// # Errors
///
/// Returns [`JsonError`] if the input is not valid JSON. Each error variant
/// carries a byte `position` pointing at the offending location.
///
/// - [`JsonError::UnexpectedToken`] — grammar violation (e.g. missing comma).
/// - [`JsonError::UnexpectedEndOfInput`] — truncated input (unclosed `{`).
/// - [`JsonError::InvalidNumber`] — malformed number literal.
/// - [`JsonError::InvalidEscape`] / [`JsonError::InvalidUnicode`] —
///   bad string escape.
pub fn parse_json(input: &str) -> Result<JsonValue, JsonError> {
    // Delegates to the single-pass [`crate::stream`] parser. The old
    // two-pass [`JsonParser`] is preserved unchanged for educational
    // purposes and its tests, but the stream parser is materially faster
    // because it avoids the intermediate `Vec<Token>` and uses memchr for
    // string scanning.
    crate::stream::parse(input)
}


/// A stateful JSON parser holding the full token stream.
///
/// Usually you want [`parse_json`] instead — it handles construction for you.
/// Use `JsonParser` directly when you need access to the intermediate token
/// state or want to run the parse in multiple steps.
pub struct JsonParser {
    tokens: Vec<Token>,
    position: usize,
}

impl JsonParser {
    /// Tokenizes `input` and returns a parser positioned at the start of
    /// the token stream.
    ///
    /// # Errors
    ///
    /// Returns [`JsonError`] if tokenization fails (bad escape, bad number,
    /// etc.). Grammar errors only surface later, during [`Self::parse`].
    pub fn new(input: &str) -> Result<Self, JsonError> {
        let mut tokenizer = Tokenizer::new(input);
        let tokens = tokenizer.tokenize()?;
        Ok(Self {
            tokens,
            position: 0,
        })
    }
    //
    // ── PUBLIC METHOD: parse ───────────────────────────────────────────
    //
    // Thin public wrapper that delegates to the internal recursive dispatcher.
    //
    /// Parse the token stream into a single [`JsonValue`].
    pub fn parse(&mut self) -> Result<JsonValue, JsonError> {
        self.parse_value()
    }
    //
    // ── PRIVATE METHOD: parse_value ───────────────────────────────────
    //
    // The recursive dispatch hub. Advances one token and matches on it
    // to decide which sub-parser to call. parse_array() and parse_object()
    // call back into parse_value() for nested values — that's the recursion.
    //
    fn parse_value(&mut self) -> Result<JsonValue, JsonError> {
        if self.is_at_end() {
            return Err(JsonError::UnexpectedEndOfInput {
                expected: "JSON value".to_string(),
                position: 0,
            });
        }

        // Separating "get the token" from "interpret the token" into
        // two steps. The .ok_or() converts Option<Token> -> Result<Token>.

        let token = self.advance().ok_or(JsonError::UnexpectedEndOfInput {
            expected: "JSON value".to_string(),
            position: self.position,
        })?;

        match token {
            Token::Null => Ok(JsonValue::Null),
            Token::Boolean(b) => Ok(JsonValue::Boolean(b)),
            Token::Number(n) => Ok(JsonValue::Number(n)),
            Token::String(s) => Ok(JsonValue::Text(s)),
            Token::LeftBracket => self.parse_array(),
            Token::LeftBrace => self.parse_object(),
            other => Err(JsonError::UnexpectedToken {
                expected: "JSON value".to_string(),
                found: format!("{:?}", other),
                position: self.position,
            }),
        }
    }
    //
    // ── PRIVATE HELPERS ────────────────────────────────────────────────
    //
    // Same pattern as Tokenizer's helpers but operating on tokens instead
    // of characters.
    //
    // rewriting this function per Jim's suggestion to simplify the logic:
    //
    // Rewrite #1 (function) -------------------------------------------------
    // fn advance(&mut self) -> Option<Token> {
    //     let token = self.tokens.get(self.position).cloned();

    //         if let Some(_) = &token {
    //             self.position += 1;
    //         }
    //         token
    // }
    // Rewrite #1 (explanation)
    // let token = self.tokens.get(self.position) -> safely indexes into Vec<Token> at self.position
    //     and returns Option<&Token> (a borrow / reference to the token), or None (if OOB).
    //
    // .cloned()  -> converts Option<&Token> into Option<Token> by cloning the inner value, b/c
    //     we need to have an owned copy, not a reference.
    //
    // if let Some(_) -> I don't care about the token's actual value, just need to know it exists
    //    in order to increment the position; therefore I can use _ in Some(_) to say "I don't care
    //    what's inside, just check that it's Some
    //
    // also don't need a final else clause b/c get() already returns None when OOB.
    // ---------------------------------------------------------------------------

    // Rewrite #3: move the token out of the Vec with std::mem::replace
    // instead of cloning it. Cloning a Token::String(String) allocates a
    // fresh heap buffer every time we advance — on a 65KB JSON that's
    // thousands of allocations just to walk the token stream. We own the
    // Vec and only walk forward, so we can swap each slot for a cheap
    // Token::Null placeholder and hand the real token to the caller.
    fn advance(&mut self) -> Option<Token> {
        if self.position >= self.tokens.len() {
            return None;
        }
        let token = std::mem::replace(&mut self.tokens[self.position], Token::Null);
        self.position += 1;
        Some(token)
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.tokens.len()
    }

    /// Peeks at the next token and checks if its variant matches `expected`,
    /// without consuming it. Returns false if we're at the end.
    fn check(&self, expected: &Token) -> bool {
        self.tokens
            .get(self.position)
            .map(|t| std::mem::discriminant(t) == std::mem::discriminant(expected))
            .unwrap_or(false)
    }

    //
    // parse_array follows the classic recursive descent pattern:
    //
    // 1. Opening [ was already consumed by parse_value
    // 2. Check for empty array — if next token is ], we're done
    // 3. Loop: parse a value, then expect either , (continue) or ] (break)
    // 4. Trailing comma detection: after seeing , if the next token is ],
    //    that's a JSON error (JSON doesn't allow [1, 2, 3,])
    //
    // The recursion happens at self.parse_value()?. If the array contains
    // another array like [[1, 2], [3, 4]], parse_value() sees [ and calls
    // parse_array() again.
    //

    fn parse_array(&mut self) -> Result<JsonValue, JsonError> {
        // Opening [ already consumed by parse_value

        if self.check(&Token::RightBracket) {
            self.advance();
            return Ok(JsonValue::Array(vec![]));
        }

        let mut elements = Vec::new();

        loop {
            let value = self.parse_value()?;
            elements.push(value);

            let token = self.advance().ok_or(JsonError::UnexpectedEndOfInput {
                expected: "] or ,".to_string(),
                position: self.position,
            })?;
            match token {
                Token::RightBracket => break,
                Token::Comma => {
                    if self.check(&Token::RightBracket) {
                        return Err(JsonError::UnexpectedToken {
                            expected: "value".to_string(),
                            found: "]".to_string(),
                            position: self.position,
                        });
                    }
                }
                other => {
                    return Err(JsonError::UnexpectedToken {
                        expected: ", or ]".to_string(),
                        found: format!("{:?}", other),
                        position: self.position,
                    });
                }
            }
        }

        Ok(JsonValue::Array(elements))
    }

    //
    // parse_object is structurally identical to parse_array but with
    // key-value pair handling:
    //
    // 1. Opening { was already consumed by parse_value
    // 2. Check for empty object {}
    // 3. Loop:
    //    a. Parse a string key (advance, match on Token::String(s))
    //    b. Expect and consume :
    //    c. Parse a value (recursive call)
    //    d. Insert into HashMap
    //    e. Expect , (continue) or } (break)
    //
    // The key extraction: `let key = match key_token { Token::String(s) => s, ... }`
    // This MOVES the String out of the Token variant. The token is destructured
    // and `s` becomes an owned String that can be inserted into the HashMap.
    //

    fn parse_object(&mut self) -> Result<JsonValue, JsonError> {
        // Opening { already consumed by parse_value

        if self.check(&Token::RightBrace) {
            self.advance();
            return Ok(JsonValue::Object(JsonObject::default()));
        }

        let mut map = JsonObject::default();

        loop {
            let key = self.parse_object_key()?;
            self.expect_colon()?;
            let value = self.parse_value()?;
            map.insert(key, value);

            if self.finish_pair_or_continue()? {
                break;
            }
        }

        Ok(JsonValue::Object(map))
    }

    // Consumes one token, requires it to be a string, returns the owned key.
    fn parse_object_key(&mut self) -> Result<String, JsonError> {
        let token = self.advance().ok_or(JsonError::UnexpectedEndOfInput {
            expected: "string key".to_string(),
            position: self.position,
        })?;
        match token {
            Token::String(s) => Ok(s),
            other => Err(JsonError::UnexpectedToken {
                expected: "string key".to_string(),
                found: format!("{:?}", other),
                position: self.position,
            }),
        }
    }

    // Consumes the `:` that separates key from value.
    fn expect_colon(&mut self) -> Result<(), JsonError> {
        let token = self.advance().ok_or(JsonError::UnexpectedEndOfInput {
            expected: ":".to_string(),
            position: self.position,
        })?;
        if matches!(token, Token::Colon) {
            Ok(())
        } else {
            Err(JsonError::UnexpectedToken {
                expected: ":".to_string(),
                found: format!("{:?}", token),
                position: self.position,
            })
        }
    }

    // After a key/value pair, consume either `}` (return true = done) or `,`
    // (return false = another pair expected). Rejects trailing commas.
    fn finish_pair_or_continue(&mut self) -> Result<bool, JsonError> {
        let token = self.advance().ok_or(JsonError::UnexpectedEndOfInput {
            expected: "} or ,".to_string(),
            position: self.position,
        })?;
        match token {
            Token::RightBrace => Ok(true),
            Token::Comma => {
                if self.check(&Token::RightBrace) {
                    return Err(JsonError::UnexpectedToken {
                        expected: "string key".to_string(),
                        found: "}".to_string(),
                        position: self.position,
                    });
                }
                Ok(false)
            }
            other => Err(JsonError::UnexpectedToken {
                expected: ", or }".to_string(),
                found: format!("{:?}", other),
                position: self.position,
            }),
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod array_tests {
    use super::*;

    #[test]
    fn test_parse_empty_array() {
        let value = parse_json("[]").unwrap();
        assert_eq!(value, JsonValue::Array(vec![]));
    }

    #[test]
    fn test_parse_array_single() {
        let value = parse_json("[1]").unwrap();
        assert_eq!(value, JsonValue::Array(vec![JsonValue::Number(1.0)]));
    }

    #[test]
    fn test_parse_array_multiple() {
        let value = parse_json("[1, 2, 3]").unwrap();
        let expected = JsonValue::Array(vec![
            JsonValue::Number(1.0),
            JsonValue::Number(2.0),
            JsonValue::Number(3.0),
        ]);
        assert_eq!(value, expected);
    }

    #[test]
    fn test_parse_array_mixed_types() {
        let value = parse_json(r#"[1, "two", true, null]"#).unwrap();
        let expected = JsonValue::Array(vec![
            JsonValue::Number(1.0),
            JsonValue::Text("two".to_string()),
            JsonValue::Boolean(true),
            JsonValue::Null,
        ]);
        assert_eq!(value, expected);
    }

    #[test]
    fn test_parse_nested_arrays() {
        let value = parse_json("[[1, 2], [3, 4]]").unwrap();
        let expected = JsonValue::Array(vec![
            JsonValue::Array(vec![JsonValue::Number(1.0), JsonValue::Number(2.0)]),
            JsonValue::Array(vec![JsonValue::Number(3.0), JsonValue::Number(4.0)]),
        ]);
        assert_eq!(value, expected);
    }

    #[test]
    fn test_parse_deeply_nested() {
        let value = parse_json("[[[1]]]").unwrap();
        let expected = JsonValue::Array(vec![JsonValue::Array(vec![JsonValue::Array(vec![
            JsonValue::Number(1.0),
        ])])]);
        assert_eq!(value, expected);
    }

    #[test]
    fn test_array_accessor() {
        let value = parse_json("[1, 2, 3]").unwrap();
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 3);
    }

    #[test]
    fn test_array_get_index() {
        let value = parse_json("[10, 20, 30]").unwrap();
        assert_eq!(value.get_index(1), Some(&JsonValue::Number(20.0)));
        assert_eq!(value.get_index(5), None);
    }
}

#[cfg(test)]
mod object_tests {
    use super::*;

    #[test]
    fn test_parse_empty_object() {
        let value = parse_json("{}").unwrap();
        assert_eq!(value, JsonValue::Object(JsonObject::default()));
    }

    #[test]
    fn test_parse_object_single_key() {
        let value = parse_json(r#"{"key": "value"}"#).unwrap();
        let mut expected = JsonObject::default();
        expected.insert("key".to_string(), JsonValue::Text("value".to_string()));
        assert_eq!(value, JsonValue::Object(expected));
    }

    #[test]
    fn test_parse_object_multiple_keys() {
        let value = parse_json(r#"{"name": "Alice", "age": 30}"#).unwrap();
        if let JsonValue::Object(obj) = value {
            assert_eq!(obj.get("name"), Some(&JsonValue::Text("Alice".to_string())));
            assert_eq!(obj.get("age"), Some(&JsonValue::Number(30.0)));
        } else {
            panic!("Expected object");
        }
    }

    #[test]
    fn test_parse_nested_object() {
        let value = parse_json(r#"{"outer": {"inner": 1}}"#).unwrap();
        if let JsonValue::Object(outer) = value {
            if let Some(JsonValue::Object(inner)) = outer.get("outer") {
                assert_eq!(inner.get("inner"), Some(&JsonValue::Number(1.0)));
            } else {
                panic!("Expected nested object");
            }
        } else {
            panic!("Expected object");
        }
    }

    #[test]
    fn test_parse_array_in_object() {
        let value = parse_json(r#"{"items": [1, 2, 3]}"#).unwrap();
        if let JsonValue::Object(obj) = value {
            if let Some(JsonValue::Array(arr)) = obj.get("items") {
                assert_eq!(arr.len(), 3);
            } else {
                panic!("Expected array");
            }
        } else {
            panic!("Expected object");
        }
    }

    #[test]
    fn test_parse_object_in_array() {
        let value = parse_json(r#"[{"a": 1}, {"b": 2}]"#).unwrap();
        if let JsonValue::Array(arr) = value {
            assert_eq!(arr.len(), 2);
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_object_accessor() {
        let value = parse_json(r#"{"name": "test"}"#).unwrap();
        let obj = value.as_object().unwrap();
        assert_eq!(obj.len(), 1);
    }

    #[test]
    fn test_object_get() {
        let value = parse_json(r#"{"name": "Alice", "age": 30}"#).unwrap();
        assert_eq!(
            value.get("name"),
            Some(&JsonValue::Text("Alice".to_string()))
        );
        assert_eq!(value.get("missing"), None);
    }
}

#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn test_error_unclosed_array() {
        let result = parse_json("[1, 2");
        assert!(result.is_err());
    }

    #[test]
    fn test_error_unclosed_object() {
        let result = parse_json(r#"{"key": 1"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_error_trailing_comma_array() {
        let result = parse_json("[1, 2,]");
        assert!(result.is_err());
    }

    #[test]
    fn test_error_trailing_comma_object() {
        let result = parse_json(r#"{"a": 1,}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_error_missing_colon() {
        let result = parse_json(r#"{"key" 1}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_error_invalid_key() {
        let result = parse_json(r#"{123: "value"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_error_missing_comma_array() {
        let result = parse_json("[1 2 3]");
        assert!(result.is_err());
    }

    #[test]
    fn test_error_missing_comma_object() {
        let result = parse_json(r#"{"a": 1 "b": 2}"#);
        assert!(result.is_err());
    }
}
