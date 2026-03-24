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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_braces() {
        let tokens = tokenize("{}").unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], Token::LeftBrace);
        assert_eq!(tokens[1], Token::RightBrace);
    }

    #[test]
    fn test_simple_string() {
        let tokens = tokenize(r#""hello""#).unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::String("hello".to_string()));
    }

    #[test]
    fn test_number() {
        let tokens = tokenize("42").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Number(42.0));
    }

    #[test]
    fn test_tokenize_string() {
        let tokens = tokenize(r#""hello world""#).unwrap();

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::String("hello world".to_string()));
    }

    #[test]
    fn test_boolean_and_null() {
        let tokens = tokenize("true false null").unwrap();
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], Token::Boolean(true));
        assert_eq!(tokens[1], Token::Boolean(false));
        assert_eq!(tokens[2], Token::Null);
    }

    #[test]
    fn test_simple_object() {
        let tokens = tokenize(r#"{"name": "Alice"}"#).unwrap();
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0], Token::LeftBrace);
        assert_eq!(tokens[1], Token::String("name".to_string()));
        assert_eq!(tokens[2], Token::Colon);
        assert_eq!(tokens[3], Token::String("Alice".to_string()));
        assert_eq!(tokens[4], Token::RightBrace);
    }

    #[test]
    fn test_multiple_values() {
        let tokens = tokenize(r#"{"age": 30, "active": true}"#).unwrap();

        // Verify we have the right tokens
        assert!(tokens.contains(&Token::String("age".to_string())));
        assert!(tokens.contains(&Token::Number(30.0)));
        assert!(tokens.contains(&Token::Comma));
        assert!(tokens.contains(&Token::String("active".to_string())));
        assert!(tokens.contains(&Token::Boolean(true)));
    }

    // String boundary tests - verify inner vs outer quote handling

    #[test]
    fn test_empty_string() {
        // Outer boundary: adjacent quotes with no inner content
        let tokens = tokenize(r#""""#).unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::String("".to_string()));
    }

    #[test]
    fn test_string_containing_json_special_chars() {
        // Inner handling: JSON delimiters inside strings don't break tokenization
        let tokens = tokenize(r#""{key: value}""#).unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::String("{key: value}".to_string()));
    }

    #[test]
    fn test_string_with_keyword_like_content() {
        // Inner handling: "true", "false", "null" inside strings stay as string content
        let tokens = tokenize(r#""not true or false""#).unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::String("not true or false".to_string()));
    }

    #[test]
    fn test_string_with_number_like_content() {
        // Inner handling: numeric content inside strings doesn't become Number tokens
        let tokens = tokenize(r#""phone: 555-1234""#).unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::String("phone: 555-1234".to_string()));
    }

    // Number parsing tests

    #[test]
    fn test_negative_number() {
        let tokens = tokenize("-42").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Number(-42.0));
    }

    #[test]
    fn test_decimal_number() {
        let tokens = tokenize("0.5").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Number(0.5));
    }

    #[test]
    fn test_leading_decimal_not_a_number() {
        // .5 is invalid JSON - numbers must have leading digit (0.5 is valid)
        assert!(tokenize(".5").is_err());
    }
}
