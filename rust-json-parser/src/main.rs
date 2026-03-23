use rust_json_parser::tokenizer;

fn main() {
    let json_string = r#"{
        "name": "Alice Johnson",
        "age": 28,
        "email": "alice@example.com"
        }"#;
    let tokens = tokenizer::tokenize(json_string);
    println!("{:?}", tokens);
}
