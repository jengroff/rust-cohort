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


pub fn tokenize(input: &str) -> Result<Vec<Token>, JsonError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '{' => {
                tokens.push(Token::LeftBrace);
                i += 1;
            }
            '}' => {
                tokens.push(Token::RightBrace);
                i += 1;
            }
            '[' => {
                tokens.push(Token::LeftBracket);
                i += 1;
            }
            ']' => {
                tokens.push(Token::RightBracket);
                i += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            ':' => {
                tokens.push(Token::Colon);
                i += 1;
            }

            '"' => {
                let start = i;
                i += 1;
                let mut s = String::new();
                while i < chars.len() && chars[i] != '"' {
                    s.push(chars[i]);
                    i += 1;
                }

                if i >= chars.len() {
                    return Err(JsonError::UnexpectedEndOfInput {
                        expected: "closing quote".to_string(),
                        position: start,
                    });
                }
                i += 1;
                tokens.push(Token::String(s));
            }

            '0'..='9' | '-' => {
                let start = i;
                let mut num_str = String::new();
                while i < chars.len()
                    && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '-')
                {
                    num_str.push(chars[i]);
                    i += 1;
                }

                match num_str.parse::<f64>() {
                    Ok(n) => tokens.push(Token::Number(n)),
                    Err(_) => {
                        return Err(JsonError::InvalidNumber {
                            value: num_str,
                            position: start,
                        });
                    }
                }
            }

            't' | 'f' | 'n' => {
                let remaining = &input[i..];
                if remaining.starts_with("true") {
                    tokens.push(Token::Boolean(true));
                    i += 4;
                } else if remaining.starts_with("false") {
                    tokens.push(Token::Boolean(false));
                    i += 5;
                } else if remaining.starts_with("null") {
                    tokens.push(Token::Null);
                    i += 4;
                } else {
                    return Err(JsonError::UnexpectedToken {
                        expected: "valid JSON keyword".to_string(),
                        found: chars[i].to_string(),
                        position: i,
                    });
                }
            }

            c if c.is_ascii_whitespace() => {
                i += 1;
            }

            other => {
                return Err(JsonError::UnexpectedToken {
                    expected: "valid JSON token".to_string(),
                    found: other.to_string(),
                    position: i,
                });
            }
        }
    }
    Ok(tokens)
}


// Making heavy use of inline comments to understand what
// is happening and for future readability.
//
// -------- THE STRUCT ---------------------------
//
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
    // ---- PUBLIC METHOD: tokenize -----------------------------
    //
    // &mut self = method can read and mutate the struct's fields.
    // We need &mut because tokenize advances self.position.
    //
    // Returns Result<Vec<Token>, JsonError> - same as before, but now 
    // the logic lives inside a method instead of a free function.
    //
    pub fn tokenize(&mut self) -> Result<Vec<Token>, JsonError> {
        let mut tokens = Vec::new();

        while !self.is_at_end() {
            // peek() looks at the current char WITHOUT advancing.
            // It returns Option<char> — Some(ch) if there's a char, None if at end.
            match self.peek() {
                Some('{') => { self.advance(); tokens.push(Token::LeftBrace); }
                Some('}') => { self.advance(); tokens.push(Token::RightBrace); }
                Some('[') => { self.advance(); tokens.push(Token::LeftBracket); }
                Some(']') => { self.advance(); tokens.push(Token::RightBracket); }
                Some(',') => { self.advance(); tokens.push(Token::Comma); }
                Some(':') => { self.advance(); tokens.push(Token::Colon); }

                // Some('"') => {
                //     let token = self.tokenize_string()?;
                //     tokens.push(token);
                // }
                // Some('0'..='9') | Some('-') => {
                //     let token = self.tokenize_number()?;
                //     tokens.push(token);
                // }
                // Some('t') => {
                //     self.expect_keyword("true")?;
                //     tokens.push(Token::Boolean(true));
                // }
                // Some('f') => {
                //     self.expect_keyword("false")?;
                //     tokens.push(Token::Boolean(false));
                // }
                // Some('n') => {
                //     self.expect_keyword("null")?;
                //     tokens.push(Token::Null);
                // }
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

    fn advance(&mut self) -> Option<char> {
        if self.position < self.input.len() {
            let ch = self.input[self.position];
            self.position += 1;
            Some(ch)
        } else {
            None
        }
    }

    fn peek(&self) -> Option<char> {
         if self.position < self.input.len() {
             Some(self.input[self.position])
         } else {
             None
         }
     }

    fn is_at_end(&self) -> bool {
        self.position >= self.input.len()
    }
}








// #[cfg(test)]
// mod tests {
//     use super::*;

//     // === Struct Usage Tests ===

//     #[test]
//     fn test_tokenizer_struct_creation() {
//         let tokenizer = Tokenizer::new(r#""hello""#);
//         // Tokenizer should be created without error
//         // Internal state is private, so we test via tokenize()
//     }

//     #[test]
//     fn test_tokenizer_multiple_tokens() {
//         // Tests that a single tokenize() call handles multiple tokens
//         // Note: Unlike Python iterators, calling tokenize() again on the same
//         // instance would return empty - the input has been consumed.
//         // Create a new Tokenizer instance if you need to parse new input.
//         let mut tokenizer = Tokenizer::new("123 456");
//         let tokens = tokenizer.tokenize().unwrap();
//         assert_eq!(tokens.len(), 2);
//     }

//     // === Basic Token Tests (from Week 1 - ensure they still pass) ===

//     #[test]
//     fn test_tokenize_number() {
//         let mut tokenizer = Tokenizer::new("42");
//         let tokens = tokenizer.tokenize().unwrap();
//         assert_eq!(tokens, vec![Token::Number(42.0)]);
//     }

//     #[test]
//     fn test_tokenize_negative_number() {
//         let mut tokenizer = Tokenizer::new("-3.14");
//         let tokens = tokenizer.tokenize().unwrap();
//         assert_eq!(tokens, vec![Token::Number(-3.14)]);
//     }

//     #[test]
//     fn test_tokenize_literals() {
//         let mut t1 = Tokenizer::new("true");
//         assert_eq!(t1.tokenize().unwrap(), vec![Token::Boolean(true)]);

//         let mut t2 = Tokenizer::new("false");
//         assert_eq!(t2.tokenize().unwrap(), vec![Token::Boolean(false)]);

//         let mut t3 = Tokenizer::new("null");
//         assert_eq!(t3.tokenize().unwrap(), vec![Token::Null]);
//     }

//     #[test]
//     fn test_tokenize_simple_string() {
//         let mut tokenizer = Tokenizer::new(r#""hello""#);
//         let tokens = tokenizer.tokenize().unwrap();
//         assert_eq!(tokens, vec![Token::String("hello".to_string())]);
//     }

//     // === Escape Sequence Tests ===

//     #[test]
//     fn test_escape_newline() {
//         let mut tokenizer = Tokenizer::new(r#""hello\nworld""#);
//         let tokens = tokenizer.tokenize().unwrap();
//         assert_eq!(tokens, vec![Token::String("hello\nworld".to_string())]);
//     }

//     #[test]
//     fn test_escape_tab() {
//         let mut tokenizer = Tokenizer::new(r#""col1\tcol2""#);
//         let tokens = tokenizer.tokenize().unwrap();
//         assert_eq!(tokens, vec![Token::String("col1\tcol2".to_string())]);
//     }

//     #[test]
//     fn test_escape_quote() {
//         let mut tokenizer = Tokenizer::new(r#""say \"hello\"""#);
//         let tokens = tokenizer.tokenize().unwrap();
//         assert_eq!(tokens, vec![Token::String("say \"hello\"".to_string())]);
//     }

//     #[test]
//     fn test_escape_backslash() {
//         let mut tokenizer = Tokenizer::new(r#""path\\to\\file""#);
//         let tokens = tokenizer.tokenize().unwrap();
//         assert_eq!(tokens, vec![Token::String("path\\to\\file".to_string())]);
//     }

//     #[test]
//     fn test_escape_forward_slash() {
//         let mut tokenizer = Tokenizer::new(r#""a\/b""#);
//         let tokens = tokenizer.tokenize().unwrap();
//         assert_eq!(tokens, vec![Token::String("a/b".to_string())]);
//     }

//     #[test]
//     fn test_escape_carriage_return() {
//         let mut tokenizer = Tokenizer::new(r#""line\r\n""#);
//         let tokens = tokenizer.tokenize().unwrap();
//         assert_eq!(tokens, vec![Token::String("line\r\n".to_string())]);
//     }

//     #[test]
//     fn test_escape_backspace_formfeed() {
//         let mut tokenizer = Tokenizer::new(r#""\b\f""#);
//         let tokens = tokenizer.tokenize().unwrap();
//         assert_eq!(tokens, vec![Token::String("\u{0008}\u{000C}".to_string())]);
//     }

//     #[test]
//     fn test_multiple_escapes() {
//         let mut tokenizer = Tokenizer::new(r#""a\nb\tc\"""#);
//         let tokens = tokenizer.tokenize().unwrap();
//         assert_eq!(tokens, vec![Token::String("a\nb\tc\"".to_string())]);
//     }

//     // === Unicode Escape Tests ===

//     #[test]
//     fn test_unicode_escape_basic() {
//         // \u0041 is 'A'
//         let mut tokenizer = Tokenizer::new(r#""\u0041""#);
//         let tokens = tokenizer.tokenize().unwrap();
//         assert_eq!(tokens, vec![Token::String("A".to_string())]);
//     }

//     #[test]
//     fn test_unicode_escape_multiple() {
//         // \u0048\u0069 is "Hi"
//         let mut tokenizer = Tokenizer::new(r#""\u0048\u0069""#);
//         let tokens = tokenizer.tokenize().unwrap();
//         assert_eq!(tokens, vec![Token::String("Hi".to_string())]);
//     }

//     #[test]
//     fn test_unicode_escape_mixed() {
//         // Mix of regular chars and unicode escapes
//         let mut tokenizer = Tokenizer::new(r#""Hello \u0057orld""#);
//         let tokens = tokenizer.tokenize().unwrap();
//         assert_eq!(tokens, vec![Token::String("Hello World".to_string())]);
//     }

//     #[test]
//     fn test_unicode_escape_lowercase() {
//         // Lowercase hex digits should work too
//         let mut tokenizer = Tokenizer::new(r#""\u004a""#);
//         let tokens = tokenizer.tokenize().unwrap();
//         assert_eq!(tokens, vec![Token::String("J".to_string())]);
//     }

//     // === Error Tests ===

//     #[test]
//     fn test_invalid_escape_sequence() {
//         let mut tokenizer = Tokenizer::new(r#""\q""#);
//         let result = tokenizer.tokenize();
//         assert!(matches!(result, Err(JsonError::InvalidEscape { .. })));
//     }

//     #[test]
//     fn test_invalid_unicode_too_short() {
//         let mut tokenizer = Tokenizer::new(r#""\u004""#);
//         let result = tokenizer.tokenize();
//         assert!(matches!(result, Err(JsonError::InvalidUnicode { .. })));
//     }

//     #[test]
//     fn test_invalid_unicode_bad_hex() {
//         let mut tokenizer = Tokenizer::new(r#""\u00GG""#);
//         let result = tokenizer.tokenize();
//         assert!(matches!(result, Err(JsonError::InvalidUnicode { .. })));
//     }

//     #[test]
//     fn test_unterminated_string_with_escape() {
//         let mut tokenizer = Tokenizer::new(r#""hello\n"#);
//         let result = tokenizer.tokenize();
//         assert!(result.is_err());
//     }
// }