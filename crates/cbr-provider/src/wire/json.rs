//! A reader for **untrusted third-party bytes**.
//!
//! # Why this is not [`cbr_encoding::parse`]
//!
//! That reader implements the protocol's JSON value domain, which has no
//! non-integer numbers, no duplicate member names and no excluded code
//! points, because two participants have to agree on canonical bytes for
//! the same command. It is right to refuse a `0.7`.
//!
//! **A provider's response is not a protocol value.** A `0.7` in one is
//! ordinary, and refusing the whole response for containing one would turn
//! a perfectly good answer into an invalid-output outcome — and then into a
//! bounded repair that spends real tokens on a number CBR never reads.
//!
//! So this reader accepts the whole JSON grammar, keeps numbers **exactly
//! as written** rather than interpreting them, and applies the bounds that
//! a stranger's input needs: a byte ceiling and a depth ceiling. What CBR
//! is going to **seal** still goes back through the protocol's domain —
//! [`Json::recordable`] — so the strictness is kept where it belongs, at
//! the point of recording rather than at the point of reading.

#![allow(dead_code)]

/// The most bytes a response may be before it is refused unread.
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
/// The deepest nesting a response may carry. A recursive reader without a
/// bound of its own is a stack overflow a stranger gets to choose.
pub const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Json {
    Null,
    Bool(bool),
    /// **Kept as written.** This reader does not interpret numbers, so a
    /// float it never reads cannot be a reason to refuse a response.
    Number(String),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

/// Read a whole JSON document, or nothing.
pub fn read(bytes: &[u8]) -> Option<Json> {
    if bytes.len() > MAX_BYTES {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let mut reader = Reader {
        text: text.as_bytes(),
        at: 0,
    };
    reader.space();
    let value = reader.value(0)?;
    reader.space();
    // Trailing bytes mean this is not one document. Reading the first and
    // ignoring the rest is how a reader is talked into disagreeing with
    // whatever else reads the same bytes.
    if reader.at != reader.text.len() {
        return None;
    }
    Some(value)
}

struct Reader<'a> {
    text: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<u8> {
        self.text.get(self.at).copied()
    }

    fn space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn word(&mut self, word: &str, value: Json) -> Option<Json> {
        if self.text[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            return Some(value);
        }
        None
    }

    fn value(&mut self, depth: usize) -> Option<Json> {
        if depth > MAX_DEPTH {
            return None;
        }
        match self.peek()? {
            b'n' => self.word("null", Json::Null),
            b't' => self.word("true", Json::Bool(true)),
            b'f' => self.word("false", Json::Bool(false)),
            b'"' => self.string().map(Json::String),
            b'[' => self.array(depth),
            b'{' => self.object(depth),
            b'-' | b'0'..=b'9' => self.number(),
            _ => None,
        }
    }

    fn array(&mut self, depth: usize) -> Option<Json> {
        self.at += 1;
        let mut items = Vec::new();
        self.space();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Some(Json::Array(items));
        }
        loop {
            self.space();
            items.push(self.value(depth + 1)?);
            self.space();
            match self.peek()? {
                b',' => self.at += 1,
                b']' => {
                    self.at += 1;
                    return Some(Json::Array(items));
                }
                _ => return None,
            }
        }
    }

    fn object(&mut self, depth: usize) -> Option<Json> {
        self.at += 1;
        let mut members: Vec<(String, Json)> = Vec::new();
        self.space();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Some(Json::Object(members));
        }
        loop {
            self.space();
            let name = self.string()?;
            // Which of two same-named members a reader keeps is a choice,
            // and any choice makes it disagree with some other reader of
            // the same bytes.
            if members.iter().any(|(held, _)| held == &name) {
                return None;
            }
            self.space();
            if self.peek()? != b':' {
                return None;
            }
            self.at += 1;
            self.space();
            let value = self.value(depth + 1)?;
            members.push((name, value));
            self.space();
            match self.peek()? {
                b',' => self.at += 1,
                b'}' => {
                    self.at += 1;
                    return Some(Json::Object(members));
                }
                _ => return None,
            }
        }
    }

    fn number(&mut self) -> Option<Json> {
        let start = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        match self.peek()? {
            b'0' => self.at += 1,
            b'1'..=b'9' => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.at += 1;
                }
            }
            _ => return None,
        }
        if self.peek() == Some(b'.') {
            self.at += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return None;
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.at += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return None;
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.at += 1;
            }
        }
        // Held as written: this reader interprets no numbers.
        Some(Json::Number(
            std::str::from_utf8(&self.text[start..self.at]).ok()?.into(),
        ))
    }

    fn string(&mut self) -> Option<String> {
        if self.peek()? != b'"' {
            return None;
        }
        self.at += 1;
        let mut out = String::new();
        loop {
            match self.peek()? {
                b'"' => {
                    self.at += 1;
                    return Some(out);
                }
                b'\\' => {
                    self.at += 1;
                    self.escape(&mut out)?;
                }
                // Unescaped control characters are not JSON.
                0x00..=0x1f => return None,
                _ => {
                    let rest = std::str::from_utf8(&self.text[self.at..]).ok()?;
                    let character = rest.chars().next()?;
                    out.push(character);
                    self.at += character.len_utf8();
                }
            }
        }
    }

    fn escape(&mut self, out: &mut String) -> Option<()> {
        let escaped = self.peek()?;
        self.at += 1;
        let plain = match escaped {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => return self.unicode(out),
            _ => return None,
        };
        out.push(plain);
        Some(())
    }

    fn unicode(&mut self, out: &mut String) -> Option<()> {
        let first = self.hex4()?;
        let code = if (0xd800..0xdc00).contains(&first) {
            // A high surrogate must be followed by its low one. A lone
            // surrogate cannot become a Rust string, and substituting
            // U+FFFD would change bytes CBR may later have to seal.
            if self.peek()? != b'\\' {
                return None;
            }
            self.at += 1;
            if self.peek()? != b'u' {
                return None;
            }
            self.at += 1;
            let second = self.hex4()?;
            if !(0xdc00..0xe000).contains(&second) {
                return None;
            }
            0x10000 + ((first as u32 - 0xd800) << 10) + (second as u32 - 0xdc00)
        } else if (0xdc00..0xe000).contains(&first) {
            return None;
        } else {
            first as u32
        };
        out.push(char::from_u32(code)?);
        Some(())
    }

    fn hex4(&mut self) -> Option<u16> {
        let digits = self.text.get(self.at..self.at + 4)?;
        self.at += 4;
        u16::from_str_radix(std::str::from_utf8(digits).ok()?, 16).ok()
    }
}

impl Json {
    /// Back to JSON text.
    ///
    /// Needed because redaction runs over **decoded** string values: an
    /// escaped `\/` or `\u0026` hides a URL's shape from a scanner that
    /// reads raw bytes, so the document is taken apart, redacted and put
    /// back together rather than patched in place.
    pub fn write(&self) -> String {
        let mut out = String::new();
        self.write_into(&mut out);
        out
    }

    fn write_into(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            // As written: this reader interprets no numbers and this
            // writer does not reformat them.
            Json::Number(written) => out.push_str(written),
            Json::String(text) => write_string(text, out),
            Json::Array(items) => {
                out.push('[');
                for (at, item) in items.iter().enumerate() {
                    if at > 0 {
                        out.push(',');
                    }
                    item.write_into(out);
                }
                out.push(']');
            }
            Json::Object(members) => {
                out.push('{');
                for (at, (name, value)) in members.iter().enumerate() {
                    if at > 0 {
                        out.push(',');
                    }
                    write_string(name, out);
                    out.push(':');
                    value.write_into(out);
                }
                out.push('}');
            }
        }
    }

    pub fn get(&self, name: &str) -> Option<&Json> {
        match self {
            Json::Object(members) => members
                .iter()
                .find(|(held, _)| held == name)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    /// A whole, non-negative count. A negative or fractional figure is not
    /// a token count, and reading one as if it were is how a usage figure
    /// of `-1` becomes a spend of nothing.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Json::Number(written) => written.parse().ok(),
            _ => None,
        }
    }

    /// The same value inside the **protocol's** JSON domain, or `None` when
    /// it does not fit in it.
    ///
    /// This is where strictness belongs: at the point where a model's answer
    /// becomes something CBR seals, rather than at the point where bytes are
    /// read off a socket.
    pub fn recordable(&self) -> Option<cbr_encoding::Value> {
        use cbr_encoding::Value;
        Some(match self {
            Json::Null => Value::Null,
            Json::Bool(held) => Value::Bool(*held),
            Json::Number(written) => {
                let number: i64 = written.parse().ok()?;
                if number.unsigned_abs() > cbr_encoding::MAX_SAFE_INTEGER as u64 {
                    return None;
                }
                Value::Int(number)
            }
            Json::String(text) => Value::String(text.clone()),
            Json::Array(items) => Value::Array(
                items
                    .iter()
                    .map(Json::recordable)
                    .collect::<Option<Vec<_>>>()?,
            ),
            Json::Object(members) => Value::Object(
                members
                    .iter()
                    .map(|(name, value)| Some((name.clone(), value.recordable()?)))
                    .collect::<Option<Vec<_>>>()?,
            ),
        })
    }
}

/// A JSON string, escaping exactly what has to be escaped. Non-ASCII is
/// written as itself, because the output is UTF-8 and an escape would only
/// give something else somewhere a place to hide.
fn write_string(text: &str, out: &mut String) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}
