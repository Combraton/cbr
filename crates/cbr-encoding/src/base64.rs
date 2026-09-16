//! Padded standard base64 (RFC 4648 section 4), strict.
//!
//! Evidence bytes travel in-band as base64 (EVIDENCE section 4), and a
//! provider and its clients must agree exactly on what is valid: decoding
//! refuses anything but the canonical padded form, so two readers never
//! disagree about which bytes a string carries.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn encode_base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Decode padded standard base64, strictly: length a multiple of 4, padding
/// only at the end, and no stray bits in the last character. `None` for
/// anything else, which the caller reports as `invalid_envelope`.
pub fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let value = |c: u8| -> Option<u32> { ALPHABET.iter().position(|a| *a == c).map(|p| p as u32) };
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for (index, quad) in bytes.chunks(4).enumerate() {
        let last = index == bytes.len() / 4 - 1;
        let padding = quad.iter().rev().take_while(|c| **c == b'=').count();
        if padding > 2 || (padding > 0 && !last) {
            return None;
        }
        let mut n = 0u32;
        for (i, c) in quad.iter().enumerate() {
            let digit = if i >= 4 - padding { 0 } else { value(*c)? };
            n = (n << 6) | digit;
        }
        // Canonical: bits the padding discards must be zero.
        if (padding == 1 && n & 0xff != 0) || (padding == 2 && n & 0xffff != 0) {
            return None;
        }
        out.push((n >> 16) as u8);
        if padding < 2 {
            out.push((n >> 8) as u8);
        }
        if padding < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_and_refuses_what_is_not_canonical() {
        for bytes in [
            &b""[..],
            b"f",
            b"fo",
            b"foo",
            b"foob",
            b"exact bytes \x00\xff\xfe ",
        ] {
            let encoded = encode_base64(bytes);
            assert_eq!(decode_base64(&encoded).as_deref(), Some(bytes), "{encoded}");
        }
        assert_eq!(
            encode_base64(b"exact bytes \x00\xff\xfe "),
            "ZXhhY3QgYnl0ZXMgAP/+IA=="
        );
        for bad in ["Zg", "Zg=", "Z===", "Zg==Zg==", "Zh==", "Zm9v!", "Zm=v"] {
            assert!(decode_base64(bad).is_none(), "{bad} must be refused");
        }
    }
}
