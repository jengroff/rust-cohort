use rust_json_parser::parser::JsonParser;
use rust_json_parser::tokenizer::Tokenizer;

fn main() {
    // Tokenizer struct
    let json_string = r#"{"name": "Alice", "age": 30}"#;
    let mut tokenizer = Tokenizer::new(json_string);
    match tokenizer.tokenize() {
        Ok(tokens) => println!("Tokens: {:?}", tokens),
        Err(e) => println!("Tokenize error: {}", e),
    }

    // Full parse pipeline with escapes
    let escaped_json = r#""Hello\nWorld\t\"quoted\"\u0041""#;
    parse_and_print(escaped_json);

    // Error case
    let bad_json = r#""\q""#;
    parse_and_print(bad_json);
}

// Helper function
fn parse_and_print(input: &str) {
    let result = JsonParser::new(input).and_then(|mut p| p.parse());
    match result {
        Ok(value) => println!("Parsed: {:?}", value),
        Err(e) => println!("Error: {}", e),
    }
}
