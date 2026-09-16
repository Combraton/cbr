# Canonical encoding and digests `encoding/1` — release candidate

> **Status: Protocol 0.1 release candidate; accepted with M1 and M2.** Nothing here is released until the owner accepts the release candidate; at acceptance these names, the schemas and the conformance fixtures are frozen together for 0.1. Evidence and alternatives: [decision 003](../../decisions/003-canonical-encoding-and-digests.md). Test vectors: `conformance/vectors/encoding.json`, added with the M1 implementation.

## 1. The JSON value domain

Every JSON text exchanged under this protocol conforms to I-JSON ([RFC 7493](https://www.rfc-editor.org/rfc/rfc7493)) and to these stricter rules. A receiver rejects violations when it parses a frame ([STREAM §2](STREAM.md#2-frame-level-failures-close-the-connection)).

1. **No duplicate member names** in any object.
2. **Strings.** A string, after unescaping, contains only Unicode scalar values. It has no unpaired surrogates, whether written raw or as escapes such as `\ud800`, and no noncharacters (U+FDD0–U+FDEF, and every code point ending in FFFE or FFFF).
3. **Numbers.** Every number token is an integer written without a fraction, exponent or leading zeros. Its value is within −(2^53 − 1) … 2^53 − 1, and `-0` is not allowed. Decimal, large or 64-bit quantities are carried as strings with a format defined by the field's schema.
4. **No normalization.** Unicode normalization is never applied. `"é"` (U+00E9) and `"é"` are different values.

Rule 3 is stricter than I-JSON on purpose. Common JSON parsers silently round integers above 2^53 and give `1.0` and `1` the same value. So two different commands could otherwise produce one digest.

## 2. Canonical form

The canonical form of a value in this domain is its serialization under the JSON Canonicalization Scheme ([RFC 8785](https://www.rfc-editor.org/rfc/rfc8785)):

- No insignificant whitespace.
- Object members sorted recursively by their names as sequences of UTF-16 code units, compared as unsigned integers.
- Arrays keep their order.
- Strings escaped per RFC 8785 §3.2.2.2:
  - `"` and `\` are escaped.
  - Control characters U+0000–U+001F use the short escapes `\b \t \n \f \r` where they exist, otherwise `\u00hh` with lowercase hex.
  - All other characters are emitted as UTF-8, unescaped.
- Integers in decimal, with a leading `-` for negatives.
- UTF-8 output.

Because §1 excludes non-integers, RFC 8785's ECMAScript number formatting reduces to plain integer formatting.

## 3. Digest strings

A digest is written `<algorithm>:<lowercase hex>`, following the [OCI digest grammar](https://github.com/opencontainers/image-spec/blob/v1.1.1/descriptor.md#digests) restricted to lowercase hexadecimal:

| Algorithm | Encoded part | Status |
|---|---|---|
| `sha256` | exactly 64 characters `[0-9a-f]` | Mandatory to implement |
| `sha512` | exactly 128 characters `[0-9a-f]` | Optional; advertised as Core feature `core.digest-sha512` |

The rules for a digest string:

- **Grammar.** A string that does not match `^[a-z0-9]+([+._-][a-z0-9]+)*:[0-9a-f]+$`, or whose encoded length is wrong for a known algorithm, is `invalid_envelope`.
- **Unsupported algorithm.** A well-formed digest naming an algorithm the receiver does not support is `unsupported_digest_algorithm`. Unsupported algorithms never pass silently.
- **Weak algorithms.** MD5 and SHA-1 are never supported.

For exchange formats that expect a digest map, such as in-toto `DigestSet`, a digest string maps to `{ "<algorithm>": "<hex>" }` on export. That export is outside 0.1.

## 4. What gets digested

| Use | Digest input |
|---|---|
| Command intent ([Core §6.1](../profiles/CORE.md#61-command-intent-and-digest)) | The canonical form (§2) of the intent object |
| Immutable content: artifacts, evidence payloads, packets (M4) | The exact stored bytes. Never a re-serialization, even when the content is JSON. |

A receiver verifying a command digest recomputes it from the parsed values, not from the received bytes. Whitespace and member order on the wire therefore do not change a command's identity. Content digests are always over bytes. A JSON packet re-serialized with different bytes is different content.

## 5. What this does not establish

A matching digest shows that two byte sequences, or two value-domain objects, are identical. It does not show who produced them, that they are correct, or that they are authorized. Attribution needs the authenticated channel or, where a profile requires it, a signature (deferred with Remote trust).
