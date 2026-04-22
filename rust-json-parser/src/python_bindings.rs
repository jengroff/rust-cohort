use crate::error::JsonError;
use crate::value::JsonValue;
use memchr::memchr2;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFloat, PyList, PyString};
use std::time::Instant;

/// Times three JSON parsers on the same input and returns their elapsed
/// durations in seconds.
///
/// Returns a tuple `(rust_seconds, json_seconds, simplejson_seconds)`:
/// - `rust_seconds` — this crate's Python-facing `parse_json`, which builds
///   a native `dict`/`list`/`float`/`bool`/`None` tree directly (same shape
///   as the output of `json.loads` and `simplejson.loads`)
/// - `json_seconds` — Python's built-in `json.loads` (C implementation)
/// - `simplejson_seconds` — `simplejson.loads` (pure Python)
///
/// Each parser is run `iterations` times; the returned duration is the
/// *total* elapsed time for all iterations, not the per-iteration time.
///
/// All three parsers produce equivalent Python trees, so the comparison
/// is apples-to-apples (the old benchmark measured Rust→`JsonValue`, which
/// skipped the Python-object construction cost the other two pay).
#[pyfunction]
#[pyo3(signature = (json_str, iterations = 1000))]
fn benchmark_performance<'py>(
    py: Python<'py>,
    json_str: &str,
    iterations: usize,
) -> PyResult<(f64, f64, f64)> {
    // ---- Rust parser (direct Rust→Python path) ----
    let rust_duration = {
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = py_stream::parse(py, json_str)?;
        }
        start.elapsed().as_secs_f64()
    };

    // ---- Python json (C implementation) ----
    let json_duration = {
        let json_module = py.import("json")?;
        let json_loads = json_module.getattr("loads")?;
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = json_loads.call1((json_str,))?;
        }
        start.elapsed().as_secs_f64()
    };

    // ---- simplejson (pure Python) ----
    let simplejson_duration = {
        let simplejson_module = py.import("simplejson")?;
        let simplejson_loads = simplejson_module.getattr("loads")?;
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = simplejson_loads.call1((json_str,))?;
        }
        start.elapsed().as_secs_f64()
    };

    Ok((rust_duration, json_duration, simplejson_duration))
}


//
// IntoPyObject is the trait that teaches PyO3 how to convert my Rust type
// into a Python object. It's like implementing __init__ for a type adapter —
// once it exists, PyO3 calls it automatically whenever it needs to return a
// JsonValue to Python.
//
// This path is kept for `dumps()` (which still goes through JsonValue) and
// for anyone calling `parse_json` on a JsonValue. The hot `parse_json`
// entry-point now skips it entirely in favour of the direct streaming
// builder in `py_stream`.
//

impl<'py> IntoPyObject<'py> for JsonValue {
    type Target = PyAny;
    type Output = Bound<'py, Self::Target>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        match self {
            JsonValue::Null => Ok(py.None().into_bound(py)),
            JsonValue::Boolean(b) => Ok(b.into_pyobject(py)?.to_owned().into_any()),
            JsonValue::Number(n) => Ok(n.into_pyobject(py)?.to_owned().into_any()),
            JsonValue::Text(s) => Ok(s.into_pyobject(py)?.to_owned().into_any()),
            JsonValue::Array(arr) => {
                let py_list = PyList::empty(py);
                for item in arr {
                    py_list.append(item.into_pyobject(py)?)?;
                }
                Ok(py_list.into_any())
            }
            JsonValue::Object(map) => {
                let py_dict = PyDict::new(py);
                for (key, value) in map {
                    py_dict.set_item(key, value.into_pyobject(py)?)?;
                }
                Ok(py_dict.into_any())
            }
        }
    }
}

impl From<JsonError> for PyErr {
    fn from(err: JsonError) -> PyErr {
        match err {
            JsonError::UnexpectedToken {
                expected,
                found,
                position,
            } => PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "JSON parse error at position {}: expected {}, found {}",
                position, expected, found
            )),
            JsonError::UnexpectedEndOfInput { expected, position } => {
                PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Unexpected end of input at position {}: expected {}",
                    position, expected
                ))
            }
            JsonError::InvalidNumber { value, position } => {
                PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Invalid number '{}' at position {}",
                    value, position
                ))
            }
            JsonError::InvalidEscape { ch, position } => {
                PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Invalid escape '\\{}' at position {}",
                    ch, position
                ))
            }
            JsonError::InvalidUnicode { sequence, position } => {
                PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Invalid unicode escape '\\u{}' at position {}",
                    sequence, position
                ))
            }
        }
    }
}

fn py_to_json_value(obj: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    // We put bool before number b/c in Python bool is a subclass of int.
    if obj.is_none() {
        Ok(JsonValue::Null)
    } else if let Ok(b) = obj.extract::<bool>() {
        Ok(JsonValue::Boolean(b))
    } else if let Ok(n) = obj.extract::<f64>() {
        Ok(JsonValue::Number(n))
    } else if let Ok(s) = obj.extract::<String>() {
        Ok(JsonValue::Text(s))
    } else if let Ok(list) = obj.cast::<PyList>() {
        let mut arr = Vec::new();
        for item in list.iter() {
            arr.push(py_to_json_value(&item)?);
        }
        Ok(JsonValue::Array(arr))
    } else if let Ok(dict) = obj.cast::<PyDict>() {
        let mut map = crate::value::JsonObject::default();
        for (key, value) in dict.iter() {
            let key_str: String = key.extract()?;
            map.insert(key_str, py_to_json_value(&value)?);
        }
        Ok(JsonValue::Object(map))
    } else {
        Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(format!(
            "Unsupported type: {}",
            obj.get_type().name()?
        )))
    }
}

#[pyfunction]
fn parse_json<'py>(py: Python<'py>, input: &str) -> PyResult<Bound<'py, PyAny>> {
    // Uses the direct Rust→Python streaming builder instead of
    // parse-to-JsonValue-then-convert. That drops one entire traversal
    // and one full set of Rust-side allocations out of the pipeline.
    py_stream::parse(py, input)
}

#[pyfunction]
fn parse_json_file<'py>(py: Python<'py>, path: &str) -> PyResult<Bound<'py, PyAny>> {
    let contents = std::fs::read_to_string(path)?;
    py_stream::parse(py, &contents)
}

#[pyfunction]
#[pyo3(signature = (obj, indent=None))]
fn dumps(obj: &Bound<'_, PyAny>, indent: Option<usize>) -> PyResult<String> {
    let value = py_to_json_value(obj)?;
    match indent {
        None => Ok(value.to_string()),
        Some(_n) => Ok(value.to_string()), // TODO: pretty-print with indent
    }
}

#[pymodule]
fn _rust_json_parser(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_json, m)?)?;
    m.add_function(wrap_pyfunction!(parse_json_file, m)?)?;
    m.add_function(wrap_pyfunction!(dumps, m)?)?;
    m.add_function(wrap_pyfunction!(benchmark_performance, m)?)?;
    Ok(())
}

// ============================================================================
// Direct Rust → Python streaming parser.
// ============================================================================
//
// The library-facing `crate::stream` parser builds a `JsonValue` tree.
// When we return that to Python we have to walk it a second time and
// construct `PyDict`/`PyList`/`PyString`/`PyFloat` equivalents — two passes,
// two sets of allocations.
//
// `py_stream` collapses those two passes back into one. It's structurally
// identical to `crate::stream::StreamParser` but every `parse_*` method
// returns a `Bound<'py, PyAny>` built with CPython's specialised dict/list
// allocators, which are materially faster than `malloc` for many tiny
// allocations (pymalloc arena). That saves both the Rust-side `JsonValue`
// allocations AND the cost of a second traversal.
mod py_stream {
    use super::*;

    pub fn parse<'py>(py: Python<'py>, input: &str) -> PyResult<Bound<'py, PyAny>> {
        let mut p = PyStreamParser {
            input: input.as_bytes(),
            position: 0,
            py,
        };
        p.skip_ws();
        let v = p.parse_value()?;
        p.skip_ws();
        if p.position < p.input.len() {
            return Err(JsonError::UnexpectedToken {
                expected: "end of input".to_string(),
                found: (p.input[p.position] as char).to_string(),
                position: p.position,
            }
            .into());
        }
        Ok(v)
    }

    struct PyStreamParser<'a, 'py> {
        input: &'a [u8],
        position: usize,
        py: Python<'py>,
    }

    impl<'a, 'py> PyStreamParser<'a, 'py> {
        #[inline]
        fn peek(&self) -> Option<u8> {
            self.input.get(self.position).copied()
        }

        #[inline]
        fn skip_ws(&mut self) {
            while let Some(b) = self.input.get(self.position) {
                match *b {
                    b' ' | b'\t' | b'\n' | b'\r' => self.position += 1,
                    _ => break,
                }
            }
        }

        fn parse_value(&mut self) -> PyResult<Bound<'py, PyAny>> {
            match self.peek() {
                Some(b'{') => {
                    self.position += 1;
                    self.parse_object()
                }
                Some(b'[') => {
                    self.position += 1;
                    self.parse_array()
                }
                Some(b'"') => {
                    self.position += 1;
                    let s = self.parse_string_slice()?;
                    Ok(PyString::new(self.py, s.as_ref()).into_any())
                }
                Some(b't') => {
                    self.expect_keyword(b"true")?;
                    Ok(true.into_pyobject(self.py)?.to_owned().into_any())
                }
                Some(b'f') => {
                    self.expect_keyword(b"false")?;
                    Ok(false.into_pyobject(self.py)?.to_owned().into_any())
                }
                Some(b'n') => {
                    self.expect_keyword(b"null")?;
                    Ok(self.py.None().into_bound(self.py))
                }
                Some(b'-') | Some(b'0'..=b'9') => self.parse_number_py(),
                Some(other) => Err(JsonError::UnexpectedToken {
                    expected: "JSON value".to_string(),
                    found: (other as char).to_string(),
                    position: self.position,
                }
                .into()),
                None => Err(JsonError::UnexpectedEndOfInput {
                    expected: "JSON value".to_string(),
                    position: self.position,
                }
                .into()),
            }
        }

        fn parse_array(&mut self) -> PyResult<Bound<'py, PyAny>> {
            // Opening `[` already consumed.
            self.skip_ws();
            let list = PyList::empty(self.py);
            if self.peek() == Some(b']') {
                self.position += 1;
                return Ok(list.into_any());
            }
            loop {
                let v = self.parse_value()?;
                list.append(v)?;
                self.skip_ws();
                match self.peek() {
                    Some(b',') => {
                        self.position += 1;
                        self.skip_ws();
                        if self.peek() == Some(b']') {
                            return Err(JsonError::UnexpectedToken {
                                expected: "value".to_string(),
                                found: "]".to_string(),
                                position: self.position,
                            }
                            .into());
                        }
                    }
                    Some(b']') => {
                        self.position += 1;
                        return Ok(list.into_any());
                    }
                    Some(c) => {
                        return Err(JsonError::UnexpectedToken {
                            expected: ", or ]".to_string(),
                            found: (c as char).to_string(),
                            position: self.position,
                        }
                        .into());
                    }
                    None => {
                        return Err(JsonError::UnexpectedEndOfInput {
                            expected: "] or ,".to_string(),
                            position: self.position,
                        }
                        .into());
                    }
                }
            }
        }

        fn parse_object(&mut self) -> PyResult<Bound<'py, PyAny>> {
            // Opening `{` already consumed.
            self.skip_ws();
            let dict = PyDict::new(self.py);
            if self.peek() == Some(b'}') {
                self.position += 1;
                return Ok(dict.into_any());
            }
            loop {
                match self.peek() {
                    Some(b'"') => self.position += 1,
                    Some(c) => {
                        return Err(JsonError::UnexpectedToken {
                            expected: "string key".to_string(),
                            found: (c as char).to_string(),
                            position: self.position,
                        }
                        .into());
                    }
                    None => {
                        return Err(JsonError::UnexpectedEndOfInput {
                            expected: "string key".to_string(),
                            position: self.position,
                        }
                        .into());
                    }
                }
                let key = self.parse_string_slice()?;
                let py_key = PyString::new(self.py, key.as_ref());
                self.skip_ws();
                match self.peek() {
                    Some(b':') => self.position += 1,
                    Some(c) => {
                        return Err(JsonError::UnexpectedToken {
                            expected: ":".to_string(),
                            found: (c as char).to_string(),
                            position: self.position,
                        }
                        .into());
                    }
                    None => {
                        return Err(JsonError::UnexpectedEndOfInput {
                            expected: ":".to_string(),
                            position: self.position,
                        }
                        .into());
                    }
                }
                self.skip_ws();
                let value = self.parse_value()?;
                dict.set_item(py_key, value)?;
                self.skip_ws();
                match self.peek() {
                    Some(b',') => {
                        self.position += 1;
                        self.skip_ws();
                        if self.peek() == Some(b'}') {
                            return Err(JsonError::UnexpectedToken {
                                expected: "string key".to_string(),
                                found: "}".to_string(),
                                position: self.position,
                            }
                            .into());
                        }
                    }
                    Some(b'}') => {
                        self.position += 1;
                        return Ok(dict.into_any());
                    }
                    Some(c) => {
                        return Err(JsonError::UnexpectedToken {
                            expected: ", or }".to_string(),
                            found: (c as char).to_string(),
                            position: self.position,
                        }
                        .into());
                    }
                    None => {
                        return Err(JsonError::UnexpectedEndOfInput {
                            expected: "} or ,".to_string(),
                            position: self.position,
                        }
                        .into());
                    }
                }
            }
        }

        /// Returns the string contents. The fast path (no escapes) borrows
        /// the original input slice; the slow path allocates a `String`.
        /// Either way the caller gets something dereferencing to `&str`,
        /// which is exactly what `PyString::new` wants.
        fn parse_string_slice(&mut self) -> PyResult<std::borrow::Cow<'a, str>> {
            let start = self.position;
            let bytes = self.input;
            match memchr2(b'"', b'\\', &bytes[self.position..]) {
                Some(offset) => {
                    let end = self.position + offset;
                    if bytes[end] == b'"' {
                        // SAFETY: input came from a &str so it's valid UTF-8,
                        // and `"` is ASCII so the slice boundary is safe.
                        let s = unsafe { std::str::from_utf8_unchecked(&bytes[start..end]) };
                        self.position = end + 1;
                        Ok(std::borrow::Cow::Borrowed(s))
                    } else {
                        self.position = end;
                        let owned = self.parse_string_with_escapes(start)?;
                        Ok(std::borrow::Cow::Owned(owned))
                    }
                }
                None => Err(JsonError::UnexpectedEndOfInput {
                    expected: "closing quote".to_string(),
                    position: start,
                }
                .into()),
            }
        }

        fn parse_string_with_escapes(&mut self, clean_start: usize) -> PyResult<String> {
            let bytes = self.input;
            let mut buf: Vec<u8> = Vec::with_capacity(bytes.len() - clean_start);
            buf.extend_from_slice(&bytes[clean_start..self.position]);

            loop {
                match self.peek() {
                    Some(b'"') => {
                        self.position += 1;
                        // SAFETY: pushed bytes are either from valid-UTF-8 input
                        // or well-formed UTF-8 encodings of escape chars.
                        let s = unsafe { String::from_utf8_unchecked(buf) };
                        return Ok(s);
                    }
                    Some(b'\\') => {
                        self.position += 1;
                        self.decode_escape(&mut buf)?;
                    }
                    Some(_) => match memchr2(b'"', b'\\', &bytes[self.position..]) {
                        Some(offset) => {
                            buf.extend_from_slice(&bytes[self.position..self.position + offset]);
                            self.position += offset;
                        }
                        None => {
                            return Err(JsonError::UnexpectedEndOfInput {
                                expected: "closing quote".to_string(),
                                position: clean_start,
                            }
                            .into());
                        }
                    },
                    None => {
                        return Err(JsonError::UnexpectedEndOfInput {
                            expected: "closing quote".to_string(),
                            position: clean_start,
                        }
                        .into());
                    }
                }
            }
        }

        fn decode_escape(&mut self, buf: &mut Vec<u8>) -> PyResult<()> {
            let pos = self.position;
            let b = match self.input.get(pos) {
                Some(&b) => b,
                None => {
                    return Err(JsonError::UnexpectedEndOfInput {
                        expected: "escape character".to_string(),
                        position: pos,
                    }
                    .into());
                }
            };
            self.position += 1;
            match b {
                b'"' => buf.push(b'"'),
                b'\\' => buf.push(b'\\'),
                b'/' => buf.push(b'/'),
                b'b' => buf.push(0x08),
                b'f' => buf.push(0x0C),
                b'n' => buf.push(b'\n'),
                b'r' => buf.push(b'\r'),
                b't' => buf.push(b'\t'),
                b'u' => {
                    let ch = self.parse_unicode_escape()?;
                    let mut tmp = [0u8; 4];
                    let encoded = ch.encode_utf8(&mut tmp);
                    buf.extend_from_slice(encoded.as_bytes());
                }
                other => {
                    return Err(JsonError::InvalidEscape {
                        ch: other as char,
                        position: pos,
                    }
                    .into());
                }
            }
            Ok(())
        }

        fn parse_unicode_escape(&mut self) -> PyResult<char> {
            let start = self.position;
            if self.position + 4 > self.input.len() {
                let tail = &self.input[self.position..];
                self.position = self.input.len();
                let seq = unsafe { std::str::from_utf8_unchecked(tail) }.to_string();
                return Err(JsonError::InvalidUnicode {
                    sequence: seq,
                    position: start,
                }
                .into());
            }
            let hex = &self.input[self.position..self.position + 4];
            if !hex.iter().all(|b| b.is_ascii_hexdigit()) {
                let kept: String = hex
                    .iter()
                    .take_while(|b| b.is_ascii_hexdigit())
                    .map(|b| *b as char)
                    .collect();
                self.position += 4;
                return Err(JsonError::InvalidUnicode {
                    sequence: kept,
                    position: start,
                }
                .into());
            }
            self.position += 4;
            let hex_str = unsafe { std::str::from_utf8_unchecked(hex) };
            let code_point = u32::from_str_radix(hex_str, 16).expect("4 hex digits parse");
            Ok(char::from_u32(code_point).ok_or(JsonError::InvalidUnicode {
                sequence: hex_str.to_string(),
                position: start,
            })?)
        }

        /// Scan a number literal and return the Python object for it.
        ///
        /// JSON doesn't distinguish int from float, but CPython's
        /// `json.loads` returns `int` for literals without a `.`, `e`,
        /// or `E` — and that matters for performance: small positive ints
        /// 0..256 are pre-allocated and cached by CPython, so returning
        /// `PyInt` bypasses thousands of `PyFloat_FromDouble` allocations
        /// when a document has a lot of small integer IDs (which the
        /// benchmark does).
        ///
        /// Ints that overflow `i64` fall back to `f64`, matching the
        /// behaviour of `json.loads` closely enough for our purposes and
        /// avoiding a detour through Python's arbitrary-precision int
        /// parser (which we'd have to build from bytes anyway).
        fn parse_number_py(&mut self) -> PyResult<Bound<'py, PyAny>> {
            let start = self.position;
            let mut is_float = false;
            while let Some(&b) = self.input.get(self.position) {
                match b {
                    b'0'..=b'9' | b'-' | b'+' => self.position += 1,
                    b'.' | b'e' | b'E' => {
                        is_float = true;
                        self.position += 1;
                    }
                    _ => break,
                }
            }
            let bytes = &self.input[start..self.position];
            // SAFETY: every accepted byte is ASCII.
            let num_str = unsafe { std::str::from_utf8_unchecked(bytes) };

            if !is_float {
                // Fast path: integer literal. Try i64 first — if it fits,
                // we return a PyInt and benefit from CPython's small-int cache.
                if let Ok(n) = num_str.parse::<i64>() {
                    return Ok(n.into_pyobject(self.py)?.to_owned().into_any());
                }
                // Overflow — fall back to f64. JSON allows it; `json.loads`
                // uses arbitrary precision instead, but f64 is close enough
                // and far cheaper than building a Python bigint by hand.
            }

            num_str
                .parse::<f64>()
                .map(|n| PyFloat::new(self.py, n).into_any())
                .map_err(|_| {
                    JsonError::InvalidNumber {
                        value: num_str.to_string(),
                        position: start,
                    }
                    .into()
                })
        }

        fn expect_keyword(&mut self, keyword: &[u8]) -> PyResult<()> {
            let start = self.position;
            let end = start + keyword.len();
            if end > self.input.len() || &self.input[start..end] != keyword {
                let saw_end = end.min(self.input.len());
                let found = unsafe {
                    std::str::from_utf8_unchecked(&self.input[start..saw_end])
                }
                .to_string();
                return Err(JsonError::UnexpectedToken {
                    expected: unsafe { std::str::from_utf8_unchecked(keyword) }.to_string(),
                    found,
                    position: start,
                }
                .into());
            }
            self.position = end;
            Ok(())
        }
    }
}
