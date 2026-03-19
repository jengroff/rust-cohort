use rust_json_parser::tokenizer;

fn main() {                                                                                                                              
    let tokens = tokenizer::tokenize(r#"{"name": "Alice"}"#);                                                                            
    println!("{:?}", tokens);                                                                                                            
}   
