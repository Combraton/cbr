//! Redaction, at the boundary between the transport and the store.
//!
//! # Why here and not afterwards
//!
//! A provider response can carry a **third party's live credential**. That
//! is not a precaution: one of the pilot repositories has already seen a
//! speech response return a presigned object-store URL with the access key
//! and the signature in its query string
//! ([ADR 001 question 6](../../../docs/decisions/001-standalone-v0.1-scope-and-stack.md)).
//! Recorded unredacted, that is somebody else's working credential sitting
//! in CBR's store, and a later sweep cannot un-write it.
//!
//! So redaction runs **before the write**, and [`Redacted`] is the only
//! thing the store accepts. There is no constructor for one but [`redact`],
//! which makes "redaction happens before the write" a property of the types
//! rather than of whoever wrote the call site. The mutant *redaction moved
//! after the write* is a compile error here, and the scan test beside it is
//! what says the redaction itself works.
//!
//! # What is recognised, and what cannot be
//!
//! **A credential is not recognisable by looking at it** — that is what
//! makes it one. So the rules key on the things around it: a parameter's
//! name, a member's name, a scheme word, a vendor's prefix. Sweeping for
//! CBR's own key would prove nothing, because the credential that matters
//! here belongs to somebody CBR has never met.
//!
//! The consequence is that redaction is **generous where it is cheap**: an
//! over-redacted query parameter costs a record some detail, and an
//! under-redacted one leaks a key. It is not generous everywhere, and
//! [`redact_tests::ordinary_text_comes_through_untouched`] is what says so
//! — a redactor that redacts everything passes every other test and
//! destroys every record.

#![allow(dead_code)]

/// What replaces a credential.
pub const REDACTED: &str = "[redacted]";

/// How many times a value is percent-decoded before the scan gives up.
///
/// **Once was not enough.** `%253A` decodes to `%3A`, which decodes to
/// `:` — so a scanner that decodes once sees a string that still looks
/// like nothing and stops. Whatever encoded a credential twice can encode
/// it three times, so decoding runs until the text stops changing. The
/// bound is here because "until stable" against input somebody else wrote
/// is an invitation to decode for ever, and a value still changing at the
/// bound is one this code cannot show to be safe.
pub const PERCENT_DECODE_ROUNDS: usize = 5;

/// Query-parameter and member names whose value is a credential. Matched
/// case-insensitively, and by suffix where the vendor prefix varies.
const SECRET_NAMES: [&str; 12] = [
    "token",
    "key",
    "keyid",
    "sig",
    "signature",
    "password",
    "passwd",
    "secret",
    "credential",
    "auth",
    "authorization",
    "session",
];

/// Prefixes that are a credential wherever they appear, with the smallest
/// run of credential characters that follows one.
const SECRET_PREFIXES: [&str; 8] = [
    "sk-",
    "sk_",
    "ghp_",
    "gho_",
    "github_pat_",
    "xoxb-",
    "xoxp-",
    "AKIA",
];

/// How many credential characters must follow a prefix before it is one.
const PREFIX_RUN: usize = 16;

/// Bytes that have been through the redaction boundary.
///
/// **The only constructor is [`redact`].** A store write that takes one of
/// these cannot be handed bytes that did not pass through it.
pub struct Redacted(Vec<u8>);

impl Redacted {
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Debug for Redacted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Redacted({} bytes)", self.0.len())
    }
}

/// Replace everything credential-shaped in `bytes`.
///
/// # Structural, not case by case
///
/// Four leaks were found in the first version, and all four were the same
/// mistake: **the scanner read a string that something else controlled the
/// encoding of.** A JSON-escaped URL (`https:\/\/…\u0026Signature=…`) hid
/// its own shape from the "is this a URL" test. A percent-encoded URL
/// nested in a parameter that named no secret hid inside one. One byte that
/// was not UTF-8 turned redaction off for the whole record. And a provider
/// echoing the key back in a shape with no known prefix was caught by
/// nothing at all, because what made those bytes a credential was only that
/// they were *this process's* credential.
///
/// So:
///
/// 1. **If the bytes are JSON**, the document is taken apart and redaction
///    runs over the **decoded** string values, then it is written back. An
///    escape cannot hide anything from a scanner that never sees escapes.
/// 2. **A URL-shaped value is percent-decoded once** and scanned again, so
///    a nested URL's parameters are judged by their own names. The decoded,
///    redacted form is what is kept, because the encoded one cannot be
///    patched safely.
/// 3. **Bytes that are not UTF-8** are scanned through a lossy decoding
///    that cannot be written back, so if anything is found the record
///    **fails closed**: it is replaced by a marker and its length.
/// 4. **The held credential is scrubbed exactly**, last, in the shapes it
///    travels in — and if any percent-decoding of the result still holds
///    it, that record fails closed too.
pub fn redact(bytes: &[u8], scrubber: Option<&crate::keychain::Scrubber>) -> Redacted {
    let mut out = match std::str::from_utf8(bytes) {
        Ok(text) => match super::json::read(bytes) {
            Some(document) => redact_json(&document).write().into_bytes(),
            None => redact_text(text).into_bytes(),
        },
        Err(_) => {
            let lossy = String::from_utf8_lossy(bytes);
            if redact_text(&lossy) != lossy {
                closed(bytes.len())
            } else {
                bytes.to_vec()
            }
        }
    };
    if let Some(scrubber) = scrubber {
        out = scrubber.scrub(out, REDACTED);
        // An exact match sees the shapes a credential is known to travel
        // in. It does not see every encoding of it, so what is about to be
        // written is decoded once more and checked: a record that still
        // holds the credential cannot be made safe and is replaced.
        let (decoded, _) = percent_decode_to_stable(&String::from_utf8_lossy(&out));
        if scrubber.found_in(decoded.as_bytes()) {
            out = closed(bytes.len());
        }
    }
    Redacted(out)
}

/// What replaces a record that cannot be redacted safely. Its length is
/// kept because a record that is only a marker still says something.
fn closed(length: usize) -> Vec<u8> {
    format!("[redacted record: {length} bytes]").into_bytes()
}

/// One JSON value, redacted.
fn redact_json(value: &super::json::Json) -> super::json::Json {
    use super::json::Json;
    match value {
        Json::String(text) => Json::String(redact_value(text)),
        Json::Array(items) => Json::Array(items.iter().map(redact_json).collect()),
        Json::Object(members) => Json::Object(
            members
                .iter()
                .map(|(name, held)| {
                    // The name is the signal, as it is in a query string.
                    let value = match (names_a_secret(name), held) {
                        (true, Json::String(_)) => Json::String(REDACTED.into()),
                        (_, held) => redact_json(held),
                    };
                    // And a name can itself be a credential, if something
                    // built an object keyed by one.
                    (redact_value(name), value)
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

/// One decoded string, redacted — including through one percent-decoding
/// when it is URL-shaped.
fn redact_value(text: &str) -> String {
    let direct = redact_text(text);
    if !url_shaped(text) {
        return direct;
    }
    let (decoded, stable) = percent_decode_to_stable(text);
    let redacted = redact_text(&decoded);
    if !stable {
        // Still changing at the bound, so what this value decodes to is
        // not something this code has seen. It is not written out.
        return UNSETTLED.to_string();
    }
    if redacted != decoded {
        // Something was hiding in the encoding. The encoded form cannot be
        // patched without guessing where its boundaries were, so the
        // decoded and redacted form is what the record keeps.
        return redacted;
    }
    direct
}

/// What replaces a value whose decoding never settled.
const UNSETTLED: &str = "[redacted: encoding did not settle]";

fn url_shaped(text: &str) -> bool {
    let upper = text.to_ascii_uppercase();
    // The encoded forms of `://`, at one and at two rounds. A value that
    // is a URL only after two decodings is exactly the case that got past
    // the first version of this.
    text.contains("://") || upper.contains("%3A%2F%2F") || upper.contains("%253A%252F%252F")
}

/// Percent-decode until the text stops changing, up to
/// [`PERCENT_DECODE_ROUNDS`] times. Returns the settled text and whether
/// it had settled by then.
fn percent_decode_to_stable(text: &str) -> (String, bool) {
    let mut settled = text.to_string();
    for _ in 0..PERCENT_DECODE_ROUNDS {
        let next = percent_decode_once(&settled);
        if next == settled {
            return (settled, true);
        }
        settled = next;
    }
    (settled.clone(), percent_decode_once(&settled) == settled)
}

/// One round of percent-decoding.
fn percent_decode_once(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%'
            && at + 2 < bytes.len()
            && let Ok(pair) = std::str::from_utf8(&bytes[at + 1..at + 3])
            && let Ok(byte) = u8::from_str_radix(pair, 16)
        {
            out.push(byte);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// The whole text pipeline, for bytes that are not a JSON document and for
/// the decoded values of one that is.
fn redact_text(text: &str) -> String {
    let text = redact_bearer(text);
    let text = redact_named(&text, '=', is_query_boundary);
    let text = redact_named(&text, ':', is_member_boundary);
    let text = redact_prefixes(&text);
    redact_jwts(&text)
}

/// `Bearer <token>` keeps the word and loses the token.
fn redact_bearer(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("Bearer ") {
        out.push_str(&rest[..at + "Bearer ".len()]);
        let after = &rest[at + "Bearer ".len()..];
        let end = after
            .find(|c: char| !is_credential_character(c))
            .unwrap_or(after.len());
        if end == 0 {
            rest = after;
            continue;
        }
        out.push_str(REDACTED);
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// A value introduced by `separator` whose **name** says it is a secret.
///
/// Used twice: once for `name=value` in a query string, once for
/// `"name": "value"` in a JSON body. The name is the signal in both.
///
/// Walked by index over the whole text rather than by consuming a
/// remainder. A first version consumed, so the boundary test only ever saw
/// the bytes since the last separator: it redacted the first secret
/// parameter of a URL and then could no longer tell it was in one, leaving
/// every parameter after it in the clear. That is precisely the presigned
/// URL case, half done.
fn redact_named(text: &str, separator: char, boundary: fn(&str) -> Option<&str>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut written = 0usize;
    let mut search = 0usize;
    while let Some(offset) = text[search..].find(separator) {
        let at = search + offset;
        search = at + separator.len_utf8();
        if search <= written {
            continue;
        }
        let Some(name) = boundary(&text[..at]) else {
            continue;
        };
        if !names_a_secret(name) {
            continue;
        }
        // The value runs to the next delimiter. An opening quote is kept so
        // the record still shows that something was there.
        let after = &text[search..];
        let quoted = after.starts_with('"') || after.starts_with('\'');
        let from = search + usize::from(quoted);
        let value = &text[from..];
        let end = value
            .find(|c: char| !is_value_character(c, quoted))
            .unwrap_or(value.len());
        if end == 0 {
            continue;
        }
        out.push_str(&text[written..from]);
        out.push_str(REDACTED);
        written = from + end;
        search = written;
    }
    out.push_str(&text[written..]);
    out
}

/// The parameter name ending at a query string's `=`.
///
/// Scoped to the token the `=` is in, so that an ordinary `a=b` in prose is
/// not treated as a query parameter because a URL appeared earlier in the
/// same response.
fn is_query_boundary(before: &str) -> Option<&str> {
    let token_start = before
        .rfind(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '(' || c == ',')
        .map_or(0, |at| at + 1);
    let token = &before[token_start..];
    if !token.contains("://") {
        return None;
    }
    let start = token.rfind(['?', '&']).map(|at| at + 1)?;
    // The name has to be inside a query string, not in the path.
    if !token[..start].contains('?') {
        return None;
    }
    Some(&token[start..])
}

/// The member name ending at a JSON object's `:`.
fn is_member_boundary(before: &str) -> Option<&str> {
    let trimmed = before.trim_end();
    let quoted = trimmed.strip_suffix('"')?;
    let start = quoted.rfind('"')? + 1;
    Some(&quoted[start..])
}

fn names_a_secret(name: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    let name = name.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    SECRET_NAMES.iter().any(|secret| {
        // Suffix rather than equality: `OSSAccessKeyId`, `X-Amz-Signature`
        // and `access_token` are all the vendor's own spelling of one of
        // these, and enumerating vendors is a losing game.
        name == *secret || name.ends_with(secret)
    })
}

fn is_credential_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || "-_.+/=~".contains(character)
}

fn is_value_character(character: char, quoted: bool) -> bool {
    if quoted {
        return character != '"' && character != '\'';
    }
    is_credential_character(character) || character == '%'
}

/// Vendor prefixes, which are a credential wherever they appear.
fn redact_prefixes(text: &str) -> String {
    let mut out = text.to_string();
    for prefix in SECRET_PREFIXES {
        while let Some(at) = out.find(prefix) {
            let after = &out[at + prefix.len()..];
            let end = after
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
                .unwrap_or(after.len());
            if end < PREFIX_RUN {
                // Not long enough to be one. Nothing else in this pass can
                // match it either, so move on rather than loop for ever.
                break;
            }
            let replaced = format!("{}{REDACTED}", &out[..at]);
            out = format!("{replaced}{}", &after[end..]);
        }
    }
    out
}

/// A JSON Web Token: three dot-separated base64url runs starting `eyJ`.
fn redact_jwts(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("eyJ") {
        let after = &rest[at..];
        let end = after
            .find(|c: char| !is_credential_character(c))
            .unwrap_or(after.len());
        let candidate = &after[..end];
        if candidate.matches('.').count() == 2 && candidate.len() >= 20 {
            out.push_str(&rest[..at]);
            out.push_str(REDACTED);
            rest = &after[end..];
        } else {
            out.push_str(&rest[..at + 3]);
            rest = &rest[at + 3..];
        }
    }
    out.push_str(rest);
    out
}
