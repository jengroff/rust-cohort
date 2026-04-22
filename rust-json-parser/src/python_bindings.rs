use crate::{JsonError, JsonValue};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::time::Instant;

/// Times three JSON parsers on the same input and returns their elapsed
/// durations in seconds.
///
/// Returns a tuple `(rust_seconds, json_seconds, simplejson_seconds)`:
/// - `rust_seconds` — this crate's [`parse_json`](crate::parse_json)
/// - `json_seconds` — Python's built-in `json.loads` (C implementation)
/// - `simplejson_seconds` — `simplejson.loads` (pure Python)
///
/// Each parser is run `iterations` times; the returned duration is the
/// *total* elapsed time for all iterations, not the per-iteration time.
#[pyfunction]
#[pyo3(signature = (json_str, iterations = 1000))]
fn benchmark_performance<'py>(
    py: Python<'py>,
    json_str: &str,
    iterations: usize,
) -> PyResult<(f64, f64, f64)> {
    // ---- Rust parser ----
    let rust_duration = {
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = crate::parser::parse_json(json_str)?;
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
// JsonValue to Python. Slick.
//

impl<'py> IntoPyObject<'py> for JsonValue {
    type Target = PyAny;
    //
    // we're creating "any Python object" (could be dict, list
    // str, etc depending on the variant)
    //
    type Output = Bound<'py, Self::Target>;
    //
    // the result is a GIL-bound reference;
    // proof that we hold the GIL; this part hurts my head
    // We can't use a Bound<'py, ...> after releasing the GIL, because
    // the GIL must be held to touch Python objects. Which is strange
    // phrasing, admittedly.
    //
    type Error = PyErr;
    //
    // conversion can fail with a Python exception
    //
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
            } //
              // For collections (Array, Object), the recursion is in the loop.
              // Each element/value calls .into_pyobject(py)? which hits this
              // same impl. In the event of nesting, e.g. if an array contains an
              // object which contains an array, the conversion recurses naturally.
              // Whoever invented this shit is a genius.
        }
    }
}

impl From<JsonError> for PyErr {
    //
    // This is the bridge between Rust erros and Python exceptions. Once this
    // exists, the ? operator can automatically convert JsonError -> PyErr
    // in any function returning PyResult<T>.
    //
    // The From trait is Rust's standard conversion trait. It's kinda like
    // writing a Python __init__ that accepts another type:
    //   class PyErr:
    //       def __init__(self, err: JsonError): ...
    //
    fn from(err: JsonError) -> PyErr {
        match err {
            JsonError::UnexpectedToken {
                expected,
                found,
                position,
            } => PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                //
                // PyErr::new::<ExceptionType, _>(message) creates a Python
                // exception. The turbofish (one of the best names for
                // a piece of syntax ever) ::<ExceptionType, _> tells Rust which
                // exception class to use. It's like Python's raise ValueError(message).
                //
                "JSON parse error at position {}: expected {}, found {}",
                position, expected, found
            )),
            JsonError::UnexpectedEndOfInput { expected, position } => {
                //
                // the position information in error messages is important b/c it helps
                // Python users debug malformed JSON without needing to understand Rust
                // internals, heaven forbid.
                //
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
    //
    // This is the reverse of IntoPyObject; it takes a Python object
    // (which could be anything) and figures out what JsonValue variant
    // it should become.
    //
    // We put bool before number b/c in Python bool is a subclass of int.
    // True == 1 and False == 0. If you check number first, Python's True
    // will extract as 1.0 and you'll get JsonValue::Number(1.0) instead of
    // JsonValue::Boolean(true), which strikes me as kind of hilarious
    // for some reason. Apparently this is a deliberate design choice
    // going back to PEP 285 (2002), chosen for backward compatibility, b/c
    // before bool existed, Python code used 0 and 1 everywhere.
    //
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
        let mut map = std::collections::HashMap::new();
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
//
// #[pyfunction] is a magic attribute which tells PyO3 to generate
// wrapper code that makes this Rust function callable from Python.
//
// py: Python<'py> is the GIL token; PyO3 acquires the GIL and passes this
// automatically. We introduce a named 'py lifetime b/c the return type
// Bound<'py, PyAny> needs to be tied to the same GIL scope as `py`.
//
fn parse_json<'py>(py: Python<'py>, input: &str) -> PyResult<Bound<'py, PyAny>> {
    let result = crate::parser::JsonParser::new(input)?.parse()?;
    result.into_pyobject(py)
}

#[pyfunction]
fn parse_json_file<'py>(py: Python<'py>, path: &str) -> PyResult<Bound<'py, PyAny>> {
    let contents = std::fs::read_to_string(path)?;
    let result = crate::parser::JsonParser::new(&contents)?.parse()?;
    result.into_pyobject(py)
}

#[pyfunction]
#[pyo3(signature = (obj, indent=None))]
//
// this tells PyO3 to generate a Python function signature with an optional
// keyword argument. From Python, you can call: dumps(data) or dumps(data, indent=2)
//
fn dumps(obj: &Bound<'_, PyAny>, indent: Option<usize>) -> PyResult<String> {
    let value = py_to_json_value(obj)?;
    //
    // Current status: pretty_print() isn't implemented on JsonValue yet, so
    // the Some(_n) arm falls back to the same compact to_string() as None.
    // Python callers using dumps(obj, indent=2) won't crash — they'll just
    // get compact output until pretty_print lands in value.rs.
    //
    match indent {
        None => Ok(value.to_string()),
        Some(_n) => Ok(value.to_string()), // TODO: pretty-print with indent
    }
}

#[pymodule]
//
// #[pymodule] defines the entry point Python calls when we do
// `import rust_json_parser._rust_json_parser`.
//
// This is like writing a Python module's __init__.py that says:
//   from ._implementation import parse_json, parse_json_file, dumps
//
// wrap_pyfunction!(parse_json, m)? — wraps our Rust function and adds
// it to the module. The m parameter is the module being constructed.
//
// Each m.add_function() can fail (returns PyResult), so we chain ? to
// propagate errors. The final Ok(()) tells Python the module initialized
// successfully.
//
// The function name `_rust_json_parser` MUST match the module-name in
// pyproject.toml (the part after the last dot), which I found out the
// hard way. If these don't match, Python will (vomit) fail with
// "ImportError: dynamic module does not define module export function."
//
fn _rust_json_parser(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_json, m)?)?;
    m.add_function(wrap_pyfunction!(parse_json_file, m)?)?;
    m.add_function(wrap_pyfunction!(dumps, m)?)?;
    m.add_function(wrap_pyfunction!(benchmark_performance, m)?)?; // NEW
    Ok(())
}
