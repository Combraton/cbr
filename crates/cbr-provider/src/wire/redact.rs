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
pub fn redact(bytes: &[u8]) -> Redacted {
    // A record is not only text. Bytes that are not UTF-8 are passed
    // through: there is nothing here that can read them, and mangling them
    // would corrupt a record to no purpose.
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Redacted(bytes.to_vec());
    };
    let text = redact_bearer(text);
    let text = redact_named(&text, '=', is_query_boundary);
    let text = redact_named(&text, ':', is_member_boundary);
    let text = redact_prefixes(&text);
    let text = redact_jwts(&text);
    Redacted(text.into_bytes())
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
