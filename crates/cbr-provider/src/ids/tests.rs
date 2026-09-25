//! The id rules, held for any input rather than for the inputs a fixture
//! happened to have.
//!
//! **Seeded, so a failure reproduces.** The generator is the xorshift the
//! evaluator's property tests use, written out here rather than taken
//! from a crate, so the lockfile does not move.

use std::collections::BTreeMap;

use super::*;
use crate::envelope::is_identifier;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<'a>(&mut self, from: &[&'a str]) -> &'a str {
        from[self.below(from.len())]
    }
}

/// The pieces a hostile path is made of: ASCII the grammar keeps, and
/// everything it does not — a space, `~`, `:`, `%`, a newline, `..`, a
/// leading dot, and one word in both of Unicode's normal forms.
const PIECES: &[&str] = &[
    "src",
    "a",
    "Z9",
    "lib.rs",
    "-",
    "_",
    ".",
    "..",
    ".hidden",
    " ",
    "~",
    ":",
    "%41",
    "\n",
    "\u{e9}t\u{e9}",
    "e\u{301}te\u{301}",
    "\u{4e00}\u{4e8c}",
    "gr\u{f6}\u{df}e",
    "~7E",
    "x~~y",
];

/// A path of up to twelve directories, cut to at most 300 bytes at a
/// character boundary.
fn path(rng: &mut Rng) -> String {
    let depth = 1 + rng.below(12);
    let mut parts = Vec::new();
    for _ in 0..depth {
        let mut part = String::new();
        for _ in 0..1 + rng.below(6) {
            part.push_str(rng.pick(PIECES));
        }
        parts.push(part);
    }
    let mut path = parts.join("/");
    while path.len() > 300 {
        path.pop();
    }
    if path.is_empty() {
        path.push('x');
    }
    path
}

/// A repository id: `[A-Za-z0-9._-]`, of 1 or 128 characters.
fn repository(rng: &mut Rng) -> String {
    const ALPHABET: &[u8] = b"abcXYZ019._-";
    let length = if rng.below(2) == 0 { 1 } else { 128 };
    let mut id = String::from("r");
    while id.len() < length {
        id.push(char::from(ALPHABET[rng.below(ALPHABET.len())]));
    }
    id
}

/// An item or claim id of one of the lengths either side of where each
/// prefix stops fitting: an identifier, so it starts with a letter.
fn item(rng: &mut Rng) -> String {
    const ALPHABET: &[u8] = b"abcXYZ019._:~-";
    let length = [114, 115, 126, 127, 128][rng.below(5)];
    let mut id = String::from("i");
    while id.len() < length {
        id.push(char::from(ALPHABET[rng.below(ALPHABET.len())]));
    }
    // An item whose own id reads as an omission is the collision recorded
    // as a follow-up (`omission`); it is pinned by its own test below.
    if id.contains(".o") {
        id = id.replace(".o", "_o");
    }
    id
}

/// `(prefix, digest, tail)` of a shortened id, or `None` for one that
/// is its prefix and rest unchanged.
fn shortened<'a>(id: &'a str, prefix: &str) -> Option<(&'a str, &'a str)> {
    let rest = id.strip_prefix(prefix)?.strip_prefix(MARK)?;
    Some((&rest[..DIGEST_DIGITS], &rest[DIGEST_DIGITS..]))
}

/// A readable span id back into its parts, for an id that was not
/// shortened.
fn read_span(rest: &str) -> (String, Vec<u8>, i64) {
    let rest = rest.strip_prefix("span-").expect("a span");
    let (repository, rest) = rest.split_once(':').expect("the repository ends at `:`");
    let (path, start) = rest.rsplit_once('-').expect("the start byte follows `-`");
    (
        repository.to_string(),
        decode(path).expect("the path decodes"),
        start.parse().expect("a number"),
    )
}

/// One id, recorded against what produced it: two different inputs
/// giving one id is the failure.
fn record(seen: &mut BTreeMap<String, String>, id: &str, input: String) {
    if let Some(earlier) = seen.insert(id.to_string(), input.clone()) {
        assert_eq!(earlier, input, "two inputs give the id {id}");
    }
}

#[test]
fn every_id_is_an_identifier_and_no_two_inputs_share_one() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    let mut shortened_spans = 0;
    for _ in 0..3000 {
        let repository = repository(&mut rng);
        let path = path(&mut rng);
        let start = match rng.below(3) {
            0 => 0,
            1 => i64::MAX,
            _ => (rng.next() >> 1) as i64,
        };
        let name = path.rsplit('/').next().unwrap_or_default().to_string();
        let item = item(&mut rng);
        let number = rng.below(12) + 1;

        let span_id = span(&repository, &path, start);
        let anchor_id = anchor(&repository, &name);
        let ids = [
            (
                "d-",
                discovered_section(&span_id),
                format!("span {repository:?} {path:?} {start}"),
            ),
            (
                "dc-",
                discovered_citation(&span_id),
                format!("span citation {repository:?} {path:?} {start}"),
            ),
            (
                "d-",
                discovered_section(&anchor_id),
                format!("anchor {repository:?} {name:?}"),
            ),
            ("d-", claim_section(&item), format!("claim {item:?}")),
            ("s-", item_section(&item), format!("item {item:?}")),
            (
                "c-",
                item_citation(&item),
                format!("item citation {item:?}"),
            ),
            (
                "s-",
                omission(&item, number),
                format!("omission {item:?} {number}"),
            ),
            (
                "packet.",
                packet_artifact(&item, number as i64),
                format!("packet {item:?} {number}"),
            ),
        ];
        for (prefix, id, input) in &ids {
            assert!(is_identifier(id), "{input} gave {id:?}");
            assert!(id.starts_with(prefix), "{input} gave {id:?}");
            record(&mut seen, id, input.clone());
            if shortened(id, prefix).is_some() {
                // The marker cannot be produced by an unshortened id: fed
                // back as a rest, the part after the prefix is shortened
                // again, to something else.
                let after = &id[prefix.len()..];
                assert_ne!(&bounded(prefix, after), id, "{id} is reproducible");
            }
        }

        // Spans and anchors are encoded, so a tail reads back, and `~~` is
        // only ever the marker. An item or claim id is an identifier as it
        // stands and is not encoded, so its tail is a suffix of it.
        for ((prefix, id, _), rest) in ids[..3].iter().zip([&span_id, &span_id, &anchor_id]) {
            if let Some((_, tail)) = shortened(id, prefix) {
                assert!(rest.ends_with(tail), "{tail} is not a tail of {rest}");
                // A tail that started inside an escape still decodes — hex
                // digits are kept as themselves — but to bytes the input
                // never ended with.
                let whole = decode(rest).expect("the rest decodes");
                let read = decode(tail).expect("the tail decodes");
                assert!(
                    whole.ends_with(&read),
                    "{tail} starts inside an escape of {rest}"
                );
            }
        }
        let rests = [
            claim(&item),
            item.clone(),
            item.clone(),
            format!("{item}.o{number}"),
            format!("{item}.{number}"),
        ];
        for ((prefix, id, _), rest) in ids[3..].iter().zip(&rests) {
            if let Some((_, tail)) = shortened(id, prefix) {
                assert!(rest.ends_with(tail), "{tail} is not a tail of {rest}");
            }
        }
        for (prefix, id, _) in &ids[..3] {
            let marker = id.find(MARK);
            assert!(
                marker.is_none() || marker == Some(prefix.len()),
                "`~~` inside {id}"
            );
        }
        let (section, citation) = (&ids[0].1, &ids[1].1);
        match (shortened(section, "d-"), shortened(citation, "dc-")) {
            (Some((one, _)), Some((other, _))) => {
                shortened_spans += 1;
                assert_eq!(one, other, "a span's section and citation share a digest");
            }
            (None, _) => {
                let (read_repository, read_path, read_start) =
                    read_span(section.strip_prefix("d-").expect("prefix"));
                assert_eq!(read_repository, repository);
                assert_eq!(read_path, path.as_bytes());
                assert_eq!(read_start, start);
            }
            (Some(_), None) => panic!("{section} was shortened and {citation}, longer, was not"),
        }
        // The omission of a shortened item keeps its `.o<n>`.
        assert!(ids[6].1.ends_with(&format!(".o{number}")), "{}", ids[6].1);
    }
    assert!(
        shortened_spans > 1000,
        "the generator reaches the shortened form: {shortened_spans}"
    );
    assert!(
        seen.len() > 20_000,
        "and produces distinct ids: {}",
        seen.len()
    );
}

#[test]
fn the_pairs_a_careless_rule_would_merge_stay_apart() {
    // `/` is written `:`, so a real `:` and a real `~` must be escaped or
    // `a/b` and `a:b` are one path.
    let spans: Vec<String> = ["a/b", "a:b", "a~3Ab", "a~3Ab/"]
        .iter()
        .map(|path| discovered_section(&span("app", path, 0)))
        .collect();
    let mut distinct = spans.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), spans.len(), "{spans:?}");
    assert_eq!(spans[0], "d-span-app:a:b-0", "a path stays readable");

    // A repository and a name joined by `-` let `a-b`+`c` and `a`+`b-c`
    // be one anchor; a repository id holds no `:`.
    assert_ne!(anchor("a-b", "c"), anchor("a", "b-c"));

    // Two long paths with the same last hundred bytes: plain truncation
    // keeps only the tail and would merge them.
    let shared = "x".repeat(100);
    let one = discovered_section(&span("app", &format!("{}/{shared}", "a".repeat(200)), 0));
    let other = discovered_section(&span("app", &format!("{}/{shared}", "b".repeat(200)), 0));
    assert_ne!(one, other);
    assert!(one.len() <= MAX && other.len() <= MAX);

    // Exactly 128 is kept; 129 is shortened.
    let at_the_limit = "i".repeat(126);
    assert_eq!(item_section(&at_the_limit), format!("s-{at_the_limit}"));
    let past_it = "i".repeat(127);
    let bounded_id = item_section(&past_it);
    assert!(bounded_id.starts_with("s-~~"), "{bounded_id}");
    assert!(bounded_id.len() <= MAX);

    // An id that already was one is unchanged, which the frozen J2 rubric
    // and every scripted fixture rely on.
    assert_eq!(item_section("log"), "s-log");
    assert_eq!(item_citation("log"), "c-log");
    assert_eq!(omission("log", 3), "s-log.o3");
    assert_eq!(packet_artifact("r-1", 2), "packet.r-1.2");
    assert_eq!(claim_section("c-adapter"), "d-claim-c-adapter");
}

#[test]
fn a_tail_never_starts_inside_an_escape_and_prefers_a_component() {
    // Every byte of this path but its last few is escaped, so most cuts
    // land inside an escape; three lengths of plain bytes at the end move
    // where the cut falls against the escapes before them, so one of them
    // lands inside one whatever the arithmetic. (Padding at the front would
    // move both together and change nothing.)
    for pad in 0..3 {
        let path = format!("{}{}", "\u{e9}".repeat(200), "x".repeat(pad));
        let id = discovered_section(&span("app", &path, 7));
        let (_, tail) = shortened(&id, "d-").expect("shortened");
        assert!(tail.starts_with('~'), "{tail}");
        let read = decode(tail).expect("the tail decodes");
        let whole = format!("span-app/{path}-7");
        assert!(
            whole.as_bytes().ends_with(&read),
            "{tail} starts inside an escape"
        );
    }

    // With components to cut at, the tail is whole components.
    let nested = format!("{}/deep/file.rs", "d".repeat(200));
    let id = discovered_section(&span("app", &nested, 0));
    let (_, tail) = shortened(&id, "d-").expect("shortened");
    assert_eq!(tail, ":deep:file.rs-0");
}

#[test]
fn the_recorded_collision_between_an_omission_and_an_item_named_like_one() {
    // **A follow-up, pinned so it is not forgotten or fixed by accident**:
    // item `x`'s first omission and item `x.o1`'s own section are one id.
    // J2's frozen rubric names omissions `s-<item>.o<n>`, so changing the
    // shape is its own change, with the rubric's.
    assert_eq!(omission("x", 1), item_section("x.o1"));
}

#[test]
fn encode_and_decode_are_inverse_on_every_byte() {
    let every: String = (1u8..=127).map(char::from).collect::<String>() + "\u{e9}\u{4e00}";
    let encoded = encode(&every);
    assert!(encoded.bytes().all(in_grammar), "{encoded}");
    assert_eq!(decode(&encoded).expect("decodes"), every.as_bytes());
    assert!(!encoded.contains(MARK), "{encoded}");
}
