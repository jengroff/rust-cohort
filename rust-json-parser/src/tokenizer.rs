use crate::error::JsonError;

/// A single JSON lexeme emitted by [`Tokenizer::tokenize`].
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// `{`
    LeftBrace,
    /// `}`
    RightBrace,
    /// `[`
    LeftBracket,
    /// `]`
    RightBracket,
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// A string literal with escapes already resolved.
    String(String),
    /// A numeric literal parsed into an `f64`.
    Number(f64),
    /// `true` or `false`.
    Boolean(bool),
    /// `null`.
    Null,
}

// Making heavy use of inline comments to understand what
// is happening and for future readability.
//
// -------- THE STRUCT ---------------------------
// Tokenizer itself is public but its attributes are private.
//
// The `'a` lifetime says "this Tokenizer borrows from some &str that
// outlives it". That lets us avoid copying the entire input up-front.
// Compared with the earlier `Vec<char>` version this saves:
//   * 4x memory (each `char` is 4 bytes; each `u8` is 1)
//   * all the allocation/memcpy work of the upfront .chars().collect()
//   * cache misses walking a sparse char buffer byte-by-byte
//
// We walk the bytes directly because every structural JSON token
// ( { } [ ] , : " 0-9 - t f n and whitespace ) is ASCII, and UTF-8
// continuation bytes never collide with those bytes. Multi-byte chars
// only matter inside string literals, where we copy them through as
// raw bytes and reconstruct a String at the end.
//
/// Lexer that converts a JSON string into a stream of [`Token`]s.
pub struct Tokenizer<'a> {
    input: &'a [u8],
    position: usize,
}

impl<'a> Tokenizer<'a> {
    // Everything inside this is either a method (takes self)
    // or an associated function (no self) - kind of like @classmethod.
    //
    /// Build a new `Tokenizer` borrowing `input`. No allocation happens here —
    /// the tokenizer just holds a byte-slice view of the caller's string.
    pub fn new(input: &'a str) -> Self {
        // `input.as_bytes()` is O(1) — it hands back the underlying UTF-8
        // buffer of the &str with zero copying. Since we only look at
        // individual bytes (and all JSON structural chars are ASCII), this
        // is both safe and much faster than the old char-vector approach.
        Self {
            input: input.as_bytes(),
            position: 0,
        }
    }
    //
    // ── PUBLIC METHOD: tokenize ────────────────────────────────────────
    //
    // &mut self = this method can READ and MUTATE the struct's fields.
    // In Python every method can mutate self.
    // We need &mut because tokenize advances self.position.
    //
    // Returns Result<Vec<Token>, JsonError> — like before except
    // the logic lives inside a method instead of a free function.
    //
    /// Consume the input and return the full token stream, or a [`JsonError`]
    /// at the first lexical problem.
    pub fn tokenize(&mut self) -> Result<Vec<Token>, JsonError> {
        // Hint: on average a JSON doc has ~1 token per ~4-5 bytes of input.
        // Pre-sizing the Vec here saves a few reallocations on big inputs.
        let mut tokens = Vec::with_capacity(self.input.len() / 4);
        loop {
            match self.peek() {
                Some(b'{') => {
                    self.advance();
                    tokens.push(Token::LeftBrace);
                }
                Some(b'}') => {
                    self.advance();
                    tokens.push(Token::RightBrace);
                }
                Some(b'[') => {
                    self.advance();
                    tokens.push(Token::LeftBracket);
                }
                Some(b']') => {
                    self.advance();
                    tokens.push(Token::RightBracket);
                }
                Some(b',') => {
                    self.advance();
                    tokens.push(Token::Comma);
                }
                Some(b':') => {
                    self.advance();
                    tokens.push(Token::Colon);
                }
                Some(b'"') => {
                    let token = self.tokenize_string()?;
                    tokens.push(token);
                }
                Some(b'0'..=b'9') | Some(b'-') => {
                    let token = self.tokenize_number()?;
                    tokens.push(token);
                }
                Some(b't') => {
                    self.expect_keyword("true")?;
                    tokens.push(Token::Boolean(true));
                }
                Some(b'f') => {
                    self.expect_keyword("false")?;
                    tokens.push(Token::Boolean(false));
                }
                Some(b'n') => {
                    self.expect_keyword("null")?;
                    tokens.push(Token::Null);
                }
                Some(c) if c.is_ascii_whitespace() => {
                    self.advance();
                }
                Some(other) => {
                    return Err(JsonError::UnexpectedToken {
                        expected: "valid JSON token".to_string(),
                        // char::from(u8) maps bytes 0x00-0xFF into chars
                        // U+0000-U+00FF. For ASCII (the common case) that's
                        // just the printable form; for non-ASCII bytes it's
                        // a Latin-1-style rendering, good enough for an
                        // error message since `position` is the real locator.
                        found: char::from(other).to_string(),
                        position: self.position,
                    });
                }
                None => break,
            }
        }
        Ok(tokens)
    }

    //
    // ── PRIVATE HELPERS ────────────────────────────────────────────────
    //
    // No `pub` = private to this module. Like Python's _underscore methods,
    // except in Rust they're actually private.
    //

    fn advance(&mut self) -> Option<u8> {
        let byte = self.input.get(self.position).copied();
        if byte.is_some() {
            self.position += 1;
        }
        byte
    }

    // Look at the current byte without moving the cursor.
    // &self (not &mut self) because peeking is read-only.
    fn peek(&self) -> Option<u8> {
        self.input.get(self.position).copied()
    }

    //
    // ── STRING TOKENIZATION with escape sequences (oh the pain) ──────────────────
    //
    // This monster method handles:
    // 1. Regular characters (push their bytes)
    // 2. Escape sequences
    // 3. Unicode escapes
    // 4. Error cases (unterminated string, invalid escape, bad unicode)
    //
    // We accumulate into a Vec<u8> and convert to String at the end.
    // That's safe because every byte we push is either:
    //   * a byte from the (valid UTF-8) input, copied through verbatim, or
    //   * the UTF-8 encoding of a char produced by an escape sequence.
    // Either way the final buffer is guaranteed-valid UTF-8.
    //

    fn tokenize_string(&mut self) -> Result<Token, JsonError> {
        let start = self.position;
        self.advance(); // skip opening "
        let content_start = self.position;

        // Fast path: scan for the closing quote OR a backslash. If we hit
        // the quote first, the string has NO escapes and we can turn the
        // raw byte slice straight into a String in one shot — no Vec<u8>,
        // no per-byte pushes.
        //
        // This is the common case: most JSON keys and most string values
        // don't contain escapes, so we want it to be as close to free as
        // possible. Matches the "avoid .clone()/use with_capacity" tip
        // from the guide — the only remaining allocation is the final
        // String itself, which is unavoidable.
        let mut i = content_start;
        while i < self.input.len() {
            let b = self.input[i];
            if b == b'"' {
                let bytes = &self.input[content_start..i];
                self.position = i + 1;
                // UTF-8 safety: the input was validated UTF-8 when taken
                // as &str in `new()`, and b'"' is ASCII so the slice ends
                // on a char boundary. Unchecked avoids a redundant pass.
                let s = unsafe { std::str::from_utf8_unchecked(bytes) }.to_string();
                return Ok(Token::String(s));
            }
            if b == b'\\' {
                break; // escape found — fall through to slow path
            }
            i += 1;
        }

        // Slow path: escape processing needed (or input ran out).
        // Pre-fill the buffer with the non-escape prefix we already scanned,
        // then continue byte-by-byte. with_capacity uses the remaining input
        // length as an upper bound to avoid reallocations.
        let prefix = &self.input[content_start..i];
        let mut buf: Vec<u8> = Vec::with_capacity(self.input.len() - content_start);
        buf.extend_from_slice(prefix);
        self.position = i;

        loop {
            match self.peek() {
                None => {
                    return Err(JsonError::UnexpectedEndOfInput {
                        expected: "closing quote".to_string(),
                        position: start,
                    });
                }
                Some(b'"') => {
                    self.advance(); // skip closing "
                    break;
                }
                Some(b'\\') => {
                    // found a backslash, handle the escape sequence.
                    // advance() past the backslash first.
                    self.advance();
                    let escaped_char = self.parse_escape_sequence()?;
                    // char::encode_utf8 writes the UTF-8 bytes of the char
                    // into a small stack buffer; we then append them.
                    let mut tmp = [0u8; 4];
                    let encoded = escaped_char.encode_utf8(&mut tmp);
                    buf.extend_from_slice(encoded.as_bytes());
                }
                Some(byte) => {
                    // Copy the raw byte through. UTF-8 multi-byte
                    // continuation bytes never equal b'"' (0x22) or b'\\'
                    // (0x5C), so this loop naturally preserves multi-byte
                    // characters without needing to decode them.
                    buf.push(byte);
                    self.advance();
                }
            }
        }
        // The buffer is valid UTF-8 by construction.
        let result = String::from_utf8(buf).map_err(|_| JsonError::UnexpectedToken {
            expected: "valid UTF-8 string contents".to_string(),
            found: "invalid UTF-8".to_string(),
            position: start,
        })?;
        Ok(Token::String(result))
    }

    //
    // ── ESCAPE SEQUENCE PARSING ───────────────────────────────────────
    //
    // Called after we've already consumed the backslash. Now we need to
    // look at the NEXT byte to figure out what escape this is.
    //
    // Python handles these in string literals automatically, but we're
    // building a JSON parser in Rust, so yeah, we're screwed.
    //
    fn parse_escape_sequence(&mut self) -> Result<char, JsonError> {
        let pos = self.position;
        match self.advance() {
            Some(b'"') => Ok('"'),
            Some(b'\\') => Ok('\\'),
            Some(b'/') => Ok('/'),
            Some(b'b') => Ok('\u{0008}'),
            Some(b'f') => Ok('\u{000C}'),
            // For \n, \r, \t — Rust has first-class escape literals, so you
            // can just write '\n' directly. No need for '\u{000A}'.
            Some(b'n') => Ok('\n'),
            Some(b'r') => Ok('\r'),
            Some(b't') => Ok('\t'),
            Some(b'u') => self.parse_unicode_escape(),
            Some(c) => Err(JsonError::InvalidEscape {
                ch: char::from(c),
                position: pos,
            }),
            None => Err(JsonError::UnexpectedEndOfInput {
                expected: "escape character".to_string(),
                position: pos,
            }),
        }
    }

    //
    // ── UNICODE ESCAPE PARSING (\uXXXX) ──────────────────────────────
    //
    // After consuming \u, we need exactly 4 hex digits, so we:
    //   1. Read 4 bytes
    //   2. Verify they're all valid hex (0-9, a-f, A-F)
    //   3. Parse the hex string to a u32 number
    //   4. Convert that number to a char
    //   5. Take 2 Advil for our headache
    //   6. Carry on without complaining
    //
    // Python comparison:
    //   hex_str = "0041"
    //   code_point = int(hex_str, 16)   # → 65
    //   character = chr(code_point)      # → 'A'
    //
    // Rust equivalent uses u32::from_str_radix() and char::from_u32().
    // No, I didn't just know that off the top of my head.
    //
    fn parse_unicode_escape(&mut self) -> Result<char, JsonError> {
        let start_pos = self.position;

        // Need exactly 4 bytes left in the input for the hex digits.
        if self.position + 4 > self.input.len() {
            // Consume whatever's left so the position field points past the
            // malformed sequence, and report what we saw.
            let tail = &self.input[self.position..];
            self.position = self.input.len();
            let seq = std::str::from_utf8(tail).unwrap_or("").to_string();
            return Err(JsonError::InvalidUnicode {
                sequence: seq,
                position: start_pos,
            });
        }

        let hex_bytes = &self.input[self.position..self.position + 4];
        if !hex_bytes.iter().all(|b| b.is_ascii_hexdigit()) {
            // Not all hex digits. Pull out whatever prefix IS hex for the
            // error message (mirrors the old behaviour) then bail.
            let kept: String = hex_bytes
                .iter()
                .take_while(|b| b.is_ascii_hexdigit())
                .map(|b| char::from(*b))
                .collect();
            self.position += 4;
            return Err(JsonError::InvalidUnicode {
                sequence: kept,
                position: start_pos,
            });
        }
        self.position += 4;

        // hex_bytes is ASCII by the check above, so from_utf8 is infallible.
        let hex_str =
            std::str::from_utf8(hex_bytes).expect("hex digits are ASCII");
        let code_point =
            u32::from_str_radix(hex_str, 16).map_err(|_| JsonError::InvalidUnicode {
                sequence: hex_str.to_string(),
                position: start_pos,
            })?;

        char::from_u32(code_point).ok_or(JsonError::InvalidUnicode {
            // Unicode scalar value (surrogate pairs, values > 0x10FFFF).
            // .ok_or() converts None → Err, Some(ch) → Ok(ch).
            sequence: hex_str.to_string(),
            position: start_pos,
        })
    }

    //
    // ── NUMBER TOKENIZATION ───────────────────────────────────────────
    //
    // We read digit-like bytes (and -, .) until we hit a non-numeric byte,
    // then parse the accumulated slice as f64,
    // then take 2 more Advil.
    //
    fn tokenize_number(&mut self) -> Result<Token, JsonError> {
        let start = self.position;

        while let Some(ch) = self.peek() {
            // Accept digits plus '.', '-', 'e', 'E', '+' so scientific
            // notation (1.5e-3) survives. The final f64::parse() will
            // reject anything that isn't really a number.
            if ch.is_ascii_digit()
                || ch == b'.'
                || ch == b'-'
                || ch == b'e'
                || ch == b'E'
                || ch == b'+'
            {
                self.advance();
            } else {
                break;
            }
        }

        // Slice the number bytes directly out of the input. No intermediate
        // String allocation — we go straight from &[u8] → &str → f64.
        let num_bytes = &self.input[start..self.position];
        // UTF-8 safety: all accepted bytes above are ASCII, so this slice is
        // valid UTF-8.
        let num_str = std::str::from_utf8(num_bytes).expect("numeric bytes are ASCII");

        match num_str.parse::<f64>() {
            Ok(n) => Ok(Token::Number(n)),
            Err(_) => Err(JsonError::InvalidNumber {
                value: num_str.to_string(),
                position: start,
            }),
        }
    }

    //
    // ── KEYWORD MATCHING ──────────────────────────────────────────────
    //
    // Checks that the next N bytes match the expected keyword exactly.
    // Used for "true", "false", "null".
    //
    fn expect_keyword(&mut self, keyword: &str) -> Result<(), JsonError> {
        let start = self.position;
        for expected_byte in keyword.as_bytes() {
            match self.advance() {
                Some(b) if b == *expected_byte => {}
                _ => {
                    // Report whatever bytes we consumed so far as the 'found'
                    // substring. These are ASCII by construction (we only got
                    // here because the dispatch matched an ASCII letter).
                    let found = std::str::from_utf8(&self.input[start..self.position])
                        .unwrap_or("")
                        .to_string();
                    return Err(JsonError::UnexpectedToken {
                        expected: keyword.to_string(),
                        found,
                        position: start,
                    });
                }
            }
        }
        Ok(())
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS, so many tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    // === Struct Usage Tests ===

    #[test]
    fn test_tokenizer_struct_creation() {}

    #[test]
    fn test_tokenizer_multiple_tokens() {
        let mut tokenizer = Tokenizer::new("123 456");
        let tokens = tokenizer.tokenize().unwrap();
        assert_eq!(tokens.len(), 2);
    }

    // === Basic Token Tests ===

    #[test]
    fn test_tokenize_number() {
        let mut tokenizer = Tokenizer::new("42");
        let tokens = tokenizer.tokenize().unwrap();
        assert_eq!(tokens, vec![Token::Number(42.0)]);
    }

    #[test]
    fn test_tokenize_negative_number() {
        let mut tokenizer = Tokenizer::new("-3.14");
        let tokens = tokenizer.tokenize().unwrap();
        assert_eq!(tokens, vec![Token::Number(-3.14)]);
    }

    #[test]
    fn test_tokenize_literals() {
        let mut t1 = Tokenizer::new("true");
        assert_eq!(t1.tokenize().unwrap(), vec![Token::Boolean(true)]);

        let mut t2 = Tokenizer::new("false");
        assert_eq!(t2.tokenize().unwrap(), vec![Token::Boolean(false)]);

        let mut t3 = Tokenizer::new("null");
        assert_eq!(t3.tokenize().unwrap(), vec![Token::Null]);
    }

    #[test]
    fn test_tokenize_simple_string() {
        let mut tokenizer = Tokenizer::new(r#""hello""#);
        let tokens = tokenizer.tokenize().unwrap();
        assert_eq!(tokens, vec![Token::String("hello".to_string())]);
    }

    // === Escape Sequence Tests ===

    #[test]
    fn test_escape_newline() {
        let mut tokenizer = Tokenizer::new(r#""hello\nworld""#);
        let tokens = tokenizer.tokenize().unwrap();
        assert_eq!(tokens, vec![Token::String("hello\nworld".to_string())]);
    }

    #[test]
    fn test_escape_tab() {
        let mut tokenizer = Tokenizer::new(r#""col1\tcol2""#);
        let tokens = tokenizer.tokenize().unwrap();
        assert_eq!(tokens, vec![Token::String("col1\tcol2".to_string())]);
    }

    #[test]
    fn test_escape_quote() {
        let mut tokenizer = Tokenizer::new(r#""say \"hi\"""#);
        let tokens = tokenizer.tokenize().unwrap();
        assert_eq!(tokens, vec![Token::String("say \"hi\"".to_string())]);
    }

    #[test]
    fn test_escape_backslash() {
        let mut tokenizer = Tokenizer::new(r#""path\\to\\file""#);
        let tokens = tokenizer.tokenize().unwrap();
        assert_eq!(tokens, vec![Token::String("path\\to\\file".to_string())]);
    }

    #[test]
    fn test_escape_forward_slash() {
        let mut tokenizer = Tokenizer::new(r#""a\/b""#);
        let tokens = tokenizer.tokenize().unwrap();
        assert_eq!(tokens, vec![Token::String("a/b".to_string())]);
    }

    #[test]
    fn test_escape_carriage_return() {
        let mut tokenizer = Tokenizer::new(r#""line\r\n""#);
        let tokens = tokenizer.tokenize().unwrap();
        assert_eq!(tokens, vec![Token::String("line\r\n".to_string())]);
    }

    #[test]
    fn test_escape_backspace_formfeed() {
        let mut tokenizer = Tokenizer::new(r#""\b\f""#);
        let tokens = tokenizer.tokenize().unwrap();
        assert_eq!(tokens, vec![Token::String("\u{0008}\u{000C}".to_string())]);
    }

    #[test]
    fn test_multiple_escapes() {
        let mut tokenizer = Tokenizer::new(r#""a\nb\tc\"""#);
        let tokens = tokenizer.tokenize().unwrap();
        assert_eq!(tokens, vec![Token::String("a\nb\tc\"".to_string())]);
    }

    // === Unicode Escape Tests ===

    #[test]
    fn test_unicode_escape_basic() {
        // A is 'A'
        let mut tokenizer = Tokenizer::new(r#""\u0041""#);
        let tokens = tokenizer.tokenize().unwrap();
        assert_eq!(tokens, vec![Token::String("A".to_string())]);
    }

    #[test]
    fn test_unicode_escape_multiple() {
        // Hi is "Hi"
        let mut tokenizer = Tokenizer::new(r#""\u0048\u0069""#);
        let tokens = tokenizer.tokenize().unwrap();
        assert_eq!(tokens, vec![Token::String("Hi".to_string())]);
    }

    #[test]
    fn test_unicode_escape_mixed() {
        let mut tokenizer = Tokenizer::new(r#""Hello \u0057orld""#);
        let tokens = tokenizer.tokenize().unwrap();
        assert_eq!(tokens, vec![Token::String("Hello World".to_string())]);
    }

    #[test]
    fn test_unicode_escape_lowercase() {
        let mut tokenizer = Tokenizer::new(r#""\u004a""#);
        let tokens = tokenizer.tokenize().unwrap();
        assert_eq!(tokens, vec![Token::String("J".to_string())]);
    }

    // === Error Tests OMG SO MANY TESTS ===

    #[test]
    fn test_invalid_escape_sequence() {
        let mut tokenizer = Tokenizer::new(r#""\q""#);
        let result = tokenizer.tokenize();
        assert!(matches!(result, Err(JsonError::InvalidEscape { .. })));
    }

    #[test]
    fn test_invalid_unicode_too_short() {
        let mut tokenizer = Tokenizer::new(r#""\u004""#);
        let result = tokenizer.tokenize();
        assert!(matches!(result, Err(JsonError::InvalidUnicode { .. })));
    }

    #[test]
    fn test_invalid_unicode_bad_hex() {
        let mut tokenizer = Tokenizer::new(r#""\u00GG""#);
        let result = tokenizer.tokenize();
        assert!(matches!(result, Err(JsonError::InvalidUnicode { .. })));
    }

    #[test]
    fn test_unterminated_string_with_escape() {
        let mut tokenizer = Tokenizer::new(r#""hello\n"#);
        let result = tokenizer.tokenize();
        assert!(result.is_err());
    }
}
