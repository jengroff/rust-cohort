use crate::enums::Token;

pub fn tokenize(input: &str) -> Vec<Token> {
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
                i += 1; // skip opening quote
                let mut s = String::new();
                while i < chars.len() && chars[i] != '"' {
                    s.push(chars[i]);
                    i += 1;
                }
                i += 1; // skip closing quote
                tokens.push(Token::String(s));
            }
            '0'..='9' | '-' => {
                let mut num_str = String::new();
                while i < chars.len()
                    && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '-')
                {
                    num_str.push(chars[i]);
                    i += 1;
                }
                if let Ok(n) = num_str.parse::<f64>() {
                    tokens.push(Token::Number(n));
                }
            }
            't' if chars[i..].starts_with(&['t', 'r', 'u', 'e']) => {
                tokens.push(Token::Boolean(true));
                i += 4;
            }
            'f' if chars[i..].starts_with(&['f', 'a', 'l', 's', 'e']) => {
                tokens.push(Token::Boolean(false));
                i += 5;
            }
            'n' if chars[i..].starts_with(&['n', 'u', 'l', 'l']) => {
                tokens.push(Token::Null);
                i += 4;
            }
            ' ' | '\n' | '\t' | '\r' => {
                i += 1;
            }
            _ => {
                println!("Unhandled character: '{}'", chars[i]);
                i += 1;
            }
        }
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_braces() {
        let tokens = tokenize("{}");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], Token::LeftBrace);
        assert_eq!(tokens[1], Token::RightBrace);
    }

    #[test]
    fn test_simple_string() {
        let tokens = tokenize(r#""hello""#);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::String("hello".to_string()));
    }

    #[test]
    fn test_number() {
        let tokens = tokenize("42");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Number(42.0));
    }

    #[test]
    fn test_tokenize_string() {
        let tokens = tokenize(r#""hello world""#);

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::String("hello world".to_string()));
    }

    #[test]
    fn test_boolean_and_null() {
        let tokens = tokenize("true false null");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], Token::Boolean(true));
        assert_eq!(tokens[1], Token::Boolean(false));
        assert_eq!(tokens[2], Token::Null);
    }

    #[test]
    fn test_simple_object() {
        let tokens = tokenize(r#"{"name": "Alice"}"#);
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0], Token::LeftBrace);
        assert_eq!(tokens[1], Token::String("name".to_string()));
        assert_eq!(tokens[2], Token::Colon);
        assert_eq!(tokens[3], Token::String("Alice".to_string()));
        assert_eq!(tokens[4], Token::RightBrace);
    }

    #[test]
    fn test_multiple_values() {
        let tokens = tokenize(r#"{"age": 30, "active": true}"#);

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
        let tokens = tokenize(r#""""#);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::String("".to_string()));
    }

    #[test]
    fn test_string_containing_json_special_chars() {
        // Inner handling: JSON delimiters inside strings don't break tokenization
        let tokens = tokenize(r#""{key: value}""#);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::String("{key: value}".to_string()));
    }

    #[test]
    fn test_string_with_keyword_like_content() {
        // Inner handling: "true", "false", "null" inside strings stay as string content
        let tokens = tokenize(r#""not true or false""#);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::String("not true or false".to_string()));
    }

    #[test]
    fn test_string_with_number_like_content() {
        // Inner handling: numeric content inside strings doesn't become Number tokens
        let tokens = tokenize(r#""phone: 555-1234""#);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::String("phone: 555-1234".to_string()));
    }

    // Number parsing tests

    #[test]
    fn test_negative_number() {
        let tokens = tokenize("-42");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Number(-42.0));
    }

    #[test]
    fn test_decimal_number() {
        let tokens = tokenize("0.5");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Number(0.5));
    }

    #[test]
    fn test_leading_decimal_not_a_number() {
        // .5 is invalid JSON - numbers must have leading digit (0.5 is valid)
        let tokens = tokenize(".5");
        // Should NOT be interpreted as 0.5
        assert!(!tokens.contains(&Token::Number(0.5)));
    }
}
