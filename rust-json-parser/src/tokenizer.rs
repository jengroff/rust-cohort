use crate::error::JsonError;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Colon,
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
}

// Making heavy use of inline comments to understand what
// is happening and for future readability.
//
// -------- THE STRUCT ---------------------------
// Tokenizer itself is public but its attributes are private
//
pub struct Tokenizer {
    input: Vec<char>,
    position: usize,
}

impl Tokenizer {
    // Everything inside this is either a method (takes self)
    // or an associated function (no self) - kind of like @classmethod.
    //
    pub fn new(input: &str) -> Self {
        // Self = Tokenizer; type of alias?
        // parameter type is `input: &str` because we are borrowing
        // the input string (caller keeps ownership of their string) but then
        // we immediately convert it to an owned Vec<char>.
        //
        Self {
            input: input.chars().collect(),
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
    pub fn tokenize(&mut self) -> Result<Vec<Token>, JsonError> {
        let mut tokens = Vec::new();
        loop {
            match self.peek() {
                Some('{') => {
                    self.advance();
                    tokens.push(Token::LeftBrace);
                }
                Some('}') => {
                    self.advance();
                    tokens.push(Token::RightBrace);
                }
                Some('[') => {
                    self.advance();
                    tokens.push(Token::LeftBracket);
                }
                Some(']') => {
                    self.advance();
                    tokens.push(Token::RightBracket);
                }
                Some(',') => {
                    self.advance();
                    tokens.push(Token::Comma);
                }
                Some(':') => {
                    self.advance();
                    tokens.push(Token::Colon);
                }
                Some('"') => {
                    let token = self.tokenize_string()?;
                    tokens.push(token);
                }
                Some('0'..='9') | Some('-') => {
                    let token = self.tokenize_number()?;
                    tokens.push(token);
                }
                Some('t') => {
                    self.expect_keyword("true")?;
                    tokens.push(Token::Boolean(true));
                }
                Some('f') => {
                    self.expect_keyword("false")?;
                    tokens.push(Token::Boolean(false));
                }
                Some('n') => {
                    self.expect_keyword("null")?;
                    tokens.push(Token::Null);
                }
                Some(c) if c.is_ascii_whitespace() => {
                    self.advance();
                }
                Some(other) => {
                    return Err(JsonError::UnexpectedToken {
                        expected: "valid JSON token".to_string(),
                        found: other.to_string(),
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

    fn advance(&mut self) -> Option<char> {
        let ch = self.input.get(self.position).copied();
        if ch.is_some() {
            self.position += 1;
        }
        ch
    }

    // Look at the current character without moving the cursor.
    // &self (not &mut self) because peeking is read-only.
    fn peek(&self) -> Option<char> {
        self.input.get(self.position).copied()
    }

    //
    // ── STRING TOKENIZATION with escape sequences (oh the pain) ──────────────────
    //
    // This monster method handles:
    // 1. Regular characters (push them)
    // 2. Escape sequences
    // 3. Unicode escapes
    // 4. Error cases (unterminated string, invalid escape, bad unicode)
    //

    fn tokenize_string(&mut self) -> Result<Token, JsonError> {
        let start = self.position;
        self.advance(); // skip opening "
        let mut result = String::new();

        loop {
            match self.peek() {
                None => {
                    return Err(JsonError::UnexpectedEndOfInput {
                        expected: "closing quote".to_string(),
                        position: start,
                    });
                }
                Some('"') => {
                    self.advance(); // skip closing "
                    break;
                }
                Some('\\') => {
                    // found a backslash, handle the escape sequence.
                    // advance() past the backslash first.
                    self.advance();
                    let escaped_char = self.parse_escape_sequence()?;
                    result.push(escaped_char);
                    // could I match against self.advance() and inline the
                    // escape handling and spare the separate method?
                }
                Some(ch) => {
                    result.push(ch);
                    self.advance();
                }
            }
        }
        Ok(Token::String(result))
    }

    //
    // ── ESCAPE SEQUENCE PARSING ───────────────────────────────────────
    //
    // Called after we've already consumed the backslash. Now we need to
    // look at the NEXT character to figure out what escape this is.
    //
    // Python handles these in string literals automatically, but we're
    // building a JSON parser in Rust, so yeah, we're screwed.
    //
    fn parse_escape_sequence(&mut self) -> Result<char, JsonError> {
        let pos = self.position;
        match self.advance() {
            Some('"') => Ok('"'),
            Some('\\') => Ok('\\'),
            Some('/') => Ok('/'),
            Some('b') => Ok('\u{0008}'),
            Some('f') => Ok('\u{000C}'),
            // For \n, \r, \t — Rust has first-class escape literals, so you
            // can just write '\n' directly. No need for '\u{000A}'.
            Some('n') => Ok('\n'),
            Some('r') => Ok('\r'),
            Some('t') => Ok('\t'),
            Some('u') => self.parse_unicode_escape(),
            Some(c) => Err(JsonError::InvalidEscape {
                ch: c,
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
    //   1. Read 4 characters
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
        let mut hex_str = String::with_capacity(4);

        for _ in 0..4 {
            match self.advance() {
                Some(c) if c.is_ascii_hexdigit() => hex_str.push(c),
                Some(_) | None => {
                    return Err(JsonError::InvalidUnicode {
                        sequence: hex_str,
                        position: start_pos,
                    });
                }
            }
        }

        let code_point =
            // .map_err() converts the ParseIntError to our JsonError type.
            // ? then propagates the error if it's Err.
            u32::from_str_radix(&hex_str, 16).map_err(|_| JsonError::InvalidUnicode {
                sequence: hex_str.clone(),
                position: start_pos,
            })?;

        char::from_u32(code_point).ok_or(JsonError::InvalidUnicode {
            // Unicode scalar value (surrogate pairs, values > 0x10FFFF).
            // .ok_or() converts None → Err, Some(ch) → Ok(ch).
            sequence: hex_str,
            position: start_pos,
        })
    }

    //
    // ── NUMBER TOKENIZATION ───────────────────────────────────────────
    //
    // We read digits (and -, .) until we hit a non-numeric char,
    // then parse the accumulated string as f64,
    // then take 2 more Advil.
    //
    fn tokenize_number(&mut self) -> Result<Token, JsonError> {
        let start = self.position;
        let mut num_str = String::new();

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() || ch == '.' || ch == '-' {
                num_str.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        match num_str.parse::<f64>() {
            Ok(n) => Ok(Token::Number(n)),
            Err(_) => Err(JsonError::InvalidNumber {
                value: num_str,
                position: start,
            }),
        }
    }

    //
    // ── KEYWORD MATCHING ──────────────────────────────────────────────
    //
    // Checks that the next N characters match the expected keyword exactly.
    // Used for "true", "false", "null".
    //
    fn expect_keyword(&mut self, keyword: &str) -> Result<(), JsonError> {
        let start = self.position;
        for expected_ch in keyword.chars() {
            match self.advance() {
                Some(ch) if ch == expected_ch => {}
                _ => {
                    return Err(JsonError::UnexpectedToken {
                        expected: keyword.to_string(),
                        found: self.input[start..self.position].iter().collect::<String>(),
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
        // \u0041 is 'A'
        let mut tokenizer = Tokenizer::new(r#""\u0041""#);
        let tokens = tokenizer.tokenize().unwrap();
        assert_eq!(tokens, vec![Token::String("A".to_string())]);
    }

    #[test]
    fn test_unicode_escape_multiple() {
        // \u0048\u0069 is "Hi"
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
