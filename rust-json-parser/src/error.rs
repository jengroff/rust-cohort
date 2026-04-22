use std::fmt;

/// Errors that can occur while tokenizing or parsing JSON.
///
/// Every variant carries a `position` (byte offset into the input) so the
/// caller can report where things went wrong. Format the error with
/// `Display` for a human-readable message.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonError {
    /// A token was found where a different one was expected.
    ///
    /// Produced by the parser when the grammar demands, say, a `,` or `]`
    /// and something else is there.
    UnexpectedToken {
        /// Human-readable description of what the parser wanted.
        expected: String,
        /// Debug-formatted view of what it actually found.
        found: String,
        /// Byte offset into the input.
        position: usize,
    },
    /// Input ended while the parser was still expecting more tokens.
    ///
    /// Typical cause: a missing closing `]` or `}`.
    UnexpectedEndOfInput {
        /// Description of what would have completed the input.
        expected: String,
        /// Position where input ended.
        position: usize,
    },
    /// A numeric literal couldn't be parsed as an `f64`.
    ///
    /// Typical cause: malformed scientific notation or a leading zero like
    /// `007` that JSON doesn't allow.
    InvalidNumber {
        /// The raw text that failed to parse.
        value: String,
        /// Position of the offending number.
        position: usize,
    },
    /// An unknown `\X` escape sequence inside a string literal.
    InvalidEscape {
        /// The character that followed the backslash.
        ch: char,
        /// Position of the backslash.
        position: usize,
    },
    /// A `\uXXXX` escape where `XXXX` wasn't four valid hex digits.
    InvalidUnicode {
        /// The 4-character sequence that failed.
        sequence: String,
        /// Position of the `\u`.
        position: usize,
    },
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TESTS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonError::UnexpectedToken {
                expected,
                found,
                position,
            } => {
                write!(
                    f,
                    "Unexpected token at position {}: expected {}, found {}",
                    position, expected, found
                )
            }
            JsonError::UnexpectedEndOfInput { expected, position } => {
                write!(
                    f,
                    "Unexpected end of input at position {}: expected {}",
                    position, expected
                )
            }
            JsonError::InvalidNumber { value, position } => {
                write!(f, "Invalid number '{}' at position {}", value, position)
            }
            JsonError::InvalidEscape { ch, position } => {
                write!(
                    f,
                    "Invalid escape sequence '\\{}' at position {}",
                    ch, position
                )
            }
            JsonError::InvalidUnicode { sequence, position } => {
                write!(
                    f,
                    "Invalid Unicode '\\u{}' at position {}",
                    sequence, position
                )
            }
        }
    }
}

impl std::error::Error for JsonError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_escape_display() {
        let err = JsonError::InvalidEscape {
            ch: 'q',
            position: 5,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("escape"));
        assert!(msg.contains("q"));
    }

    #[test]
    fn test_invalid_unicode_display() {
        let err = JsonError::InvalidUnicode {
            sequence: "00GG".to_string(),
            position: 3,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("unicode") || msg.contains("Unicode"));
    }

    #[test]
    fn test_error_is_std_error() {
        let err = JsonError::InvalidEscape {
            ch: 'x',
            position: 0,
        };
        let _: &dyn std::error::Error = &err; // Must implement Error trait
    }
}
