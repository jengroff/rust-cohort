use crate::parser::JsonParser;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Boolean(bool),
    Number(f64),
    Text(String),
    Array(Vec<JsonValue>),
    Object(HashMap<String, JsonValue>),
}

//
// Array(Vec<JsonValue>) — growable array. Vec lives on the heap,
//   owns its elements, and can grow or shrink dynamically.
//
//  Object(HashMap<String, JsonValue>) — hash map. Keys must be String (owned),
//   not &str (borrowed), because the HashMap needs to own its keys.
//

impl JsonValue {
    //
    // These methods all return Option<&T>, which is a borrowed
    // reference wrapped in Option (wrapped in an enigma, lol).
    // I think it means "give me a view into the data if the type is
    // right, otherwise None."
    //
    pub fn as_array(&self) -> Option<&Vec<JsonValue>> {
        match self {
            JsonValue::Array(arr) => Some(arr),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&HashMap<String, JsonValue>> {
        match self {
            JsonValue::Object(obj) => Some(obj),
            _ => None,
        }
    }
    //
    // as_array() / as_object() are kind of like Python's
    //   isinstance(val, list)
    //   isinstance(val, dict)
    // but collapsed into one pithy call that returns None on mismatch

    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(obj) => obj.get(key),
            _ => None,
        }
    }
    //
    // get(key) delegates to HashMap::get() which is the safe Option-returning
    // version, unlike get_index(i) which would panic if OOB. Seems similar to
    // Python's get() method for dict.

    pub fn get_index(&self, index: usize) -> Option<&JsonValue> {
        match self {
            JsonValue::Array(arr) => arr.get(index),
            _ => None,
        }
    }
}

//
// HELPER function to handle the reverse of what tokenizer does with escape
// sequences, which is to turn \" in the input into a literal " in the parsed
// string. But Display needs to torn that " back into \" in the output so the
// result is valid JSON. Sigh.

fn escape_json_string(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    //
    // ok this is cool, just learned about it, pre-allocating
    // using String::with_capacity(s.len()). Which is kind of like bytearray(len)
    // in Python I think, except for strings. This avoid repeated reallocation as
    // we push characters, which might start to matter if we're parsing
    // a gigantic chunk of JSON.
    //
    for c in s.chars() {
        match c {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c => escaped.push(c),
        }
    }
    escaped
}

impl fmt::Display for JsonValue {
    //
    // This is Rust's __str__. By implementing it, we get .to_string() and
    // format!("{}", value) for free, similar to Python's str(obj) calling __str__.
    //
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        //
        // This is a number formatting trick; JSON doesn't distinguish int
        // from float, but 42.0.to_string() gives "42" while 3.14.to_string() gives "3.14".
        // We can check n.fract() == 0.0 to decide whether to cast to i64 and print without
        // decimal if there's no fractional part. Similar to what json.dumps does in Python.
        //
        match self {
            JsonValue::Null => write!(f, "null"),
            JsonValue::Boolean(b) => write!(f, "{}", b),
            JsonValue::Number(n) => {
                if n.fract() == 0.0 {
                    write!(f, "{}", *n as i64)
                } else {
                    write!(f, "{}", n)
                }
            }
            JsonValue::Text(s) => {
                write!(f, "\"{}\"", escape_json_string(s))
            }
            JsonValue::Array(arr) => {
                //
                //  arr.iter()  -> borrows each element (&JsonValue); a little faster than
                //    iterating over a Python list, lol
                //
                //  .enumerate()  -> pairs each element with its index (i, &val).
                //    I think python equivalent might be:
                //        if isinstance(self.value, list):
                //            return "[" + ",".join(str(v)) for v in self.value) + "]"
                //
                write!(f, "[")?;
                for (i, val) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", val)?;
                }
                write!(f, "]")
            }
            JsonValue::Object(obj) => {
                write!(f, "{{")?;
                for (i, (key, val)) in obj.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "\"{}\":{}", escape_json_string(key), val)?;
                }
                write!(f, "}}")
            }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
#[cfg(test)]
mod display_tests {
    use super::*;

    #[test]
    fn test_display_primitives() {
        assert_eq!(JsonValue::Null.to_string(), "null");
        assert_eq!(JsonValue::Boolean(true).to_string(), "true");
        assert_eq!(JsonValue::Boolean(false).to_string(), "false");
        assert_eq!(JsonValue::Number(42.0).to_string(), "42");
        assert_eq!(JsonValue::Number(3.14).to_string(), "3.14");
        assert_eq!(
            JsonValue::Text("hello".to_string()).to_string(),
            "\"hello\""
        );
    }

    #[test]
    fn test_display_array() {
        let value = JsonValue::Array(vec![JsonValue::Number(1.0), JsonValue::Number(2.0)]);
        assert_eq!(value.to_string(), "[1,2]");
    }

    #[test]
    fn test_display_empty_containers() {
        assert_eq!(JsonValue::Array(vec![]).to_string(), "[]");
        assert_eq!(JsonValue::Object(HashMap::new()).to_string(), "{}");
    }

    #[test]
    fn test_display_escape_string() {
        let value = JsonValue::Text("hello\nworld".to_string());
        assert_eq!(value.to_string(), "\"hello\\nworld\"");
    }

    #[test]
    fn test_display_escape_quotes() {
        let value = JsonValue::Text("say \"hi\"".to_string());
        assert_eq!(value.to_string(), "\"say \\\"hi\\\"\"");
    }

    #[test]
    #[ignore]
    fn test_display_nested() {
        let value = JsonParser::new(r#"{"arr": [1, 2]}"#)
            .unwrap()
            .parse()
            .unwrap();
        let output = value.to_string();
        assert!(output.contains("\"arr\""));
        assert!(output.contains("[1,2]"));
    }

    #[test]
    fn test_display_nested_array() {
        let value = JsonValue::Array(vec![JsonValue::Array(vec![
            JsonValue::Number(1.0),
            JsonValue::Number(2.0),
        ])]);
        assert_eq!(value.to_string(), "[[1,2]]");
    }
}
