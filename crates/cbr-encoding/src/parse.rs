//! A strict reader for the domain of ENCODING section 1.
//!
//! Written by hand rather than layered on a general JSON parser, because the
//! rules that matter here are exactly the ones a permissive parser discards:
//! duplicate members silently collapse, `1.0` and `1` compare equal, and large
//! integers round. Any of those would let two different commands produce one
//! digest.

use std::collections::HashSet;

use crate::error::{Error, ErrorKind};
use crate::value::{MAX_SAFE_INTEGER, Value, is_noncharacter};

/// Default nesting limit. Core section 9 lets a profile set a tighter one;
/// this bound only stops a hostile input from exhausting the stack.
pub const DEFAULT_MAX_DEPTH: usize = 128;

/// Parse one JSON text into the domain, rejecting anything outside it.
pub fn parse(input: &[u8]) -> Result<Value, Error> {
    parse_with_depth(input, DEFAULT_MAX_DEPTH)
}

/// Parse with an explicit nesting limit.
pub fn parse_with_depth(input: &[u8], max_depth: usize) -> Result<Value, Error> {
    // Reject invalid UTF-8 before anything else. This is also what rejects
    // overlong forms and surrogates encoded as UTF-8, which are not merely
    // unwanted here but invalid UTF-8 outright.
    let text = std::str::from_utf8(input).map_err(|e| Error {
        kind: ErrorKind::InvalidUtf8,
        offset: e.valid_up_to(),
    })?;
    if text.starts_with('\u{FEFF}') {
        return Err(Error {
            kind: ErrorKind::ByteOrderMark,
            offset: 0,
        });
    }
    let mut parser = Parser {
        bytes: text.as_bytes(),
        text,
        at: 0,
        max_depth,
    };
    parser.skip_whitespace();
    let value = parser.value(0)?;
    parser.skip_whitespace();
    if parser.at != parser.bytes.len() {
        return Err(parser.err(ErrorKind::TrailingGarbage));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    text: &'a str,
    at: usize,
    max_depth: usize,
}

impl<'a> Parser<'a> {
    fn err(&self, kind: ErrorKind) -> Error {
        Error {
            kind,
            offset: self.at,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    /// RFC 8259 insignificant whitespace, and only that.
    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), Error> {
        match self.peek() {
            Some(found) if found == byte => {
                self.at += 1;
                Ok(())
            }
            Some(_) => Err(self.err(ErrorKind::NotJson)),
            None => Err(self.err(ErrorKind::UnexpectedEnd)),
        }
    }

    fn literal(&mut self, word: &str, value: Value) -> Result<Value, Error> {
        if self.text[self.at..].starts_with(word) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(self.err(ErrorKind::NotJson))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, Error> {
        if depth > self.max_depth {
            return Err(self.err(ErrorKind::DepthExceeded));
        }
        match self.peek() {
            None => Err(self.err(ErrorKind::UnexpectedEnd)),
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => Ok(Value::String(self.string()?)),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            // Anything else, including `NaN` and `Infinity`, is not JSON.
            Some(_) => Err(self.err(ErrorKind::NotJson)),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, Error> {
        self.expect(b'{')?;
        let mut members: Vec<(String, Value)> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(Value::Object(members));
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return Err(self.err(ErrorKind::NotJson));
            }
            let name_at = self.at;
            let name = self.string()?;
            if !seen.insert(name.clone()) {
                return Err(Error {
                    kind: ErrorKind::DuplicateMember,
                    offset: name_at,
                });
            }
            self.skip_whitespace();
            self.expect(b':')?;
            self.skip_whitespace();
            let value = self.value(depth + 1)?;
            members.push((name, value));
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(Value::Object(members));
                }
                Some(_) => return Err(self.err(ErrorKind::NotJson)),
                None => return Err(self.err(ErrorKind::UnexpectedEnd)),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, Error> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.skip_whitespace();
            items.push(self.value(depth + 1)?);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(Value::Array(items));
                }
                Some(_) => return Err(self.err(ErrorKind::NotJson)),
                None => return Err(self.err(ErrorKind::UnexpectedEnd)),
            }
        }
    }

    fn number(&mut self) -> Result<Value, Error> {
        let start = self.at;
        let negative = self.peek() == Some(b'-');
        if negative {
            self.at += 1;
        }
        let digits_at = self.at;
        match self.peek() {
            Some(b'0') => {
                self.at += 1;
                // JSON forbids a leading zero before another digit.
                if matches!(self.peek(), Some(b'0'..=b'9')) {
                    return Err(Error {
                        kind: ErrorKind::LeadingZero,
                        offset: digits_at,
                    });
                }
            }
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.at += 1;
                }
            }
            _ => return Err(self.err(ErrorKind::NotJson)),
        }
        // A fraction or exponent makes this a non-integer, which the domain
        // excludes even though JSON allows it.
        if matches!(self.peek(), Some(b'.' | b'e' | b'E')) {
            return Err(Error {
                kind: ErrorKind::NonIntegerNumber,
                offset: start,
            });
        }
        let digits = &self.text[digits_at..self.at];
        if negative && digits == "0" {
            return Err(Error {
                kind: ErrorKind::NegativeZero,
                offset: start,
            });
        }
        // Accumulate in i128 so an oversized literal is reported as out of
        // range rather than overflowing on the way to the check.
        let mut magnitude: i128 = 0;
        for byte in digits.bytes() {
            magnitude = magnitude * 10 + i128::from(byte - b'0');
            if magnitude > i128::from(MAX_SAFE_INTEGER) {
                return Err(Error {
                    kind: ErrorKind::IntegerOutOfRange,
                    offset: start,
                });
            }
        }
        let signed = if negative { -magnitude } else { magnitude };
        Ok(Value::Int(signed as i64))
    }

    fn string(&mut self) -> Result<String, Error> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let byte = self
                .peek()
                .ok_or_else(|| self.err(ErrorKind::UnexpectedEnd))?;
            match byte {
                b'"' => {
                    self.at += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.at += 1;
                    self.escape(&mut out)?;
                }
                // RFC 8259 forbids unescaped control characters in strings.
                0x00..=0x1F => return Err(self.err(ErrorKind::ControlCharacterInString)),
                _ => {
                    let ch = self.text[self.at..]
                        .chars()
                        .next()
                        .ok_or_else(|| self.err(ErrorKind::UnexpectedEnd))?;
                    if is_noncharacter(ch as u32) {
                        return Err(self.err(ErrorKind::Noncharacter));
                    }
                    out.push(ch);
                    self.at += ch.len_utf8();
                }
            }
        }
    }

    fn escape(&mut self, out: &mut String) -> Result<(), Error> {
        let byte = self
            .peek()
            .ok_or_else(|| self.err(ErrorKind::UnexpectedEnd))?;
        self.at += 1;
        let ch = match byte {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'b' => '\u{0008}',
            b'f' => '\u{000C}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => return self.unicode_escape(out),
            _ => {
                return Err(Error {
                    kind: ErrorKind::InvalidEscape,
                    offset: self.at - 1,
                });
            }
        };
        out.push(ch);
        Ok(())
    }

    fn unicode_escape(&mut self, out: &mut String) -> Result<(), Error> {
        let start = self.at - 2;
        let first = self.hex4()?;
        let code_point = if (0xD800..=0xDBFF).contains(&first) {
            // A high surrogate must be followed by a `\u` low surrogate.
            if !self.text[self.at..].starts_with("\\u") {
                return Err(Error {
                    kind: ErrorKind::UnpairedSurrogate,
                    offset: start,
                });
            }
            self.at += 2;
            let second = self.hex4()?;
            if !(0xDC00..=0xDFFF).contains(&second) {
                return Err(Error {
                    kind: ErrorKind::UnpairedSurrogate,
                    offset: start,
                });
            }
            0x10000 + ((u32::from(first) - 0xD800) << 10) + (u32::from(second) - 0xDC00)
        } else if (0xDC00..=0xDFFF).contains(&first) {
            // A low surrogate on its own has no pair to join.
            return Err(Error {
                kind: ErrorKind::UnpairedSurrogate,
                offset: start,
            });
        } else {
            u32::from(first)
        };
        if is_noncharacter(code_point) {
            return Err(Error {
                kind: ErrorKind::Noncharacter,
                offset: start,
            });
        }
        let ch = char::from_u32(code_point).ok_or(Error {
            kind: ErrorKind::UnpairedSurrogate,
            offset: start,
        })?;
        out.push(ch);
        Ok(())
    }

    fn hex4(&mut self) -> Result<u16, Error> {
        let start = self.at;
        if self.at + 4 > self.bytes.len() {
            return Err(self.err(ErrorKind::UnexpectedEnd));
        }
        let mut value: u16 = 0;
        for offset in 0..4 {
            let byte = self.bytes[start + offset];
            let digit = match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                b'A'..=b'F' => byte - b'A' + 10,
                _ => {
                    return Err(Error {
                        kind: ErrorKind::InvalidEscape,
                        offset: start + offset,
                    });
                }
            };
            value = value * 16 + u16::from(digit);
        }
        self.at += 4;
        Ok(value)
    }
}
