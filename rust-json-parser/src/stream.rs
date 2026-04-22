//! Single-pass streaming JSON parser.
//!
//! The tokenizer-plus-parser design in [`crate::parser`] is nice to read
//! but it materialises a `Vec<Token>` before parsing begins. On a 65 KB
//! document that's thousands of `Token::String(String)` heap allocations
//! before the parser touches a single byte of structure.
//!
//! This module collapses the two passes into one recursive descent over
//! the raw `&[u8]`. `memchr` handles the hot inner loop (scanning string
//! bodies for `"` or `\`), so the common case of a key/value without
//! escape sequences becomes one SIMD-accelerated scan plus one `String`
//! allocation.
//!
//! # Invariants
//!
//! * Input is a `&str`, so `self.input` is guaranteed-valid UTF-8.
//! * Every `position` advance either consumes an ASCII byte or a complete
//!   UTF-8 sequence inside a string body. That means `from_utf8_unchecked`
//!   on the slices we hand back is safe.

use memchr::memchr2;

use crate::error::JsonError;
use crate::value::{JsonObject, JsonValue};

/// Parse a JSON document into a [`JsonValue`] in a single pass.
pub fn parse(input: &str) -> Result<JsonValue, JsonError> {
    let mut p = StreamParser::new(input);
    p.skip_ws();
    let value = p.parse_value()?;
    p.skip_ws();
    if p.position < p.input.len() {
        return Err(JsonError::UnexpectedToken {
            expected: "end of input".to_string(),
            found: (p.input[p.position] as char).to_string(),
            position: p.position,
        });
    }
    Ok(value)
}

/// Internal parser state. Not exposed — callers should use [`parse`].
struct StreamParser<'a> {
    input: &'a [u8],
    position: usize,
}

impl<'a> StreamParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            position: 0,
        }
    }

    #[inline]
    fn peek(&self) -> Option<u8> {
        self.input.get(self.position).copied()
    }

    #[inline]
    fn skip_ws(&mut self) {
        // JSON whitespace is exactly 0x20 (space), 0x09 (tab), 0x0A (LF),
        // 0x0D (CR). A tight inline loop is faster than is_ascii_whitespace
        // which also accepts form-feed and vertical-tab.
        while let Some(b) = self.input.get(self.position) {
            match *b {
                b' ' | b'\t' | b'\n' | b'\r' => self.position += 1,
                _ => break,
            }
        }
    }

    fn parse_value(&mut self) -> Result<JsonValue, JsonError> {
        match self.peek() {
            Some(b'{') => {
                self.position += 1;
                self.parse_object()
            }
            Some(b'[') => {
                self.position += 1;
                self.parse_array()
            }
            Some(b'"') => {
                self.position += 1;
                let s = self.parse_string_body()?;
                Ok(JsonValue::Text(s))
            }
            Some(b't') => {
                self.expect_keyword(b"true")?;
                Ok(JsonValue::Boolean(true))
            }
            Some(b'f') => {
                self.expect_keyword(b"false")?;
                Ok(JsonValue::Boolean(false))
            }
            Some(b'n') => {
                self.expect_keyword(b"null")?;
                Ok(JsonValue::Null)
            }
            Some(b'-') | Some(b'0'..=b'9') => {
                let n = self.parse_number()?;
                Ok(JsonValue::Number(n))
            }
            Some(other) => Err(JsonError::UnexpectedToken {
                expected: "JSON value".to_string(),
                found: (other as char).to_string(),
                position: self.position,
            }),
            None => Err(JsonError::UnexpectedEndOfInput {
                expected: "JSON value".to_string(),
                position: self.position,
            }),
        }
    }

    fn parse_array(&mut self) -> Result<JsonValue, JsonError> {
        // Opening `[` already consumed.
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.position += 1;
            return Ok(JsonValue::Array(Vec::new()));
        }

        // Small starting capacity — we'll double as needed. A blind guess
        // larger than this wastes memory on single-element arrays, which
        // are common inside nested objects.
        let mut elements: Vec<JsonValue> = Vec::with_capacity(4);
        loop {
            let value = self.parse_value()?;
            elements.push(value);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.position += 1;
                    self.skip_ws();
                    if self.peek() == Some(b']') {
                        return Err(JsonError::UnexpectedToken {
                            expected: "value".to_string(),
                            found: "]".to_string(),
                            position: self.position,
                        });
                    }
                }
                Some(b']') => {
                    self.position += 1;
                    return Ok(JsonValue::Array(elements));
                }
                Some(c) => {
                    return Err(JsonError::UnexpectedToken {
                        expected: ", or ]".to_string(),
                        found: (c as char).to_string(),
                        position: self.position,
                    });
                }
                None => {
                    return Err(JsonError::UnexpectedEndOfInput {
                        expected: "] or ,".to_string(),
                        position: self.position,
                    });
                }
            }
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, JsonError> {
        // Opening `{` already consumed.
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.position += 1;
            return Ok(JsonValue::Object(JsonObject::default()));
        }

        let mut map = JsonObject::default();
        loop {
            // Key must be a string.
            match self.peek() {
                Some(b'"') => self.position += 1,
                Some(c) => {
                    return Err(JsonError::UnexpectedToken {
                        expected: "string key".to_string(),
                        found: (c as char).to_string(),
                        position: self.position,
                    });
                }
                None => {
                    return Err(JsonError::UnexpectedEndOfInput {
                        expected: "string key".to_string(),
                        position: self.position,
                    });
                }
            }
            let key = self.parse_string_body()?;
            self.skip_ws();
            // Consume `:`.
            match self.peek() {
                Some(b':') => self.position += 1,
                Some(c) => {
                    return Err(JsonError::UnexpectedToken {
                        expected: ":".to_string(),
                        found: (c as char).to_string(),
                        position: self.position,
                    });
                }
                None => {
                    return Err(JsonError::UnexpectedEndOfInput {
                        expected: ":".to_string(),
                        position: self.position,
                    });
                }
            }
            self.skip_ws();
            let value = self.parse_value()?;
            map.insert(key, value);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.position += 1;
                    self.skip_ws();
                    if self.peek() == Some(b'}') {
                        return Err(JsonError::UnexpectedToken {
                            expected: "string key".to_string(),
                            found: "}".to_string(),
                            position: self.position,
                        });
                    }
                }
                Some(b'}') => {
                    self.position += 1;
                    return Ok(JsonValue::Object(map));
                }
                Some(c) => {
                    return Err(JsonError::UnexpectedToken {
                        expected: ", or }".to_string(),
                        found: (c as char).to_string(),
                        position: self.position,
                    });
                }
                None => {
                    return Err(JsonError::UnexpectedEndOfInput {
                        expected: "} or ,".to_string(),
                        position: self.position,
                    });
                }
            }
        }
    }

    /// Parse a JSON string body — the opening `"` has already been consumed.
    ///
    /// Fast path: scan with `memchr` for the next `"` or `\`. If it's a
    /// `"` we slice the untouched bytes straight into a String. If it's
    /// a `\` we fall through to the slow per-byte escape handler.
    fn parse_string_body(&mut self) -> Result<String, JsonError> {
        let start = self.position;
        let bytes = self.input;

        // Fast path: memchr2 is SIMD-accelerated; finding a quote or
        // backslash in a long clean string takes ~one instruction per
        // 16/32 bytes on modern CPUs.
        match memchr2(b'"', b'\\', &bytes[self.position..]) {
            Some(offset) => {
                let end = self.position + offset;
                if bytes[end] == b'"' {
                    // No escapes — cheapest possible path.
                    // SAFETY: input is valid UTF-8 (it came from a &str),
                    // and `"` is ASCII so the slice boundary is valid.
                    let s =
                        unsafe { std::str::from_utf8_unchecked(&bytes[start..end]) }.to_owned();
                    self.position = end + 1;
                    return Ok(s);
                }
                // Escape — copy the clean prefix, then fall into the slow path.
                self.position = end;
                self.parse_string_with_escapes(start)
            }
            None => Err(JsonError::UnexpectedEndOfInput {
                expected: "closing quote".to_string(),
                position: start,
            }),
        }
    }

    /// Slow path for strings containing `\` escapes.
    ///
    /// On entry `self.position` points at the first `\` byte; `clean_start`
    /// points at the first byte of the string body.
    fn parse_string_with_escapes(&mut self, clean_start: usize) -> Result<String, JsonError> {
        let bytes = self.input;
        // Upper-bound capacity: the decoded string is at most as long as
        // the remaining input. One allocation, no reallocs.
        let mut buf: Vec<u8> = Vec::with_capacity(bytes.len() - clean_start);
        // Copy the prefix we already validated is escape-free.
        buf.extend_from_slice(&bytes[clean_start..self.position]);

        loop {
            match self.peek() {
                Some(b'"') => {
                    self.position += 1;
                    // SAFETY: all bytes pushed are either verbatim from a
                    // valid-UTF-8 input or well-formed UTF-8 encodings of
                    // escape-expanded `char` values.
                    let s = unsafe { String::from_utf8_unchecked(buf) };
                    return Ok(s);
                }
                Some(b'\\') => {
                    self.position += 1;
                    self.decode_escape(&mut buf)?;
                }
                Some(_) => {
                    // Copy a contiguous non-escape run in one memcpy.
                    match memchr2(b'"', b'\\', &bytes[self.position..]) {
                        Some(offset) => {
                            buf.extend_from_slice(
                                &bytes[self.position..self.position + offset],
                            );
                            self.position += offset;
                        }
                        None => {
                            return Err(JsonError::UnexpectedEndOfInput {
                                expected: "closing quote".to_string(),
                                position: clean_start,
                            });
                        }
                    }
                }
                None => {
                    return Err(JsonError::UnexpectedEndOfInput {
                        expected: "closing quote".to_string(),
                        position: clean_start,
                    });
                }
            }
        }
    }

    /// Handle one escape sequence after the leading `\` has been consumed.
    /// Appends the decoded bytes to `buf`.
    fn decode_escape(&mut self, buf: &mut Vec<u8>) -> Result<(), JsonError> {
        let pos = self.position;
        let b = match self.input.get(pos) {
            Some(&b) => b,
            None => {
                return Err(JsonError::UnexpectedEndOfInput {
                    expected: "escape character".to_string(),
                    position: pos,
                });
            }
        };
        self.position += 1;
        match b {
            b'"' => buf.push(b'"'),
            b'\\' => buf.push(b'\\'),
            b'/' => buf.push(b'/'),
            b'b' => buf.push(0x08),
            b'f' => buf.push(0x0C),
            b'n' => buf.push(b'\n'),
            b'r' => buf.push(b'\r'),
            b't' => buf.push(b'\t'),
            b'u' => {
                let ch = self.parse_unicode_escape()?;
                let mut tmp = [0u8; 4];
                let encoded = ch.encode_utf8(&mut tmp);
                buf.extend_from_slice(encoded.as_bytes());
            }
            other => {
                return Err(JsonError::InvalidEscape {
                    ch: other as char,
                    position: pos,
                });
            }
        }
        Ok(())
    }

    /// Parse exactly four hex digits into a `char`.
    fn parse_unicode_escape(&mut self) -> Result<char, JsonError> {
        let start = self.position;
        if self.position + 4 > self.input.len() {
            let tail = &self.input[self.position..];
            self.position = self.input.len();
            // SAFETY: tail is valid UTF-8 (came from a &str slice).
            let seq = unsafe { std::str::from_utf8_unchecked(tail) }.to_string();
            return Err(JsonError::InvalidUnicode {
                sequence: seq,
                position: start,
            });
        }
        let hex = &self.input[self.position..self.position + 4];
        if !hex.iter().all(|b| b.is_ascii_hexdigit()) {
            let kept: String = hex
                .iter()
                .take_while(|b| b.is_ascii_hexdigit())
                .map(|b| *b as char)
                .collect();
            self.position += 4;
            return Err(JsonError::InvalidUnicode {
                sequence: kept,
                position: start,
            });
        }
        self.position += 4;
        // Hex digits are ASCII so this is safe.
        let hex_str = unsafe { std::str::from_utf8_unchecked(hex) };
        let code_point = u32::from_str_radix(hex_str, 16).expect("4 hex digits parse");
        char::from_u32(code_point).ok_or(JsonError::InvalidUnicode {
            sequence: hex_str.to_string(),
            position: start,
        })
    }

    fn parse_number(&mut self) -> Result<f64, JsonError> {
        let start = self.position;
        // Accept the superset that `f64::from_str` can handle; the parse
        // itself will reject anything malformed.
        while let Some(&b) = self.input.get(self.position) {
            match b {
                b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E' => self.position += 1,
                _ => break,
            }
        }
        let bytes = &self.input[start..self.position];
        // SAFETY: all accepted bytes are ASCII.
        let num_str = unsafe { std::str::from_utf8_unchecked(bytes) };
        num_str.parse::<f64>().map_err(|_| JsonError::InvalidNumber {
            value: num_str.to_string(),
            position: start,
        })
    }

    fn expect_keyword(&mut self, keyword: &[u8]) -> Result<(), JsonError> {
        let start = self.position;
        let end = start + keyword.len();
        if end > self.input.len() || &self.input[start..end] != keyword {
            // Collect what we actually saw, up to keyword length.
            let saw_end = end.min(self.input.len());
            let saw = &self.input[start..saw_end];
            // SAFETY: input is valid UTF-8.
            let found = unsafe { std::str::from_utf8_unchecked(saw) }.to_string();
            return Err(JsonError::UnexpectedToken {
                expected: unsafe { std::str::from_utf8_unchecked(keyword) }.to_string(),
                found,
                position: start,
            });
        }
        self.position = end;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_small() {
        let v = parse(r#"{"name": "Alice", "age": 30}"#).unwrap();
        let o = v.as_object().unwrap();
        assert_eq!(o.get("name"), Some(&JsonValue::Text("Alice".to_string())));
        assert_eq!(o.get("age"), Some(&JsonValue::Number(30.0)));
    }

    #[test]
    fn test_parse_nested() {
        let v = parse(r#"{"a": [1, [2, 3], {"b": null}]}"#).unwrap();
        let arr = v.get("a").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 3);
    }

    #[test]
    fn test_parse_escapes() {
        let v = parse(r#""hello\nworld\t\"quoted\"\u0041""#).unwrap();
        assert_eq!(v.as_str(), Some("hello\nworld\t\"quoted\"A"));
    }

    #[test]
    fn test_parse_error_trailing() {
        assert!(parse("{}x").is_err());
    }

    #[test]
    fn test_parse_error_trailing_comma_array() {
        assert!(parse("[1, 2,]").is_err());
    }

    #[test]
    fn test_parse_error_trailing_comma_object() {
        assert!(parse(r#"{"a": 1,}"#).is_err());
    }

    #[test]
    fn test_parse_empty_containers() {
        assert_eq!(parse("{}").unwrap(), JsonValue::Object(JsonObject::default()));
        assert_eq!(parse("[]").unwrap(), JsonValue::Array(vec![]));
    }

    #[test]
    fn test_parse_numbers() {
        assert_eq!(parse("42").unwrap(), JsonValue::Number(42.0));
        assert_eq!(parse("-3.14").unwrap(), JsonValue::Number(-3.14));
        assert_eq!(parse("1.5e-3").unwrap(), JsonValue::Number(0.0015));
    }
}
