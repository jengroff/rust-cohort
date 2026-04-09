use crate::error::JsonError;
use crate::tokenizer::{Token, Tokenizer};
use crate::value::JsonValue;

pub struct JsonParser {
    tokens: Vec<Token>,
    position: usize,
}

impl JsonParser {
    //
    // Returns Result<Self, JsonError> because tokenization can fail.
    // This is different from Tokenizer::new() which was infallible, like Superman.
    //
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
    // Takes the first token and converts it to a JsonValue.
    // We're only parsing primitive values (not arrays or objects).
    //
    pub fn parse(&mut self) -> Result<JsonValue, JsonError> {
        if self.is_at_end() {
            return Err(JsonError::UnexpectedEndOfInput {
                expected: "JSON value".to_string(),
                position: 0,
            });
        }

        // Separating "get the token" from "interpret the token" into
        // two steps. The .ok_or() converts Option<Token> → Result<Token>.

        let token = self.advance().ok_or(JsonError::UnexpectedEndOfInput {
            expected: "JSON value".to_string(),
            position: self.position,
        })?;

        match token {
            Token::Null => Ok(JsonValue::Null),
            Token::Boolean(b) => Ok(JsonValue::Boolean(b)),
            Token::Number(n) => Ok(JsonValue::Number(n)),
            Token::String(s) => Ok(JsonValue::Text(s)),
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

    // Rewrite #2 (within inline comments) ->
    // because Rewrite #2 threw a warning -->  #[warn(clippy::redundant_pattern_matching)] :-(
    //
    fn advance(&mut self) -> Option<Token> {
        match self.tokens.get(self.position).cloned() {
            // Using match to handle both cases explicitly:
            //    Some(token) -> there is a token at that position; binds it to token (unlike _),
            //    then increments the position, then rewraps it in Some(token) to return it.
            Some(token) => {
                self.position += 1;
                Some(token)
            }
            None => None,
            // None -> OOB, just pass None through
        }
        // key difference from the rewrite #1 version -> match forces me to handle every
        // case, necessitating the explicit None => None arm.
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.tokens.len()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    // === Struct Usage Tests ===

    #[test]
    fn test_parser_creation() {
        let parser = JsonParser::new("42");
        assert!(parser.is_ok());
    }

    #[test]
    fn test_parser_creation_tokenize_error() {
        let parser = JsonParser::new(r#""\q""#); // Invalid escape
        assert!(parser.is_err());
    }

    // === Primitive Parsing Tests ===

    #[test]
    fn test_parse_number() {
        let mut parser = JsonParser::new("42").unwrap();
        let value = parser.parse().unwrap();
        assert_eq!(value, JsonValue::Number(42.0));
    }

    #[test]
    fn test_parse_negative_number() {
        let mut parser = JsonParser::new("-3.14").unwrap();
        let value = parser.parse().unwrap();
        assert_eq!(value, JsonValue::Number(-3.14));
    }

    #[test]
    fn test_parse_boolean_true() {
        let mut parser = JsonParser::new("true").unwrap();
        let value = parser.parse().unwrap();
        assert_eq!(value, JsonValue::Boolean(true));
    }

    #[test]
    fn test_parse_boolean_false() {
        let mut parser = JsonParser::new("false").unwrap();
        let value = parser.parse().unwrap();
        assert_eq!(value, JsonValue::Boolean(false));
    }

    #[test]
    fn test_parse_null() {
        let mut parser = JsonParser::new("null").unwrap();
        let value = parser.parse().unwrap();
        assert_eq!(value, JsonValue::Null);
    }

    #[test]
    fn test_parse_simple_string() {
        let mut parser = JsonParser::new(r#""hello""#).unwrap();
        let value = parser.parse().unwrap();
        assert_eq!(value, JsonValue::Text("hello".to_string()));
    }

    // === Escape Sequence Integration Tests ===

    #[test]
    fn test_parse_string_with_newline() {
        let mut parser = JsonParser::new(r#""hello\nworld""#).unwrap();
        let value = parser.parse().unwrap();
        assert_eq!(value, JsonValue::Text("hello\nworld".to_string()));
    }

    #[test]
    fn test_parse_string_with_tab() {
        let mut parser = JsonParser::new(r#""col1\tcol2""#).unwrap();
        let value = parser.parse().unwrap();
        assert_eq!(value, JsonValue::Text("col1\tcol2".to_string()));
    }

    #[test]
    fn test_parse_string_with_quotes() {
        let mut parser = JsonParser::new(r#""say \"hi\"""#).unwrap();
        let value = parser.parse().unwrap();
        assert_eq!(value, JsonValue::Text("say \"hi\"".to_string()));
    }

    #[test]
    fn test_parse_string_with_unicode() {
        let mut parser = JsonParser::new(r#""\u0048\u0065\u006c\u006c\u006f""#).unwrap();
        let value = parser.parse().unwrap();
        assert_eq!(value, JsonValue::Text("Hello".to_string()));
    }

    #[test]
    fn test_parse_complex_escapes() {
        let mut parser = JsonParser::new(r#""line1\nline2\t\"quoted\"\u0021""#).unwrap();
        let value = parser.parse().unwrap();
        assert_eq!(
            value,
            JsonValue::Text("line1\nline2\t\"quoted\"!".to_string())
        );
    }

    // === Error Tests ===

    #[test]
    fn test_parse_empty_input() {
        let parser = JsonParser::new("");
        assert!(parser.is_err() || parser.unwrap().parse().is_err());
    }

    #[test]
    fn test_parse_whitespace_only() {
        let parser = JsonParser::new("   ");
        assert!(parser.is_err() || parser.unwrap().parse().is_err());
    }
}
