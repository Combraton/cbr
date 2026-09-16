//! Why a byte sequence is not a value in the domain.
//!
//! Each variant names one rule so a caller can report a specific reason rather
//! than "malformed". The protocol layer maps these onto `invalid_envelope`
//! with a `reason`, so they are part of an observable contract, not debug text.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    /// Byte offset into the input where the problem was detected.
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// Not valid UTF-8. Covers overlong forms and UTF-8-encoded surrogates,
    /// both of which are invalid UTF-8 rather than merely unwanted.
    InvalidUtf8,
    /// A byte-order mark. Valid UTF-8, but senders must not write one.
    ByteOrderMark,
    /// Structurally not JSON, including unknown tokens such as `NaN`.
    NotJson,
    /// Input continued after the top-level value ended.
    TrailingGarbage,
    /// The same member name twice in one object (rule 1).
    DuplicateMember,
    /// A number with a fraction or an exponent (rule 3).
    NonIntegerNumber,
    /// `-0`, which rule 3 excludes.
    NegativeZero,
    /// A leading zero, which JSON itself forbids.
    LeadingZero,
    /// An integer outside +/- (2^53 - 1) (rule 3).
    IntegerOutOfRange,
    /// A surrogate that is not part of a valid pair (rule 2).
    UnpairedSurrogate,
    /// A noncharacter code point (rule 2).
    Noncharacter,
    /// An unescaped control character inside a string.
    ControlCharacterInString,
    /// A `\` escape JSON does not define.
    InvalidEscape,
    /// Input ended in the middle of a value.
    UnexpectedEnd,
    /// Nesting deeper than the configured limit.
    DepthExceeded,
}

impl ErrorKind {
    /// A short, stable reason string suitable for an error `details.reason`.
    pub fn reason(&self) -> &'static str {
        match self {
            ErrorKind::InvalidUtf8 => "invalid UTF-8",
            ErrorKind::ByteOrderMark => "byte-order mark",
            ErrorKind::NotJson => "not JSON",
            ErrorKind::TrailingGarbage => "trailing garbage",
            ErrorKind::DuplicateMember => "duplicate member name",
            ErrorKind::NonIntegerNumber => "non-integer number",
            ErrorKind::NegativeZero => "negative zero",
            ErrorKind::LeadingZero => "leading zero",
            ErrorKind::IntegerOutOfRange => "integer outside safe range",
            ErrorKind::UnpairedSurrogate => "unpaired surrogate",
            ErrorKind::Noncharacter => "noncharacter",
            ErrorKind::ControlCharacterInString => "control character in string",
            ErrorKind::InvalidEscape => "invalid escape",
            ErrorKind::UnexpectedEnd => "unexpected end of input",
            ErrorKind::DepthExceeded => "nesting too deep",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", self.kind.reason(), self.offset)
    }
}

impl std::error::Error for Error {}
