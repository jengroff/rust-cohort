use rust_json_parser::{JsonError, parse_json};

fn main() -> Result<(), JsonError> {
    let json_string = r#"{
        "name": "Alice Johnson",
        "age": 28,
        "email": "alice@example.com"
        }"#;
    let value = parse_json(json_string);
    println!("Parsed: {:?}", value);

    Ok(())
}
