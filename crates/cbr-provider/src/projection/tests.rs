//! The projection family's reading, its bounds and its arithmetic.
//!
//! **Every bound is reached by a fixture here**, and the assertion is on
//! what was cut, sent or rendered rather than on a count somebody typed
//! (READINESS §8): a bound nothing presses against is not tested, which
//! is what the work pool's unsettled slot and the union's cap both taught
//! at M4.

use super::read as parse;
use super::*;
use crate::wire::Dialect;

// ---- fixtures -----------------------------------------------------------------

/// Cargo's test output: `binaries` test binaries of `tests` tests each,
/// the `(binary, test)` pairs in `failing` failing, with a failure block
/// of `block_lines` lines each.
fn cargo_log(
    binaries: usize,
    tests: usize,
    failing: &[(usize, usize)],
    block_lines: usize,
) -> String {
    let mut log = String::new();
    for binary in 0..binaries {
        let name = |test: usize| format!("crate_{binary}::tests::case_{test}");
        log.push_str(&format!(
            "     Running unittests src/lib.rs (target/debug/deps/crate_{binary}-0123456789abcdef)\n\n"
        ));
        log.push_str(&format!("running {tests} tests\n"));
        let mut failed = Vec::new();
        for test in 0..tests {
            if failing.contains(&(binary, test)) {
                log.push_str(&format!("test {} ... FAILED\n", name(test)));
                failed.push(test);
            } else {
                log.push_str(&format!("test {} ... ok\n", name(test)));
            }
        }
        log.push('\n');
        if !failed.is_empty() {
            log.push_str("failures:\n\n");
            for test in &failed {
                log.push_str(&format!("---- {} stdout ----\n", name(*test)));
                for line in 0..block_lines {
                    log.push_str(&format!(
                        "thread panicked at src/lib.rs:{line}: the queue held {line} ms, expected 5 ms\n"
                    ));
                }
                log.push('\n');
            }
            log.push_str("failures:\n");
            for test in &failed {
                log.push_str(&format!("    {}\n", name(*test)));
            }
            log.push_str(&format!(
                "\ntest result: FAILED. {} passed; {} failed; 0 ignored\n\n",
                tests - failed.len(),
                failed.len()
            ));
        } else {
            log.push_str(&format!(
                "test result: ok. {tests} passed; 0 failed; 0 ignored\n\n"
            ));
        }
    }
    log
}

/// A conformance manifest, pretty-printed, with `failing` results failed.
fn manifest(results: usize, failing: &[usize]) -> String {
    let entries: Vec<String> = (0..results)
        .map(|at| {
            let outcome = if failing.contains(&at) { "fail" } else { "pass" };
            format!(
                "    {{\n      \"fixture\": \"core.case-{at}\",\n      \"outcome\": \"{outcome}\",\n      \
                 \"elapsed\": \"{at}.5 ms\",\n      \"requirements\": [\n        \"CORE-{at}\"\n      ]\n    }}"
            )
        })
        .collect();
    format!(
        "{{\n  \"format\": \"combraton-conformance-result/1\",\n  \"runner\": {{\n    \"version\": \"0.1.0\"\n  }},\n  \
         \"results\": [\n{}\n  ],\n  \"summary\": {{\n    \"pass\": {}\n  }}\n}}\n",
        entries.join(",\n"),
        results - failing.len()
    )
}

/// Units tile the artifact, in order, each no larger than a unit and cut
/// on character boundaries, with lines that agree with the bytes.
fn assert_tiles(read: &Read, bytes: &[u8]) {
    let text = std::str::from_utf8(bytes).expect("text");
    let newlines = |to: usize| bytes[..to].iter().filter(|b| **b == b'\n').count();
    let mut expected = 0;
    for unit in &read.units {
        assert_eq!(unit.start, expected, "a gap or an overlap at {unit:?}");
        assert!(unit.end > unit.start, "an empty unit {unit:?}");
        assert!(
            unit.end - unit.start <= UNIT_BYTES,
            "a unit over the bound {unit:?}"
        );
        assert!(text.is_char_boundary(unit.start) && text.is_char_boundary(unit.end));
        let first = newlines(unit.start) + 1;
        let last = newlines(unit.end - 1) + 1;
        assert_eq!(
            (unit.first_line, unit.last_line),
            (first, last),
            "lines disagree with bytes for {unit:?}"
        );
        expected = unit.end;
    }
    assert_eq!(
        expected,
        bytes.len(),
        "the units stop short of the artifact"
    );
}

const SUBJECT: Subject<'static> = Subject {
    artifact: "ingest.0123456789abcdef.1",
    digest: "sha256:0000000000000000000000000000000000000000000000000000000000000000",
    capture: &[],
};

/// The extents a rendered projection declares, read from its text:
/// `(start, end, Some(bytes))` for an excerpt, `(start, end, None)` with
/// the reason for an omission.
fn extents(content: &str) -> Vec<(usize, usize, Result<Vec<u8>, String>)> {
    let bytes = content.as_bytes();
    let mut found = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let end = bytes[at..]
            .iter()
            .position(|b| *b == b'\n')
            .map_or(bytes.len(), |offset| at + offset);
        let line = std::str::from_utf8(&bytes[at..end]).expect("utf-8");
        at = end + 1;
        let Some((tag, rest)) = line.split_once("] bytes ") else {
            continue;
        };
        let (range, rest) = rest.split_once(", lines ").expect("lines");
        let (start, stop) = range.split_once('-').expect("a range");
        let (start, stop): (usize, usize) =
            (start.parse().expect("start"), stop.parse().expect("end"));
        if tag.starts_with("[e") {
            let excerpt = bytes[at..at + (stop - start)].to_vec();
            at += stop - start;
            let closing = format!("\n[end {}]\n", &tag[1..]);
            assert_eq!(&bytes[at..at + closing.len()], closing.as_bytes());
            at += closing.len();
            found.push((start, stop, Ok(excerpt)));
        } else {
            let reason = rest.split_once("omitted: ").expect("a reason").1;
            found.push((start, stop, Err(reason.to_string())));
        }
    }
    found
}

/// A rendering tiles its artifact, and every excerpt is the artifact's
/// bytes at its range.
fn assert_renders_honestly(rendered: &Rendered, bytes: &[u8]) {
    let mut expected = 0;
    let mut omissions = 0;
    for (start, end, carried) in extents(&rendered.content) {
        assert_eq!(start, expected, "a gap or an overlap at {start}");
        match carried {
            Ok(excerpt) => assert_eq!(excerpt.as_slice(), &bytes[start..end]),
            Err(_) => omissions += 1,
        }
        expected = end;
    }
    assert_eq!(expected, bytes.len(), "the ledger stops short");
    assert_eq!(
        omissions,
        rendered.omitted.len(),
        "an omission the packet would not declare"
    );
    assert!(
        rendered.content.len() <= PROJECTION_BYTES,
        "over the projection's bytes"
    );
}

/// [`super::render`], for every test here that renders a projection that
/// fits: a refusal is a failure of the test that met it.
/// `a_model_arm_that_does_not_fit_is_refused_and_never_published` calls
/// the real one.
fn render(
    read: &Read,
    bytes: &[u8],
    choice: &Choice,
    plan: &Plan,
    subject: Subject<'_>,
) -> Rendered {
    super::render(read, bytes, choice, plan, subject).expect("a projection that fits")
}

// ---- reading ---------------------------------------------------------------------

#[test]
fn every_format_tiles_its_artifact_with_units_no_larger_than_a_unit() {
    let long_line = format!("{}\n", "\u{4e00}".repeat(UNIT_BYTES));
    let cases: Vec<(Format, String)> = vec![
        (Format::Libtest, cargo_log(3, 40, &[(1, 7)], 90)),
        (Format::Json, manifest(60, &[3, 40])),
        (
            Format::JsonLines,
            "{\"reason\":\"compiler-artifact\"}\n\n{\"reason\":\"build-finished\",\"success\":true}\n"
                .to_string(),
        ),
        (Format::Lines, "one\ntwo\r\nthree without a newline".to_string()),
        (Format::Lines, format!("before\n{long_line}after\n")),
        (Format::Lines, String::new()),
    ];
    for (format, text) in cases {
        let read = parse(text.as_bytes());
        assert_eq!(read.format, format, "{text:.80}");
        assert_tiles(&read, text.as_bytes());
    }
}

#[test]
fn formats_are_recognised_in_order_and_anything_else_is_lines() {
    assert_eq!(parse(&[0xff, 0xfe, 0x00]).format, Format::Binary);
    assert!(
        parse(&[0xff, 0xfe]).units.is_empty(),
        "nothing is cut from bytes that are not text"
    );
    assert_eq!(parse(b"  {\"a\": [1, 2]}  \n").format, Format::Json);
    assert_eq!(parse(b"[1]\n[2]\n").format, Format::JsonLines);
    assert_eq!(
        parse(b"\"a lone string\"\n").format,
        Format::Lines,
        "a scalar is not a document"
    );
    assert_eq!(parse(b"{\"a\": 1} trailing\n").format, Format::Lines);
    assert_eq!(parse(b"{\"a\": 1}\nnot json\n").format, Format::Lines);
    assert_eq!(parse(b"running 3 tests\n").format, Format::Libtest);
    assert_eq!(parse(b"test a::b ... ok\n").format, Format::Libtest);
    assert_eq!(parse(b"plain words\n").format, Format::Lines);
    // **The depth bound, reached**: one level deeper than a document may
    // nest is read as text, which is always safe.
    let deep = |levels: usize| format!("{}1{}", "[".repeat(levels), "]".repeat(levels));
    assert_eq!(parse(deep(JSON_DEPTH).as_bytes()).format, Format::Json);
    assert_eq!(parse(deep(JSON_DEPTH + 1).as_bytes()).format, Format::Lines);
}

#[test]
fn cargos_test_output_is_cut_where_its_own_structure_is() {
    let log = cargo_log(2, 5, &[(0, 1), (1, 3)], 4)
        + "error: test failed, to rerun pass `-p crate_1 --lib`\n";
    let read = parse(log.as_bytes());
    assert_eq!(read.format, Format::Libtest);
    let text_of = |unit: &Unit| &log[unit.start..unit.end];
    // Run identity is its own unit, a line each with the blank lines after
    // it.
    let identity: Vec<&str> = read
        .units
        .iter()
        .filter(|unit| unit.kind == Kind::Identity)
        .map(text_of)
        .collect();
    assert_eq!(identity.len(), 6, "{identity:?}");
    assert!(
        identity
            .iter()
            .all(|line| line.lines().filter(|one| !one.is_empty()).count() == 1),
        "{identity:?}"
    );
    assert!(identity[0].ends_with(")\n\n"), "{:?}", identity[0]);
    // A failure's block runs from its header to the next block or the list
    // of names, and is a failure.
    let block = read
        .units
        .iter()
        .find(|unit| text_of(unit).starts_with("---- crate_0::tests::case_1 stdout ----"))
        .expect("the block");
    assert_eq!(block.kind, Kind::Failure);
    assert!(
        text_of(block).ends_with("expected 5 ms\n\n"),
        "{:?}",
        text_of(block)
    );
    // The names are the bytes at their ranges, in byte order, and the
    // cargo error is named too.
    let names: Vec<&str> = read
        .named
        .iter()
        .map(|named| &log[named.start..named.end])
        .collect();
    assert_eq!(
        names,
        [
            "crate_0::tests::case_1",
            "crate_1::tests::case_3",
            "error: test failed, to rerun pass `-p crate_1 --lib`"
        ]
    );
    assert_tiles(&read, log.as_bytes());
}

#[test]
fn a_block_longer_than_a_unit_is_cut_on_lines_and_a_line_longer_than_a_unit_on_characters() {
    // **The unit bound, reached**, both ways a block can exceed it.
    let log = cargo_log(1, 3, &[(0, 1)], 120);
    let read = parse(log.as_bytes());
    let pieces: Vec<&Unit> = read
        .units
        .iter()
        .filter(|unit| unit.kind == Kind::Failure && unit.last_line > unit.first_line)
        .collect();
    assert!(
        pieces.len() >= 3,
        "a 120-line block is several pieces: {pieces:?}"
    );
    assert!(
        pieces.iter().all(|unit| unit.group == pieces[0].group),
        "one group"
    );
    assert!(
        pieces
            .iter()
            .all(|unit| log.as_bytes()[unit.end - 1] == b'\n'),
        "cut after a newline"
    );
    assert!(
        pieces
            .iter()
            .any(|unit| unit.end - unit.start > UNIT_BYTES - 100),
        "no piece came near the bound"
    );

    let wide = "\u{4e00}".repeat(UNIT_BYTES);
    let read = parse(wide.as_bytes());
    assert!(read.units.len() >= 3);
    assert_tiles(&read, wide.as_bytes());
    assert!(
        read.units
            .iter()
            .any(|unit| unit.end - unit.start == UNIT_BYTES - UNIT_BYTES % 3),
        "the widest piece is the bound, backed off to a character boundary"
    );
}

#[test]
fn plain_text_is_cut_every_block_of_lines() {
    let text: String = (0..(2 * BLOCK_LINES + 5))
        .map(|n| format!("line {n}\n"))
        .collect();
    let read = parse(text.as_bytes());
    let sizes: Vec<usize> = read
        .units
        .iter()
        .map(|unit| unit.last_line - unit.first_line + 1)
        .collect();
    assert_eq!(sizes, [BLOCK_LINES, BLOCK_LINES, 5]);
}

#[test]
fn a_json_document_is_cut_at_its_members_and_what_is_not_a_list_is_its_identity() {
    let json = manifest(60, &[3, 40]);
    let read = parse(json.as_bytes());
    assert_eq!(read.format, Format::Json);
    let text_of = |unit: &Unit| &json[unit.start..unit.end];
    // The members at the root that are not the list of results.
    let identity: String = read
        .units
        .iter()
        .filter(|unit| unit.kind == Kind::Identity)
        .map(text_of)
        .collect();
    assert!(identity.contains("\"format\": \"combraton-conformance-result/1\""));
    assert!(identity.contains("\"version\": \"0.1.0\""));
    assert!(identity.contains("\"pass\": 58"));
    assert!(
        !identity.contains("\"fixture\""),
        "a result is not the run's identity"
    );
    // A result is a unit, labelled by a path built from the raw key.
    let third = read
        .units
        .iter()
        .find(|unit| text_of(unit).contains("\"core.case-3\""))
        .expect("result 3");
    assert_eq!(third.label, "$.\"results\"[3]");
    assert_eq!(third.kind, Kind::Failure);
    // Named by its fixture, the bytes inside the quotes.
    let names: Vec<&str> = read
        .named
        .iter()
        .map(|named| &json[named.start..named.end])
        .collect();
    assert_eq!(names, ["core.case-3", "core.case-40"]);
    // **A value and its unit are bytes, never restated.**
    assert!(text_of(third).contains("\"elapsed\": \"3.5 ms\""));
    assert_tiles(&read, json.as_bytes());
}

#[test]
fn json_lines_are_cut_a_record_a_line_and_cargos_verdicts_are_read() {
    let lines = concat!(
        "{\"reason\":\"compiler-artifact\",\"package_id\":\"a\"}\n",
        "{\"reason\":\"compiler-message\",\"message\":{\"level\":\"error\",\"rendered\":\"error[E0308]\"}}\n",
        "{\"reason\":\"compiler-message\",\"message\":{\"level\":\"warning\"}}\n",
        "{\"reason\":\"build-finished\",\"success\":false}\n",
    );
    let read = parse(lines.as_bytes());
    assert_eq!(read.format, Format::JsonLines);
    let kinds: Vec<Kind> = read.units.iter().map(|unit| unit.kind).collect();
    assert_eq!(
        kinds,
        [Kind::Other, Kind::Failure, Kind::Other, Kind::Failure],
        "an error message and a failed build are failures; a failed build stays one"
    );
    // **rustc's own diagnostics say `level` at the top**, where cargo's
    // wrap it in `message`: each is a failure the way its format says.
    let diagnostics = concat!(
        "{\"$message_type\":\"diagnostic\",\"message\":\"unused variable\",\"level\":\"warning\"}\n",
        "{\"$message_type\":\"diagnostic\",\"message\":\"mismatched types\",\"level\":\"error\"}\n",
    );
    let rustc = parse(diagnostics.as_bytes());
    assert_eq!(rustc.format, Format::JsonLines);
    assert_eq!(
        rustc.units.iter().map(|unit| unit.kind).collect::<Vec<_>>(),
        [Kind::Other, Kind::Failure]
    );
    let finished = "{\"reason\":\"build-finished\",\"success\":true}\n";
    let read_ok = parse(format!("{{\"reason\":\"x\"}}\n{finished}").as_bytes());
    assert_eq!(
        read_ok.units[1].kind,
        Kind::Identity,
        "a finished build is the run's identity"
    );
    // **Cargo's error is a cargo error**, named by what it renders; the
    // failed build says only that the build failed, and is not a second.
    let named: Vec<(&str, Failing)> = read
        .named
        .iter()
        .map(|named| (&lines[named.start..named.end], named.kind))
        .collect();
    assert_eq!(named, [("error[E0308]", Failing::Error)]);
}

#[test]
fn a_json_label_is_cut_within_its_bound() {
    let key = "k".repeat(LABEL_BYTES);
    let json = format!("{{\"{key}\": [{}]}}", vec!["\"x\""; UNIT_BYTES].join(","));
    let read = parse(json.as_bytes());
    assert!(read.units.len() > 1);
    for unit in &read.units {
        assert!(unit.label.len() <= LABEL_BYTES, "{}", unit.label);
    }
    assert!(
        read.units[0].label.ends_with("..."),
        "{}",
        read.units[0].label
    );
}

// ---- partitioning ------------------------------------------------------------------

/// What each part's candidates cost, as the partition counted them.
fn part_costs(read: &Read, bytes: &[u8], plan: &Plan) -> Vec<usize> {
    plan.parts
        .iter()
        .map(|units| {
            units
                .iter()
                .map(|&index| cost(read, &read.units[index], bytes))
                .sum()
        })
        .collect()
}

#[test]
fn a_part_holds_at_most_its_bytes_as_the_body_carries_them() {
    // **The part bound, reached**, and counted escaped: a log of control
    // characters, which JSON writes six bytes apiece, fills a part at a
    // sixth of the raw bytes.
    let plain = "x".repeat(60) + "\n";
    let log: String = plain.repeat(1500);
    let read_plain = parse(log.as_bytes());
    let plan = partition(&read_plain, log.as_bytes(), SUBJECT, true);
    let costs = part_costs(&read_plain, log.as_bytes(), &plan);
    assert!(plan.parts.len() >= 2, "{costs:?}");
    assert!(costs.iter().all(|&cost| cost <= PART_BYTES), "{costs:?}");
    let largest_unit = read_plain
        .units
        .iter()
        .map(|unit| cost(&read_plain, unit, log.as_bytes()))
        .max()
        .expect("units");
    assert!(
        costs[0] > PART_BYTES - largest_unit,
        "the first part was not filled to its bound: {costs:?}"
    );

    let control = "\u{1}".repeat(60) + "\n";
    let noisy: String = control.repeat(1500);
    let read_noisy = parse(noisy.as_bytes());
    let noisy_plan = partition(&read_noisy, noisy.as_bytes(), SUBJECT, true);
    assert!(
        noisy_plan.parts.len() > plan.parts.len() * 4,
        "escaping was not counted: {} parts against {}",
        noisy_plan.parts.len(),
        plan.parts.len()
    );
    assert!(
        part_costs(&read_noisy, noisy.as_bytes(), &noisy_plan)
            .iter()
            .all(|&cost| cost <= PART_BYTES)
    );
}

#[test]
fn a_part_holds_at_most_its_count_of_candidates() {
    // **The candidate bound, reached**: tiny units, each its own group,
    // one more than a part may hold. JSON lines give a unit a record, where
    // plain text would put twenty lines in one.
    let records: String = (0..=PART_UNITS).map(|n| format!("[{n}]\n")).collect();
    let read = parse(records.as_bytes());
    assert_eq!(read.units.len(), PART_UNITS + 1);
    let plan = partition(&read, records.as_bytes(), SUBJECT, true);
    let sizes: Vec<usize> = plan.parts.iter().map(Vec::len).collect();
    assert_eq!(sizes, [PART_UNITS, 1]);
}

#[test]
fn identity_is_never_offered_to_a_model() {
    let log = cargo_log(2, 3, &[(0, 1)], 2);
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes(), SUBJECT, true);
    for units in &plan.parts {
        for &index in units {
            assert_ne!(read.units[index].kind, Kind::Identity);
        }
    }
    let offered: usize = plan.parts.iter().map(Vec::len).sum();
    let identity = read
        .units
        .iter()
        .filter(|unit| unit.kind == Kind::Identity)
        .count();
    assert_eq!(
        offered + identity,
        read.units.len(),
        "every other unit is offered once"
    );
}

#[test]
fn a_group_larger_than_a_part_is_the_one_cut_and_the_model_is_told_it_was_made() {
    // A failure block of about three parts' bytes: the only thing
    // partitioning has to cut.
    let log = cargo_log(1, 2, &[(0, 0)], 1400);
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes(), SUBJECT, true);
    assert!(plan.parts.len() >= 3, "{}", plan.parts.len());
    assert_eq!(plan.split.len(), 1, "{:?}", plan.split);
    let (first, last, from, to) = plan.split[0];
    assert_eq!(read.units[first].group, read.units[last].group);
    assert!(to > from, "a cut group spans parts: {:?}", plan.split[0]);
    for (part, units) in plan.parts.iter().enumerate() {
        let holds = units.iter().any(|&index| index >= first && index <= last);
        assert_eq!(holds, part >= from && part <= to, "part {part}");
    }
    // **A failure's block is never offered**, being the rule's to carry, so
    // no model saw a piece of it and nothing is unresolved about it.
    assert!(
        offered(&plan)
            .iter()
            .all(|&index| index < first || index > last)
    );
    let chosen = choose_by_model(&read, &[]);
    let rendered = render(&read, log.as_bytes(), &chosen, &plan, SUBJECT);
    assert!(!rendered.content.contains("unresolved: "));

    // Twenty lines of two kilobytes are one block of plain text, larger
    // than a part: a model is offered it in two questions, and the model
    // arm says so, numbered by the questions it was asked.
    let text = format!("{}\n", "y".repeat(2000)).repeat(BLOCK_LINES);
    let read = parse(text.as_bytes());
    let plan = partition(&read, text.as_bytes(), SUBJECT, true);
    assert_eq!(plan.split.len(), 1, "{:?}", plan.split);
    assert_eq!(plan.asked.len(), 2, "{:?}", plan.asked);
    let (first, _, _, _) = plan.split[0];
    let chosen = choose_by_model(&read, &[first]);
    let rendered = render(&read, text.as_bytes(), &chosen, &plan, SUBJECT);
    assert!(
        rendered
            .content
            .contains("one block cut across parts 1-2; each part saw only its own pieces"),
        "{}",
        rendered.content
    );
    // The deterministic rule showed no part to anybody, so it has nothing
    // unresolved to say.
    let ruled = render(
        &read,
        text.as_bytes(),
        &choose_by_rule(&read),
        &plan,
        SUBJECT,
    );
    assert!(!ruled.content.contains("unresolved: "));
}

// ---- what happens next ----------------------------------------------------------------

/// A log whose offerable units fill exactly `parts` parts.
fn filling(parts: usize) -> String {
    // Each block of twenty 90-byte lines is one unit of about 1.9 KB
    // escaped, so sixteen of them fill a part.
    let line = format!("{}\n", "z".repeat(89));
    let mut text = String::new();
    let mut count = 0;
    loop {
        text.push_str(&line);
        count += 1;
        if count % BLOCK_LINES == 0 {
            let read = parse(text.as_bytes());
            if partition(&read, text.as_bytes(), SUBJECT, true).parts.len() > parts {
                // One block too many: take it back.
                text.truncate(text.len() - line.len() * BLOCK_LINES);
                return text;
            }
        }
    }
}

#[test]
fn more_parts_than_the_bound_is_insufficient_capacity_with_or_without_a_model() {
    // **The capacity, reached from both sides.**
    let at_bound = filling(MAX_PARTS);
    let read_at = parse(at_bound.as_bytes());
    let plan_at = partition(&read_at, at_bound.as_bytes(), SUBJECT, true);
    assert_eq!(plan_at.parts.len(), MAX_PARTS);
    for may_ask in [true, false] {
        assert_ne!(
            next(&read_at, at_bound.as_bytes(), &plan_at, SUBJECT, may_ask),
            Next::Insufficient,
            "the bound itself is within capacity"
        );
    }
    let over = format!(
        "{at_bound}{}",
        format!("{}\n", "z".repeat(89)).repeat(BLOCK_LINES)
    );
    let read_over = parse(over.as_bytes());
    let plan_over = partition(&read_over, over.as_bytes(), SUBJECT, true);
    assert_eq!(plan_over.parts.len(), MAX_PARTS + 1);
    for may_ask in [true, false] {
        assert_eq!(
            next(&read_over, over.as_bytes(), &plan_over, SUBJECT, may_ask),
            Next::Insufficient,
            "one block past the bound, may_ask {may_ask}"
        );
    }
    assert!(!too_large(INPUT_BYTES));
    assert!(too_large(INPUT_BYTES + 1));
    assert!(
        over.len() <= INPUT_BYTES,
        "the parts bound bites before the bytes bound for this input: {}",
        over.len()
    );
}

#[test]
fn a_whole_artifact_that_fits_is_carried_whole_and_nothing_is_asked() {
    let log = cargo_log(1, 6, &[(0, 2)], 3);
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes(), SUBJECT, true);
    let Next::Carry(choice) = next(&read, log.as_bytes(), &plan, SUBJECT, true) else {
        panic!("a model was asked about a log that fits whole");
    };
    assert_eq!(choice.by, By::Everything);
    let rendered = render(&read, log.as_bytes(), &choice, &plan, SUBJECT);
    assert_renders_honestly(&rendered, log.as_bytes());
    assert!(rendered.omitted.is_empty());
    assert_eq!(
        extents(&rendered.content).len(),
        1,
        "one excerpt, the whole"
    );
    // **It needs no question, and says so**: the header's count is the
    // questions the input needs, which the harness reads to decide what it
    // asks.
    assert!(plan.asked.is_empty(), "offered {:?}", plan.asked);
    assert!(
        how(&rendered.content).contains(" in 0 parts;"),
        "{}",
        how(&rendered.content)
    );
}

#[test]
fn a_whole_artifact_is_carried_as_one_excerpt_however_many_units_it_has() {
    // **Everything fits, so everything is carried**, even when carrying it
    // unit by unit would pass through more excerpts than the bound on the
    // way: forty failures between passing tests are eighty-one runs until
    // the last unit joins them into one.
    let failing: Vec<(usize, usize)> = (0..40).map(|test| (0, test * 2)).collect();
    let log = cargo_log(1, 80, &failing, 0);
    assert!(log.len() < PROJECTION_BYTES / 2, "{}", log.len());
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes(), SUBJECT, true);
    let Next::Carry(choice) = next(&read, log.as_bytes(), &plan, SUBJECT, true) else {
        panic!("a model was asked about a log that fits whole");
    };
    assert_eq!(choice.by, By::Everything);
    let rendered = render(&read, log.as_bytes(), &choice, &plan, SUBJECT);
    assert_renders_honestly(&rendered, log.as_bytes());
    assert!(rendered.omitted.is_empty(), "{}", rendered.content);
    assert_eq!(extents(&rendered.content).len(), 1);
}

#[test]
fn with_nothing_a_model_could_be_offered_the_rule_is_used_and_says_so() {
    // **Nothing offerable is nothing asked.** A log that is all run
    // identity and too large to carry whole leaves no part to put to a
    // model, and a projection that said a model chose its excerpts would
    // be reporting a choice nobody made.
    let log: String = (0..400)
        .map(|n| format!("running {n} tests\n\ntest result: ok. {n} passed; 0 failed\n\n"))
        .collect();
    let read = parse(log.as_bytes());
    assert!(read.units.iter().all(|unit| unit.kind == Kind::Identity));
    let plan = partition(&read, log.as_bytes(), SUBJECT, true);
    assert!(plan.parts.is_empty());
    let Next::Carry(choice) = next(&read, log.as_bytes(), &plan, SUBJECT, true) else {
        panic!("a model was to be asked with nothing to show it");
    };
    assert_eq!(choice.by, By::Rule);
}

#[test]
fn with_no_model_the_rule_carries_the_failures_and_then_the_runs_identity() {
    let log = cargo_log(6, 150, &[(2, 5), (4, 9)], 6);
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes(), SUBJECT, true);
    let Next::Carry(choice) = next(&read, log.as_bytes(), &plan, SUBJECT, false) else {
        panic!("no model, and something was asked");
    };
    assert_eq!(choice.by, By::Rule);
    assert_eq!(next(&read, log.as_bytes(), &plan, SUBJECT, true), Next::Ask);
    let rendered = render(&read, log.as_bytes(), &choice, &plan, SUBJECT);
    assert_renders_honestly(&rendered, log.as_bytes());
    for name in ["crate_2::tests::case_5", "crate_4::tests::case_9"] {
        assert!(
            rendered
                .content
                .contains(&format!("---- {name} stdout ----")),
            "{name}'s block was not carried"
        );
    }
    for (_, _, carried) in extents(&rendered.content) {
        if let Err(reason) = carried {
            assert!(
                reason == "not_selected" || reason == "over_projection",
                "{reason}"
            );
        }
    }
    assert!(
        rendered.content.contains("test result: "),
        "no identity at all was carried"
    );
}

#[test]
fn a_failure_is_carried_even_where_run_identity_alone_would_fill_the_projection() {
    // **Why failures come first.** Forty test binaries print two runs of
    // identity lines apiece, eighty excerpts where the bound is
    // twenty-four; carried first they would fill every excerpt before the
    // last binary's failure was reached. The failure is what the
    // projection is for.
    let log = cargo_log(40, 3, &[(39, 1)], 3);
    let read = parse(log.as_bytes());
    let identity_runs = read
        .units
        .windows(2)
        .filter(|pair| pair[0].kind == Kind::Identity && pair[1].kind != Kind::Identity)
        .count();
    assert!(identity_runs > MAX_EXCERPTS, "{identity_runs}");
    let plan = partition(&read, log.as_bytes(), SUBJECT, true);
    let rendered = render(
        &read,
        log.as_bytes(),
        &choose_by_rule(&read),
        &plan,
        SUBJECT,
    );
    assert_renders_honestly(&rendered, log.as_bytes());
    assert!(
        rendered
            .content
            .contains("---- crate_39::tests::case_1 stdout ----"),
        "the last binary's failure was crowded out by run identity"
    );
}

#[test]
fn bytes_that_are_not_text_are_one_declared_omission() {
    let bytes = vec![0x80_u8; 5000];
    let read = parse(&bytes);
    let plan = partition(&read, &bytes, SUBJECT, true);
    let Next::Carry(choice) = next(&read, &bytes, &plan, SUBJECT, true) else {
        panic!("a model was asked about bytes that are not text");
    };
    assert_eq!(choice.by, By::NotText);
    let rendered = render(&read, &bytes, &choice, &plan, SUBJECT);
    assert_eq!(rendered.omitted, [(1, "unavailable")]);
    assert_eq!(
        extents(&rendered.content),
        [(0, 5000, Err("not_text".to_string()))]
    );
}

// ---- rendering ------------------------------------------------------------------------

#[test]
fn an_omission_that_held_something_chosen_is_over_projection_and_one_that_held_nothing_is_not() {
    // Twenty failures of thirty-line blocks: more than the projection's
    // bytes, so the rule's later choices are out of room.
    let failing: Vec<(usize, usize)> = (0..20).map(|test| (0, test * 3)).collect();
    let log = cargo_log(1, 60, &failing, 30);
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes(), SUBJECT, true);
    let choice = choose_by_rule(&read);
    let rendered = render(&read, log.as_bytes(), &choice, &plan, SUBJECT);
    assert_renders_honestly(&rendered, log.as_bytes());
    let reasons: Vec<String> = extents(&rendered.content)
        .into_iter()
        .filter_map(|(_, _, carried)| carried.err())
        .collect();
    assert!(
        reasons.iter().any(|reason| reason == "over_projection"),
        "{reasons:?}"
    );
    for (number, protocol) in &rendered.omitted {
        let reason = &reasons[number - 1];
        assert_eq!(
            *protocol,
            match reason.as_str() {
                "over_projection" => "output_capacity",
                "not_selected" => "applicability",
                other => panic!("{other}"),
            }
        );
    }
    // An extent with nothing chosen in it is not selected, whatever its
    // neighbours were.
    for (start, end, carried) in extents(&rendered.content) {
        if carried == Err("not_selected".to_string()) {
            assert!(
                read.units
                    .iter()
                    .filter(|unit| unit.start >= start && unit.end <= end)
                    .all(|unit| unit.kind == Kind::Other),
                "a not_selected extent held something chosen: {start}-{end}"
            );
        }
    }
}

#[test]
fn a_projection_never_exceeds_its_bytes_and_this_one_reaches_them() {
    // **The projection bound, reached**: failures enough to fill it
    // several times, and what is left of it is less than the smallest
    // failure that was out of room.
    let failing: Vec<(usize, usize)> = (0..40).map(|test| (0, test)).collect();
    let log = cargo_log(1, 40, &failing, 25);
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes(), SUBJECT, true);
    let rendered = render(
        &read,
        log.as_bytes(),
        &choose_by_rule(&read),
        &plan,
        SUBJECT,
    );
    assert_renders_honestly(&rendered, log.as_bytes());
    let excerpts = extents(&rendered.content)
        .iter()
        .filter(|(_, _, carried)| carried.is_ok())
        .count();
    assert!(
        excerpts < MAX_EXCERPTS,
        "this fixture is about bytes, not excerpts"
    );
    let smallest_block = read
        .units
        .iter()
        .filter(|unit| unit.kind == Kind::Failure && unit.last_line > unit.first_line)
        .map(|unit| unit.end - unit.start)
        .min()
        .expect("blocks");
    assert!(
        PROJECTION_BYTES - rendered.content.len() < smallest_block + 128,
        "room was left for another block: {} of {PROJECTION_BYTES}",
        rendered.content.len()
    );
}

#[test]
fn a_projection_carries_at_most_its_count_of_excerpts() {
    // **The excerpt bound, reached**: small failures, each its own
    // excerpt between runs of passing tests, far more than the bound.
    let failing: Vec<(usize, usize)> = (0..60).map(|test| (0, test * 25 + 12)).collect();
    let log = cargo_log(1, 1500, &failing, 0);
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes(), SUBJECT, true);
    let choice = Choice {
        by: By::Model,
        chosen: read
            .units
            .iter()
            .map(|unit| unit.kind == Kind::Failure)
            .collect(),
    };
    let rendered = render(&read, log.as_bytes(), &choice, &plan, SUBJECT);
    assert_renders_honestly(&rendered, log.as_bytes());
    let excerpts = extents(&rendered.content)
        .iter()
        .filter(|(_, _, carried)| carried.is_ok())
        .count();
    assert_eq!(excerpts, MAX_EXCERPTS);
    assert!(rendered.omitted.len() <= MAX_EXCERPTS + 1);
    assert!(
        rendered.content.len() < PROJECTION_BYTES,
        "this fixture is about excerpts, not bytes"
    );
}

#[test]
fn named_failures_are_listed_to_their_bound_and_counted_past_it() {
    let failing: Vec<(usize, usize)> = (0..NAMED_FAILURES + 4).map(|test| (0, test)).collect();
    let log = cargo_log(1, NAMED_FAILURES + 10, &failing, 0);
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes(), SUBJECT, true);
    let rendered = render(
        &read,
        log.as_bytes(),
        &choose_by_rule(&read),
        &plan,
        SUBJECT,
    );
    assert!(
        rendered
            .content
            .contains(&format!("failures named: {}\n", NAMED_FAILURES + 4))
    );
    let listed = rendered
        .content
        .lines()
        .filter(|line| line.starts_with("  crate_0::"))
        .count();
    assert_eq!(listed, NAMED_FAILURES);
    let first_unlisted = read.named[NAMED_FAILURES];
    assert!(rendered.content.contains(&format!(
        "  and 4 more, the first at bytes {}-{}\n",
        first_unlisted.start, first_unlisted.end
    )));
}

#[test]
fn a_long_name_is_shown_to_its_bound_at_the_range_it_shows() {
    let name = format!("tests::{}", "\u{4e00}".repeat(NAME_BYTES));
    let log = format!("running 1 test\ntest {name} ... FAILED\n\ntest result: FAILED.\n");
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes(), SUBJECT, true);
    let rendered = render(
        &read,
        log.as_bytes(),
        &choose_by_rule(&read),
        &plan,
        SUBJECT,
    );
    let line = rendered
        .content
        .lines()
        .find(|line| line.starts_with("  tests::"))
        .expect("the name");
    let (shown, range) = line[2..].rsplit_once(" at bytes ").expect("a range");
    let (start, end) = range.split_once('-').expect("start-end");
    let (start, end): (usize, usize) = (start.parse().expect("start"), end.parse().expect("end"));
    assert!(
        shown.len() <= NAME_BYTES && shown.len() > NAME_BYTES - 4,
        "{}",
        shown.len()
    );
    assert_eq!(
        &log.as_bytes()[start..end],
        shown.as_bytes(),
        "the range is the shown bytes"
    );
}

#[test]
fn the_capture_anchors_are_shown_to_their_bound() {
    let anchors: Vec<(String, String)> = (0..CAPTURE_ANCHORS + 2)
        .map(|n| (format!("kind{n}"), "a".repeat(ANCHOR_BYTES + 10)))
        .collect();
    let subject = Subject {
        capture: &anchors,
        ..SUBJECT
    };
    let log = cargo_log(1, 2, &[], 0);
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes(), subject, true);
    let rendered = render(
        &read,
        log.as_bytes(),
        &choose_by_rule(&read),
        &plan,
        subject,
    );
    let line = rendered
        .content
        .lines()
        .find(|line| line.starts_with("captured at "))
        .expect("the capture line");
    assert_eq!(line.matches("kind").count(), CAPTURE_ANCHORS);
    assert!(line.ends_with(", and 2 more"));
    assert!(line.contains(&format!("{}...", "a".repeat(ANCHOR_BYTES - 3))));
    assert!(!line.contains(&"a".repeat(ANCHOR_BYTES - 2)));
    let none = render(
        &read,
        log.as_bytes(),
        &choose_by_rule(&read),
        &plan,
        SUBJECT,
    );
    assert!(none.content.contains("\ncaptured with no anchors\n"));
}

#[test]
fn a_capture_anchor_is_shown_on_one_line_whatever_it_holds() {
    // **An anchor is the descriptor's text, not CBR's**, and evidence/1
    // lets it hold anything of 1 to 256 characters. Shown raw, one holding
    // a newline wrote header lines of its own. Each is escaped as JSON
    // escapes a string — the escape character too, so two anchors are
    // never shown alike — plus the two Unicode separators a line reader
    // may break on, and cut to its bound between escapes, never inside one.
    let anchors = [
        (
            "note".to_string(),
            "abc\nfailures named: 0\nread as lines".to_string(),
        ),
        (
            "k\r\u{1}\u{2028}".to_string(),
            "back\\slash\ttab\u{7f}\u{85}\u{2029}".to_string(),
        ),
        ("long".to_string(), "\u{1}".repeat(ANCHOR_BYTES)),
    ];
    let subject = Subject {
        capture: &anchors,
        ..SUBJECT
    };
    let log = cargo_log(1, 2, &[(0, 1)], 1);
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes(), subject, true);
    let rendered = render(
        &read,
        log.as_bytes(),
        &choose_by_rule(&read),
        &plan,
        subject,
    );
    let lines: Vec<&str> = rendered.content.split('\n').collect();
    let starting = |prefix: &str| lines.iter().filter(|line| line.starts_with(prefix)).count();
    assert_eq!(starting("failures named: "), 1, "{}", rendered.content);
    assert_eq!(starting("read as "), 1, "{}", rendered.content);
    let line = lines
        .iter()
        .find(|line| line.starts_with("captured at "))
        .expect("the capture line");
    assert_eq!(
        *line,
        format!(
            "captured at note abc\\nfailures named: 0\\nread as lines, \
             k\\r\\u0001\\u2028 back\\\\slash\\ttab\\u007f\\u0085\\u2029, long {}...",
            "\\u0001".repeat((ANCHOR_BYTES - 3) / 6)
        )
    );
    assert!(!line.chars().any(char::is_control), "{line:?}");
}

#[test]
fn the_largest_header_leaves_most_of_a_projection_for_its_ledger() {
    // **The header's own worst case**, computed: every anchor at its
    // bound, every name at its bound, the widest numbers an input may
    // have, and every cut a partition can make. What is left is the room
    // the excerpts have, and it has to be most of the projection.
    let anchors: Vec<(String, String)> = (0..CAPTURE_ANCHORS)
        .map(|_| ("k".repeat(ANCHOR_BYTES), "i".repeat(ANCHOR_BYTES)))
        .collect();
    let artifact = "a".repeat(128);
    let subject = Subject {
        artifact: &artifact,
        capture: &anchors,
        ..SUBJECT
    };
    let name = "n".repeat(NAME_BYTES);
    let mut log = String::from("running 1 test\n");
    for _ in 0..NAMED_FAILURES + 1 {
        log.push_str(&format!("test {name} ... FAILED\n"));
    }
    log.push_str(&"w".repeat(INPUT_BYTES - log.len() - 1));
    log.push('\n');
    let read = parse(log.as_bytes());
    let plan = Plan {
        parts: vec![Vec::new(); MAX_PARTS],
        split: vec![(0, 0, 0, MAX_PARTS - 1); MAX_PARTS - 1],
        asked: vec![Vec::new(); MAX_PARTS],
        floor: Floor::default(),
        rule: None,
    };
    let nothing = Choice {
        by: By::Model,
        chosen: vec![false; read.units.len()],
    };
    let carried = vec![false; read.units.len()];
    // **Drawn at the model arm's widest**: its label listing every excerpt
    // a projection may hold, and every cut group's line.
    let header = draw(
        &read,
        log.as_bytes(),
        &nothing,
        &carried,
        Some(&carried),
        &Frame::widest(subject, &plan),
    );
    assert!(header.content.contains(&widest_suffix()));
    assert!(
        header.content.len() < PROJECTION_BYTES / 2,
        "the header alone takes {} of {PROJECTION_BYTES}",
        header.content.len()
    );
}

/// The digest of the projection a fixed fixture renders.
///
/// The same shape as the compiler's golden packet digest, for the same
/// reason: [`FORMAT`] is a string somebody has to remember to change, and
/// it is inside the bytes this covers. Change how a document is cut or
/// rendered and this changes; change [`FORMAT`] and it changes too. Either
/// way the two move in one commit.
const GOLDEN_PROJECTION_DIGEST: &str =
    "sha256:b11ec1bf551011793a789eb7f9c69db952ef034d476b70a69171f0827cb4fc3b";

#[test]
fn the_projection_a_fixed_fixture_renders_has_not_changed_without_its_format() {
    let failing: Vec<(usize, usize)> = (0..6).map(|test| (1, test * 7)).collect();
    let log = cargo_log(3, 45, &failing, 40);
    let read = parse(log.as_bytes());
    let anchors = [("git_tree".to_string(), "t".repeat(40))];
    let subject = Subject {
        capture: &anchors,
        ..SUBJECT
    };
    let plan = partition(&read, log.as_bytes(), subject, true);
    let rendered = render(
        &read,
        log.as_bytes(),
        &choose_by_rule(&read),
        &plan,
        subject,
    );
    assert!(rendered.content.contains(FORMAT));
    // The fixture reaches what it guards: something is omitted for room.
    assert!(
        rendered.content.contains("omitted: over_projection"),
        "{}",
        rendered.content
    );
    assert!(rendered.content.contains("omitted: not_selected"));
    assert_eq!(
        cbr_encoding::digest_bytes(rendered.content.as_bytes()),
        GOLDEN_PROJECTION_DIGEST,
        "\n\nThe projection this fixture renders has changed. If that is intended, it is a \
         change to the format, so `FORMAT` must change in the same commit and this digest \
         with it.\n\n{}",
        rendered.content
    );
}

// ---- asking a model ----------------------------------------------------------------------

#[test]
fn a_parts_body_carries_no_more_than_the_partition_counted() {
    let failing: Vec<(usize, usize)> = (0..30).map(|test| (0, test * 5)).collect();
    let log = cargo_log(2, 160, &failing, 12);
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes(), SUBJECT, true);
    assert!(plan.parts.len() >= 2);
    for (number, units) in plan.parts.iter().enumerate() {
        let (candidates, labels) = candidates(&read, log.as_bytes(), SUBJECT.artifact, units);
        let part = Part {
            artifact: SUBJECT.artifact.to_string(),
            format: read.format.name(),
            size: read.size,
            number: number + 1,
            of: plan.parts.len(),
            labels,
            floor: Floor::default(),
        };
        let body = ask("MiniMax-M3", "which tests failed", &part, &candidates);
        let text = &body.messages[0].text;
        let block = &text[text.find("Excerpts:\n").expect("excerpts") + "Excerpts:\n".len()..];
        assert!(
            body_bytes(block) <= PART_BYTES,
            "part {number} sent {} of {PART_BYTES}",
            body_bytes(block)
        );
        // Every candidate is its unit's exact bytes, framed by an id the
        // schema offers and nothing else does.
        for (candidate, &index) in candidates.iter().zip(units) {
            let unit = &read.units[index];
            assert_eq!(
                candidate.text.as_bytes(),
                &log.as_bytes()[unit.start..unit.end]
            );
            assert_eq!(
                candidate.path,
                format!("{}@{}-{}", SUBJECT.artifact, unit.start, unit.end)
            );
            assert!(text.contains(&format!("\n[{}] lines ", candidate.id)));
        }
    }
}

#[test]
fn an_answer_is_read_against_its_own_part_and_nothing_else() {
    let log = cargo_log(1, 30, &[(0, 3)], 4);
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes(), SUBJECT, true);
    let (candidates, _) = candidates(&read, log.as_bytes(), SUBJECT.artifact, &plan.parts[0]);
    let reply = |json: &str| {
        crate::wire::response::Reply::Structure(cbr_encoding::parse(json.as_bytes()).expect("json"))
    };
    assert_eq!(
        chosen(&reply(r#"{"ids":["u2","u1","u2"]}"#), &candidates),
        Ok(vec![0, 1])
    );
    assert_eq!(
        chosen(&reply(r#"{"ids":["u999"]}"#), &candidates),
        Err(crate::selection::NOT_OFFERED)
    );
    assert_eq!(
        chosen(&reply(r#"{"terms":["u1"]}"#), &candidates),
        Err(crate::selection::NOT_STRUCTURED)
    );
    let too_many = format!(
        r#"{{"ids":[{}]}}"#,
        vec!["\"u1\""; PART_UNITS + 1].join(",")
    );
    assert_eq!(
        chosen(&reply(&too_many), &candidates),
        Err(crate::discovery::OVER_BOUND),
        "more ids than a part may offer is over its bound, not a shorter list"
    );
}

#[test]
fn cbrs_own_instruction_is_the_only_instruction_and_the_selector_names_the_part() {
    let log = cargo_log(1, 30, &[(0, 3)], 4);
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes(), SUBJECT, true);
    let (candidates, labels) = candidates(&read, log.as_bytes(), SUBJECT.artifact, &plan.parts[0]);
    let part = Part {
        artifact: SUBJECT.artifact.to_string(),
        format: read.format.name(),
        size: read.size,
        number: 1,
        of: 1,
        labels,
        floor: Floor::default(),
    };
    let body = ask("MiniMax-M3", "which tests failed", &part, &candidates);
    let system = body.system.as_deref().expect("an instruction");
    assert!(system.contains("not instructions"), "{system}");
    assert_eq!(body.messages.len(), 1);
    let Want::Structure { schema } = &body.want else {
        panic!("not a structure");
    };
    let offered: Vec<&str> = schema
        .get("properties")
        .and_then(|properties| properties.get("ids"))
        .and_then(|ids| ids.get("items"))
        .and_then(|items| items.get("enum"))
        .and_then(Value::as_array)
        .expect("an enum")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let ids: Vec<&str> = candidates
        .iter()
        .map(|candidate| candidate.id.as_str())
        .collect();
    assert_eq!(offered, ids);
    assert_eq!(
        selector(SUBJECT.artifact, SUBJECT.digest, 2, 3),
        format!(
            "project_large_result {FORMAT} {} {} part 2 of 3",
            SUBJECT.artifact, SUBJECT.digest
        )
    );
}

// ---- the floor the model adds to (m5a-3) -------------------------------------------------
//
// J2's live run found what a model choosing *instead of* the rule does to
// a failing log: both models left out failure blocks the parser had
// already found, and the projection declared them `not_selected`. From
// `cbr-project-large-result/2` the rule's projection is a **floor**: the
// model is asked what to add to it, and nothing it answers can take
// anything away.

/// The byte ranges a rendering carries.
fn carried(content: &str) -> Vec<(usize, usize)> {
    extents(content)
        .into_iter()
        .filter(|(_, _, carried)| carried.is_ok())
        .map(|(start, end, _)| (start, end))
        .collect()
}

/// Whether every byte of `inner` lies inside one of `outer`'s ranges.
fn covers(outer: &[(usize, usize)], inner: &[(usize, usize)]) -> bool {
    inner.iter().all(|&(start, end)| {
        (start..end).all(|at| outer.iter().any(|&(from, to)| from <= at && at < to))
    })
}

/// The header line that says what was read, and how.
fn how(content: &str) -> &str {
    content
        .lines()
        .find(|line| line.starts_with("read as "))
        .expect("a `read as` line")
}

/// The excerpts the header says the model added, or `None` when it names
/// no model at all.
fn added(content: &str) -> Option<Vec<usize>> {
    let (_, list) = how(content).split_once(", which added ")?;
    if list == "nothing" {
        return Some(Vec::new());
    }
    Some(
        list.split(", ")
            .map(|id| id.strip_prefix('e').expect("eN").parse().expect("a number"))
            .collect(),
    )
}

/// The excerpts of a rendering, numbered as it numbers them.
fn numbered(content: &str) -> Vec<(usize, usize)> {
    carried(content)
}

/// The label's longest form, built here from the words the header uses:
/// every excerpt a projection may hold, listed.
fn widest_suffix() -> String {
    let ids: Vec<String> = (1..=MAX_EXCERPTS).map(|n| format!("e{n}")).collect();
    format!(
        "; then chosen by the model, one question per part, which added {}",
        ids.join(", ")
    )
}

/// **The widest the model arm could have drawn this rendering**, as a
/// reader computes it from the text: the label at its longest, and every
/// omission under the longer of its two reasons.
fn widest_fits(content: &str) -> bool {
    let line = how(content);
    let base = line
        .split_once("; then chosen by the model")
        .map_or(line, |(base, _)| base);
    let widened = content
        .replacen(line, &format!("{base}{}", widest_suffix()), 1)
        .replace("omitted: not_selected", "omitted: over_projection");
    widened.len() <= PROJECTION_BYTES && carried(content).len() <= MAX_EXCERPTS
}

/// The units a rendering carries whole.
fn units_in(read: &Read, ranges: &[(usize, usize)]) -> Vec<bool> {
    read.units
        .iter()
        .map(|unit| covers(ranges, &[(unit.start, unit.end)]))
        .collect()
}

/// Every unit any part offers a model, in byte order.
fn offered(plan: &Plan) -> Vec<usize> {
    let mut all: Vec<usize> = plan.asked.iter().flatten().copied().collect();
    all.sort_unstable();
    all
}

/// The room the deterministic rule's projection of `text` leaves under
/// the projection's bytes.
fn slack(text: &str) -> usize {
    let read = parse(text.as_bytes());
    let plan = partition(&read, text.as_bytes(), SUBJECT, true);
    let rendered = render(
        &read,
        text.as_bytes(),
        &choose_by_rule(&read),
        &plan,
        SUBJECT,
    );
    PROJECTION_BYTES - rendered.content.len()
}

/// `build(pad)` for a pad that leaves the rule's projection room inside
/// `window`: the pad grows the first failure, which the rule always
/// carries first, so each byte of it is a byte less room until the last
/// failure the rule carried no longer fits.
fn tuned(build: impl Fn(usize) -> String, window: std::ops::Range<usize>) -> String {
    let middle = (window.start + window.end) / 2;
    let mut pad = 0;
    for _ in 0..64 {
        let text = build(pad);
        let room = slack(&text);
        if window.contains(&room) {
            return text;
        }
        pad = if room > middle {
            pad + room - middle
        } else {
            pad + room + 1
        };
    }
    panic!("no pad leaves the rule's projection room inside {window:?}");
}

/// JSON lines: `small` passing records of about 80 bytes, `failing`
/// failing records of about a kilobyte, the first grown by `pad`, and
/// `big` passing records of about 1.9 kilobytes, in that order.
fn records(small: usize, failing: usize, big: usize, pad: usize) -> String {
    let mut text = String::new();
    for n in 0..small {
        text.push_str(&format!(
            "{{\"name\":\"small-{n}\",\"outcome\":\"pass\",\"elapsed\":\"0.{n:04} ms\",\"note\":\"quick\"}}\n"
        ));
    }
    for n in 0..failing {
        let detail = "d".repeat(950 + if n == 0 { pad } else { 0 });
        text.push_str(&format!(
            "{{\"name\":\"failing-{n}\",\"outcome\":\"fail\",\"detail\":\"{detail}\"}}\n"
        ));
    }
    for n in 0..big {
        text.push_str(&format!(
            "{{\"name\":\"big-{n}\",\"outcome\":\"pass\",\"detail\":\"{}\"}}\n",
            "b".repeat(1900)
        ));
    }
    text
}

/// JSON lines as [`records`] makes them, with passing records **of every
/// size** from about 80 bytes to about a kilobyte in steps of 13: whatever
/// room the floor leaves, some record is within a frame of it, so an offer
/// that counted a unit's cost other than by drawing it would be wrong
/// about one of them.
fn graded(pad: usize) -> String {
    let mut text = String::new();
    for n in 0..70 {
        text.push_str(&format!(
            "{{\"name\":\"graded-{n}\",\"outcome\":\"pass\",\"note\":\"{}\"}}\n",
            "g".repeat(n * 13)
        ));
    }
    text + &records(0, 20, 4, pad)
}

/// A failing cargo log with room left: three failures, a cargo error, a
/// blank line and another cargo error — the `\n` between two carried
/// excerpts that J2's red log had four of — and passing tests enough that
/// it does not fit whole.
fn red_shaped() -> String {
    cargo_log(6, 80, &[(1, 4), (1, 9), (2, 3)], 12)
        + "error: test failed, to rerun pass `-p crate_1 --lib`\n\n\
           error: 2 targets failed:\n    `-p crate_1 --lib`\n    `-p crate_2 --lib`\n"
}

/// What a model arm renders for `picks`, **through the order `next`
/// decides**: when no model would be asked, what is carried is what the
/// request gets.
fn arm(read: &Read, bytes: &[u8], plan: &Plan, picks: &[usize]) -> Rendered {
    match next(read, bytes, plan, SUBJECT, true) {
        Next::Ask => render(read, bytes, &choose_by_model(read, picks), plan, SUBJECT),
        Next::Carry(choice) => render(read, bytes, &choice, plan, SUBJECT),
        Next::Insufficient => panic!("the fixture is over the projection's capacity"),
    }
}

/// Every answer a test here gives a model: nothing, everything it was
/// offered, each offered unit alone, and every other one.
fn answers(plan: &Plan) -> Vec<Vec<usize>> {
    let all = offered(plan);
    let mut answers = vec![Vec::new(), all.clone()];
    answers.extend(all.iter().map(|&one| vec![one]));
    answers.push(all.iter().copied().step_by(2).collect());
    answers
}

/// A failing log whose failures are each their own excerpt until the run
/// identity between them joins them into one, **with the last failure
/// fitting the rule's header by less than the model's label**: the rule
/// carries every failure only because it carries them before the header
/// grows, and the finished floor has room enough that a model is asked.
/// Repacked under the model's header, the floor would lose that failure.
fn joined_floor() -> String {
    let build = |pad: usize| {
        let mut log = String::new();
        for n in 0..20 {
            let name = "n".repeat(500 + if n == 0 { pad } else { 0 });
            log.push_str(&format!(
                "test tests::{name}_{n} ... FAILED\nrunning 1 test\n"
            ));
        }
        for n in 0..300 {
            log.push_str(&format!("test tests::passing_{n} ... ok\n"));
        }
        log
    };
    // The room the rule's header leaves when every failure, and no run
    // identity yet, is carried: the moment the last failure is accepted.
    let before_identity = |text: &str| {
        let read = parse(text.as_bytes());
        let plan = partition(&read, text.as_bytes(), SUBJECT, true);
        let failures: Vec<bool> = read
            .units
            .iter()
            .map(|unit| unit.kind == Kind::Failure)
            .collect();
        let drawn = draw(
            &read,
            text.as_bytes(),
            &choose_by_rule(&read),
            &failures,
            None,
            &Frame::of(SUBJECT, &plan),
        );
        PROJECTION_BYTES as i64 - drawn.content.len() as i64
    };
    let mut pad = 0;
    for _ in 0..64 {
        let text = build(pad);
        let room = before_identity(&text);
        if (1..60).contains(&room) {
            let read = parse(text.as_bytes());
            let plan = partition(&read, text.as_bytes(), SUBJECT, true);
            assert_eq!(
                next(&read, text.as_bytes(), &plan, SUBJECT, true),
                Next::Ask,
                "the joined floor is meant to leave room for a question"
            );
            return text;
        }
        pad = (pad as i64 + room - 30).max(0) as usize;
    }
    panic!("no pad puts the last failure within the model's label of the bound");
}

#[test]
fn the_model_arm_carries_every_byte_the_rule_carries_whatever_the_model_answers() {
    // **Criterion 1, by construction.** The rule's projection is the floor,
    // and a model's answer — none, all it was offered, any one of them,
    // every other one — only adds to it: every byte the rule carries, the
    // model arm carries, and both bounds hold. A red log, two green ones
    // (one whose run identity leaves room, one it fills), a manifest, and
    // one whose rule projection is within 60 bytes of the bound.
    let near = tuned(|pad| records(30, 24, 4, pad), 1..60);
    let joined = joined_floor();
    let fixtures = [
        red_shaped(),
        cargo_log(8, 60, &[], 0),
        cargo_log(30, 20, &[], 0),
        manifest(120, &[5, 70]),
        near,
        joined,
    ];
    let mut asked_somewhere = false;
    for text in &fixtures {
        let bytes = text.as_bytes();
        let read = parse(bytes);
        let plan = partition(&read, bytes, SUBJECT, true);
        let rule = render(&read, bytes, &choose_by_rule(&read), &plan, SUBJECT);
        let floor = carried(&rule.content);
        asked_somewhere |= next(&read, bytes, &plan, SUBJECT, true) == Next::Ask;
        for picks in answers(&plan) {
            let rendered = arm(&read, bytes, &plan, &picks);
            assert_renders_honestly(&rendered, bytes);
            assert!(
                covers(&carried(&rendered.content), &floor),
                "answering {picks:?} took away bytes the rule carries:\n{}",
                rendered.content
            );
            assert!(rendered.fits(), "{picks:?}: {}", rendered.content.len());
        }
    }
    assert!(asked_somewhere, "no fixture here asks a model");
}

#[test]
fn a_failure_the_parser_named_is_never_not_selected_in_either_arm() {
    // **The word J2's judges read as a lie.** A failure the parser found is
    // carried, or declared chosen and out of room; `not_selected` is for
    // bytes nothing chose, and a failure is chosen by the rule in both
    // arms, whatever a model answers.
    for text in [
        red_shaped(),
        cargo_log(1, 40, &(0..40).map(|t| (0, t)).collect::<Vec<_>>(), 25),
    ] {
        let bytes = text.as_bytes();
        let read = parse(bytes);
        let plan = partition(&read, bytes, SUBJECT, true);
        let mut arms = vec![render(&read, bytes, &choose_by_rule(&read), &plan, SUBJECT)];
        let all = offered(&plan);
        for picks in [
            Vec::new(),
            all.clone(),
            all.iter().copied().step_by(3).collect(),
        ] {
            arms.push(render(
                &read,
                bytes,
                &choose_by_model(&read, &picks),
                &plan,
                SUBJECT,
            ));
        }
        let failures: Vec<(usize, usize)> = read
            .units
            .iter()
            .filter(|unit| unit.kind == Kind::Failure)
            .map(|unit| (unit.start, unit.end))
            .chain(read.named.iter().map(|named| (named.start, named.end)))
            .collect();
        for rendered in &arms {
            for (start, end, carried) in extents(&rendered.content) {
                if carried == Err("not_selected".to_string()) {
                    for (from, to) in &failures {
                        assert!(
                            *to <= start || *from >= end,
                            "the failure at {from}-{to} is inside a not_selected extent {start}-{end}:\n{}",
                            rendered.content
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn only_units_the_rule_does_not_carry_and_that_could_fit_are_offered() {
    // **The offer is exactly what could be added**, both ways: a unit
    // offered and chosen alone is carried, drawn at the widest the model
    // arm can draw; one not offered is either already carried, not
    // something a model chooses, or could not have fitted. The red shape
    // holds the `\n` between two carried cargo errors; the records hold
    // units that fit and units that do not.
    let fixtures = [
        red_shaped(),
        tuned(|pad| records(40, 20, 10, pad), 500..1000),
        tuned(graded, 500..1000),
    ];
    let (mut some_offered, mut some_refused, mut joins) = (false, false, false);
    for text in &fixtures {
        let bytes = text.as_bytes();
        let read = parse(bytes);
        let plan = partition(&read, bytes, SUBJECT, true);
        assert!(plan.split.is_empty(), "the oracle here draws no cut groups");
        assert!(plan.parts.len() <= MAX_PARTS);
        let rule = render(&read, bytes, &choose_by_rule(&read), &plan, SUBJECT);
        let floor = units_in(&read, &carried(&rule.content));
        let offer = offered(&plan);
        for (index, unit) in read.units.iter().enumerate() {
            let is_offered = offer.contains(&index);
            if unit.kind != Kind::Other || floor[index] {
                assert!(
                    !is_offered,
                    "offered a unit the rule carries or a model does not choose: {unit:?}"
                );
                continue;
            }
            if &text[unit.start..unit.end] == "\n"
                && index > 0
                && floor[index - 1]
                && floor.get(index + 1) == Some(&true)
            {
                joins = true;
            }
            let alone = render(
                &read,
                bytes,
                &choose_by_model(&read, &[index]),
                &plan,
                SUBJECT,
            );
            let carried_alone = units_in(&read, &carried(&alone.content))[index];
            if is_offered {
                some_offered = true;
                assert!(
                    carried_alone && widest_fits(&alone.content),
                    "offered {unit:?}, which could not be carried:\n{}",
                    alone.content
                );
            } else {
                some_refused = true;
                assert!(
                    !carried_alone || !widest_fits(&alone.content),
                    "{unit:?} would have fitted and was not offered"
                );
            }
        }
    }
    assert!(
        some_offered && some_refused,
        "a fixture must reach both sides"
    );
    assert!(joins, "no `\\n` between two carried excerpts was checked");
}

/// A failing cargo log whose first failure's block grows by `pad`.
fn padded_log(pad: usize) -> String {
    let failing = [(0, 1), (0, 3), (0, 5), (0, 7), (0, 9), (0, 11)];
    cargo_log(4, 100, &failing, 25).replacen(
        "expected 5 ms\n",
        &format!("expected 5 ms{}\n", "p".repeat(pad)),
        1,
    )
}

#[test]
fn no_question_is_asked_when_the_floor_would_not_fit_under_the_models_header() {
    // **A projection with less slack than the model's label is never put
    // to a model**: the floor is not repacked to make room for a label,
    // so a question whose every answer would have to take something away
    // is not asked. The header says so, with no parts.
    let near = tuned(padded_log, 1..60);
    let bytes = near.as_bytes();
    let read = parse(bytes);
    let plan = partition(&read, bytes, SUBJECT, true);
    assert_eq!(
        next(&read, bytes, &plan, SUBJECT, true),
        Next::Carry(choose_by_rule(&read))
    );
    let rendered = render(&read, bytes, &choose_by_rule(&read), &plan, SUBJECT);
    assert!(
        how(&rendered.content).contains(" 0 parts; "),
        "{}",
        how(&rendered.content)
    );
    assert!(!how(&rendered.content).contains("the model"));

    // The same shape with room is asked.
    let roomy = tuned(padded_log, 1500..4000);
    let read = parse(roomy.as_bytes());
    let plan = partition(&read, roomy.as_bytes(), SUBJECT, true);
    assert_eq!(
        next(&read, roomy.as_bytes(), &plan, SUBJECT, true),
        Next::Ask
    );
    assert!(!plan.asked.is_empty());
}

#[test]
fn with_nothing_that_could_fit_no_model_is_asked_and_the_header_says_so() {
    // Forty failures between passing tests, each its own excerpt: the
    // rule's floor holds every excerpt a projection may, so nothing a model
    // could add would fit, and nobody is asked.
    let failing: Vec<(usize, usize)> = (0..40).map(|test| (0, test * 2 + 1)).collect();
    let log = cargo_log(1, 80, &failing, 20);
    let bytes = log.as_bytes();
    let read = parse(bytes);
    let plan = partition(&read, bytes, SUBJECT, true);
    assert!(plan.parts.len() <= MAX_PARTS && !plan.parts.is_empty());
    assert!(plan.asked.is_empty(), "offered {:?}", plan.asked);
    let Next::Carry(choice) = next(&read, bytes, &plan, SUBJECT, true) else {
        panic!("a model was asked with nothing it could add");
    };
    assert_eq!(choice.by, By::Rule);
    let rendered = render(&read, bytes, &choice, &plan, SUBJECT);
    assert!(
        how(&rendered.content).contains(" 0 parts; "),
        "{}",
        how(&rendered.content)
    );
    assert!(
        how(&rendered.content)
            .ends_with("excerpts chosen by the deterministic rule: failures, then run identity"),
        "{}",
        how(&rendered.content)
    );
}

#[test]
fn the_header_names_exactly_the_excerpts_the_model_added() {
    // The unit a model adds right after a failure's block is its own
    // excerpt, never joined to the rule's, so the header can say which
    // excerpts are the model's and be exactly right: every one it lists is
    // wholly the model's, and none it leaves out holds anything the model
    // added.
    let log = red_shaped();
    let bytes = log.as_bytes();
    let read = parse(bytes);
    let plan = partition(&read, bytes, SUBJECT, true);
    assert_eq!(next(&read, bytes, &plan, SUBJECT, true), Next::Ask);
    let offer = offered(&plan);
    let beside = *offer
        .iter()
        .find(|&&index| index > 0 && read.units[index - 1].kind == Kind::Failure)
        .expect("an offered unit right after a failure");
    let far = *offer.last().expect("offered units");
    for picks in [vec![beside], vec![beside, far]] {
        let rendered = render(
            &read,
            bytes,
            &choose_by_model(&read, &picks),
            &plan,
            SUBJECT,
        );
        let listed = added(&rendered.content).expect("the header names the model");
        let excerpts = numbered(&rendered.content);
        assert_eq!(listed.len(), picks.len(), "{}", how(&rendered.content));
        for (number, &(start, end)) in excerpts.iter().enumerate() {
            let inside: Vec<usize> = (0..read.units.len())
                .filter(|&index| read.units[index].start >= start && read.units[index].end <= end)
                .collect();
            if listed.contains(&(number + 1)) {
                assert!(
                    inside.iter().all(|index| picks.contains(index)),
                    "e{} holds bytes the model did not add",
                    number + 1
                );
            } else {
                assert!(
                    inside.iter().all(|index| !picks.contains(index)),
                    "e{} holds what the model added and is not listed",
                    number + 1
                );
            }
        }
    }
}

#[test]
fn an_empty_answer_is_the_rules_ledger_and_says_the_model_added_nothing() {
    let log = red_shaped();
    let bytes = log.as_bytes();
    let read = parse(bytes);
    let plan = partition(&read, bytes, SUBJECT, true);
    assert_eq!(next(&read, bytes, &plan, SUBJECT, true), Next::Ask);
    let rule = render(&read, bytes, &choose_by_rule(&read), &plan, SUBJECT);
    let nothing = render(&read, bytes, &choose_by_model(&read, &[]), &plan, SUBJECT);
    assert_eq!(extents(&nothing.content), extents(&rule.content));
    assert_eq!(nothing.omitted, rule.omitted);
    assert!(
        how(&nothing.content).ends_with(
            "excerpts chosen by the deterministic rule: failures, then run identity; \
             then chosen by the model, one question per part, which added nothing"
        ),
        "{}",
        how(&nothing.content)
    );
}

#[test]
fn the_longest_model_header_is_computed_and_a_fixture_reaches_it() {
    // **The bound, computed and reached.** The model's part of the label is
    // longest when it lists every excerpt a projection may hold; plain
    // text gives the rule nothing to carry, so a model that adds every
    // other one of its blocks is credited with all of them.
    assert_eq!(ADDED_LABEL_BYTES, widest_suffix().len());
    let text = "xxx\n".repeat(BLOCK_LINES * 210);
    let bytes = text.as_bytes();
    let read = parse(bytes);
    let plan = partition(&read, bytes, SUBJECT, true);
    assert!(plan.parts.len() <= MAX_PARTS);
    assert_eq!(next(&read, bytes, &plan, SUBJECT, true), Next::Ask);
    let picks: Vec<usize> = (0..MAX_EXCERPTS).map(|n| n * 2).collect();
    let rendered = render(
        &read,
        bytes,
        &choose_by_model(&read, &picks),
        &plan,
        SUBJECT,
    );
    assert!(rendered.fits());
    let line = how(&rendered.content);
    let suffix = &line[line
        .find("; then chosen by the model")
        .expect("the model's label")..];
    assert_eq!(suffix, widest_suffix());
    assert_eq!(suffix.len(), ADDED_LABEL_BYTES);
}

#[test]
fn capacity_is_the_inputs_not_the_offers() {
    // **A capacity is a property of the input, not of who reads it.** An
    // input whose units need one part more than a projection may ask is
    // insufficient, even where what a model would be offered of it is
    // within the bound.
    //
    // **And it is decided before anything is offered.** Working out an
    // offer draws the whole projection once for every unit a model could
    // choose, and nothing bounds those units until capacity has been
    // decided: an input of 131,072 bytes of one-line records took a minute
    // of a debug build's time to be refused. So an input over capacity has
    // no questions, although `offer` would have offered this one some.
    let text = tuned(|pad| records(128, 40, 20, pad), 500..1000);
    let bytes = text.as_bytes();
    assert!(bytes.len() <= INPUT_BYTES);
    let read = parse(bytes);
    let plan = partition(&read, bytes, SUBJECT, true);
    assert_eq!(plan.parts.len(), MAX_PARTS + 1, "{}", plan.parts.len());
    let (addable, _) = work_out(&read, bytes, &plan, SUBJECT)
        .1
        .expect("an offer, had one been made");
    let would_offer: Vec<usize> = plan
        .parts
        .iter()
        .flatten()
        .copied()
        .filter(|&index| addable[index])
        .collect();
    assert!(
        !would_offer.is_empty(),
        "the premise: something could be added"
    );
    assert!(plan.asked.is_empty(), "offered {:?}", plan.asked);
    assert_eq!((plan.floor.bytes, plan.floor.excerpts), (0, 0));
    for may_ask in [true, false] {
        assert_eq!(
            next(&read, bytes, &plan, SUBJECT, may_ask),
            Next::Insufficient
        );
    }

    // **Too large is capacity too**, however few parts the rest of it
    // fills: a document whose run identity alone is over the input bound,
    // with a few results a model could otherwise have been offered.
    let document = format!(
        "{{\n  \"environment\": \"{}\",\n  \"results\": [\n{}\n  ]\n}}\n",
        "v".repeat(INPUT_BYTES),
        (0..20)
            .map(|n| format!("    {{\"fixture\": \"core.case-{n}\", \"outcome\": \"pass\"}}"))
            .collect::<Vec<_>>()
            .join(",\n")
    );
    let bytes = document.as_bytes();
    assert!(too_large(bytes.len()));
    let read = parse(bytes);
    assert_eq!(read.format, Format::Json);
    let plan = partition(&read, bytes, SUBJECT, true);
    assert!(plan.parts.len() <= MAX_PARTS, "{}", plan.parts.len());
    let (addable, _) = work_out(&read, bytes, &plan, SUBJECT)
        .1
        .expect("an offer, had one been made");
    assert!(
        plan.parts.iter().flatten().any(|&index| addable[index]),
        "the premise: a result could be added"
    );
    assert!(plan.asked.is_empty(), "offered {:?}", plan.asked);
    for may_ask in [true, false] {
        assert_eq!(
            next(&read, bytes, &plan, SUBJECT, may_ask),
            Next::Insufficient
        );
    }
}

#[test]
fn the_offer_preamble_is_bounded() {
    // **The model is told what is already carried in counts, never in
    // bytes of the document**, so what it is told is bounded by the widest
    // numbers an input within capacity can have.
    assert_eq!(preamble(&widest_floor()).len(), PREAMBLE_BYTES);
    for floor in [
        Floor::default(),
        Floor {
            tests: 18,
            errors: 3,
            bytes: 4321,
            excerpts: 5,
        },
    ] {
        let said = preamble(&floor);
        assert!(said.len() <= PREAMBLE_BYTES, "{said}");
        assert!(
            said.contains(&format!("{} failing tests", floor.tests))
                && said.contains(&format!("{} cargo errors", floor.errors))
                && said.contains(&format!("{} bytes", floor.bytes))
                && said.contains(&format!("{} excerpts", floor.excerpts)),
            "{said:?}"
        );
    }
    let log = red_shaped();
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes(), SUBJECT, true);
    let (candidates, labels) = candidates(&read, log.as_bytes(), SUBJECT.artifact, &plan.asked[0]);
    let part = Part {
        artifact: SUBJECT.artifact.to_string(),
        format: read.format.name(),
        size: read.size,
        number: 1,
        of: plan.asked.len(),
        labels,
        floor: plan.floor.clone(),
    };
    let body = ask("MiniMax-M3", "which tests failed", &part, &candidates);
    let text = &body.messages[0].text;
    let said = preamble(&plan.floor);
    assert!(!said.is_empty());
    assert_eq!(text.matches(&said).count(), 1);
    assert!(text.find(&said) < text.find("Excerpts:\n"));
    assert_eq!(plan.floor.tests, 3);
    assert_eq!(plan.floor.errors, 2);
    let system = body.system.as_deref().expect("an instruction");
    assert!(
        system.contains("empty list is a complete answer"),
        "{system}"
    );
    // **The instruction and the preamble say the same thing** about what
    // is carried: a failure the floor had no room for is not carried, and
    // a model told otherwise is told something false.
    for said in [system, said.as_str()] {
        assert!(
            said.contains("whatever you answer, as far as they fit"),
            "{said}"
        );
    }
}

#[test]
fn a_cargo_error_does_not_swallow_the_run_that_follows_it() {
    // Cargo prints the next `Running` line straight after `error: test
    // failed, to rerun …`, with no blank line between (J2 live, F3). The
    // diagnostic stops there, and the run's identity is its own.
    let log = "running 1 test\ntest a::b ... FAILED\n\ntest result: FAILED. 0 passed; 1 failed\n\n\
               error: test failed, to rerun pass `-p x --test y`\n     \
               Running tests/next.rs (target/debug/deps/next-0123456789abcdef)\n\n\
               running 1 test\ntest c::d ... ok\n\ntest result: ok. 1 passed; 0 failed\n\n";
    let read = parse(log.as_bytes());
    assert_eq!(read.format, Format::Libtest);
    let running = log.find("     Running tests/next.rs").expect("the run");
    let holder = read
        .units
        .iter()
        .find(|unit| unit.start <= running && running < unit.end)
        .expect("tiled");
    assert_eq!(
        holder.kind,
        Kind::Identity,
        "{:?}",
        &log[holder.start..holder.end]
    );
    let error = read
        .units
        .iter()
        .find(|unit| log[unit.start..unit.end].starts_with("error: "))
        .expect("the error");
    assert_eq!(error.kind, Kind::Failure);
    assert!(!log[error.start..error.end].contains("Running"));
}

#[test]
fn the_header_counts_failing_tests_and_cargo_errors_apart() {
    // `failures named: 21` counted cargo's three `error:` lines with
    // eighteen tests (J2 live, F1). The count stays, because a reader
    // checks it; the line before it says what it is made of.
    let libtest = cargo_log(2, 5, &[(0, 1), (1, 3)], 2)
        + "error: test failed, to rerun pass `-p crate_1 --lib`\n";
    let document = manifest(60, &[3, 40]);
    // **A compiler error in cargo's JSON dialect is a cargo error**. It is
    // named by its message where it has one, before what it renders, and
    // by its `reason` where it has neither: a string, so on one line
    // wherever the record is. The `message` object inside it is not a
    // second one.
    let lines = "{\"name\":\"a\",\"outcome\":\"pass\"}\n{\"name\":\"b\",\"outcome\":\"fail\"}\n\
                 {\"reason\":\"compiler-message\",\"message\":{\"level\":\"error\"}}\n\
                 {\"reason\":\"compiler-message\",\"package_id\":\"x 0.1.0\",\"message\":\
                 {\"message\":\"cannot find value `y` in this scope\",\"level\":\"error\",\
                 \"rendered\":\"error[E0425]: cannot find value `y`\"}}\n";
    for (text, tests, errors) in [(libtest, 2, 1), (document, 2, 0), (lines.to_string(), 1, 2)] {
        let read = parse(text.as_bytes());
        let plan = partition(&read, text.as_bytes(), SUBJECT, true);
        let rendered = render(
            &read,
            text.as_bytes(),
            &choose_by_rule(&read),
            &plan,
            SUBJECT,
        );
        let expected = format!(
            "failing tests: {tests}; cargo errors: {errors}\nfailures named: {}\n",
            tests + errors
        );
        assert!(rendered.content.contains(&expected), "{}", rendered.content);
        let named: Vec<&str> = read
            .named
            .iter()
            .map(|named| &text[named.start..named.end])
            .collect();
        if errors == 2 {
            assert!(
                named.contains(&"cannot find value `y` in this scope"),
                "{named:?}"
            );
            assert!(named.contains(&"compiler-message"), "{named:?}");
        }
    }
}

#[test]
fn an_empty_list_at_a_documents_root_is_identity() {
    // `"coverage_limits": []` is not a list of anything; it says the run
    // had none, which is the run's identity (J2 live, F5). A list with
    // members is still cut by them.
    let json = manifest(60, &[3]).replacen("{\n", "{\n  \"coverage_limits\": [],\n", 1);
    let read = parse(json.as_bytes());
    assert_eq!(read.format, Format::Json);
    let at = json.find("\"coverage_limits\"").expect("the member");
    let holder = read
        .units
        .iter()
        .find(|unit| unit.start <= at && at < unit.end)
        .expect("tiled");
    assert_eq!(
        holder.kind,
        Kind::Identity,
        "{:?}",
        &json[holder.start..holder.end]
    );
    let result = json.find("\"core.case-7\"").expect("a result");
    let holder = read
        .units
        .iter()
        .find(|unit| unit.start <= result && result < unit.end)
        .expect("tiled");
    assert_eq!(holder.kind, Kind::Other);
}

// ---- m5a-3's fix round: what the verifiers found unguarded ----------------------

/// A failing cargo log that opens with `groups` blocks of plain lines,
/// **each larger than a part**: sixteen lines of about two kilobytes, each
/// its own unit, and four short ones. After them, seven failures between
/// passing tests, the first failure's block grown by `pad`.
fn cut_log(groups: usize, pad: usize) -> String {
    let mut log = String::from("running 30 tests\n");
    for group in 0..groups {
        for _ in 0..16 {
            log.push_str(&format!("{}\n", "w".repeat(2040)));
        }
        for line in 0..4 {
            log.push_str(&format!("short {group}.{line}\n"));
        }
    }
    let failing: Vec<(usize, usize)> = (0..7).map(|test| (0, test * 2 + 1)).collect();
    log + &cargo_log(1, 30, &failing, 25).replacen(
        "expected 5 ms\n",
        &format!("expected 5 ms{}\n", "p".repeat(pad)),
        1,
    )
}

/// The questions that hold a piece of a planned cut's group, numbered as
/// they are asked.
fn questions_holding(plan: &Plan, cut: Cut) -> Vec<usize> {
    let (first, last, _, _) = cut;
    plan.asked
        .iter()
        .enumerate()
        .filter(|(_, units)| units.iter().any(|&index| index >= first && index <= last))
        .map(|(number, _)| number)
        .collect()
}

#[test]
fn a_group_offered_in_one_question_is_not_reported_as_cut() {
    // **A cut is reported only where a model saw one.** The plan cuts this
    // block across two parts, but its first part's pieces are each too
    // large to add beside the floor, so only its last pieces are offered,
    // in one question. That question saw every piece of the block it was
    // shown, and the model arm says nothing unresolved about it.
    let log = tuned(|pad| cut_log(1, pad), 700..1500);
    let bytes = log.as_bytes();
    let read = parse(bytes);
    let plan = partition(&read, bytes, SUBJECT, true);
    assert_eq!(plan.split.len(), 1, "{:?}", plan.split);
    let (_, _, from, to) = plan.split[0];
    assert!(to > from, "the plan cuts the block: {:?}", plan.split[0]);
    assert_eq!(next(&read, bytes, &plan, SUBJECT, true), Next::Ask);
    assert_eq!(
        questions_holding(&plan, plan.split[0]).len(),
        1,
        "the premise: one question holds the block's pieces: {:?}",
        plan.asked
    );
    for picks in answers(&plan) {
        let rendered = render(
            &read,
            bytes,
            &choose_by_model(&read, &picks),
            &plan,
            SUBJECT,
        );
        assert!(
            !rendered.content.contains("unresolved: "),
            "{picks:?}:\n{}",
            rendered.content
        );
    }
}

/// How far the rule's floor of `text` is from the projection's bytes,
/// drawn at the model arm's widest: `(with every cut group's unresolved
/// line, without them)`.
fn widest_margins(text: &str) -> (i64, i64) {
    let bytes = text.as_bytes();
    let read = parse(bytes);
    let plan = partition(&read, bytes, SUBJECT, true);
    let carried = floor(&read, bytes, &Frame::of(SUBJECT, &plan));
    let model = choose_by_model(&read, &[]);
    let margin = |frame: &Frame<'_>| {
        PROJECTION_BYTES as i64
            - draw(&read, bytes, &model, &carried, Some(&carried), frame)
                .content
                .len() as i64
    };
    let with = Frame::widest(SUBJECT, &plan);
    let without = Frame {
        split: Vec::new(),
        ..Frame::widest(SUBJECT, &plan)
    };
    (margin(&with), margin(&without))
}

#[test]
fn a_floor_with_no_room_for_the_lines_a_cut_group_takes_is_not_put_to_a_model() {
    // **The widest frame counts the model arm's `unresolved:` lines.** Two
    // blocks larger than a part are cut, and a model arm that was offered
    // their pieces says so, a line each. This floor fits beside the model's
    // widest label with room for a passing test, but not beside those two
    // lines as well: a model asked here could be told its section has room
    // it has not. So no model is asked, whatever it would have been
    // offered.
    let mut pad = 0usize;
    let mut found = None;
    for _ in 0..64 {
        let log = cut_log(2, pad);
        let (with, without) = widest_margins(&log);
        if (100..190).contains(&without) {
            assert!(with < 0, "the fixture's premise: {with} with the lines");
            found = Some(log);
            break;
        }
        pad = (pad as i64 + without - 145).max(0) as usize;
    }
    let log = found.expect("a pad that puts the floor between the two margins");
    let bytes = log.as_bytes();
    let read = parse(bytes);
    let plan = partition(&read, bytes, SUBJECT, true);
    assert_eq!(plan.split.len(), 2, "{:?}", plan.split);
    assert!(plan.parts.len() <= MAX_PARTS);
    // What a model would have been offered, had the lines not been counted:
    // a passing test's line, alone, fits beside the floor without them.
    let rule = render(&read, bytes, &choose_by_rule(&read), &plan, SUBJECT);
    let floor = units_in(&read, &carried(&rule.content));
    let without = Frame {
        split: Vec::new(),
        ..Frame::widest(SUBJECT, &plan)
    };
    let model = choose_by_model(&read, &[]);
    assert!(
        (0..read.units.len()).any(|index| {
            let mut with = floor.clone();
            with[index] = true;
            read.units[index].kind == Kind::Other
                && !floor[index]
                && draw(&read, bytes, &model, &with, Some(&floor), &without).fits()
        }),
        "the premise: something would have been offered"
    );
    assert!(plan.asked.is_empty(), "offered {:?}", plan.asked);
    assert_eq!(
        next(&read, bytes, &plan, SUBJECT, true),
        Next::Carry(choose_by_rule(&read))
    );
}

#[test]
fn no_question_is_asked_about_a_floor_over_the_bound_though_an_addition_would_shrink_it() {
    // **The floor must fit beside the model's widest label before anything
    // is offered**, and that check is not implied by the offer's own: a
    // model's unit is drawn as an excerpt of its own, and an excerpt of a
    // one-byte unit is shorter than the omission line it replaces. The
    // blank line between this log's two cargo errors is such a unit: drawn
    // at the widest, the floor is a few bytes over the bound, and the floor
    // with that line carried is under it. So the question would be asked
    // about a floor that does not fit — unless the floor is checked first.
    let mut found = None;
    let mut pad = 0usize;
    for _ in 0..64 {
        let log = padded_log(pad)
            + "error: test failed, to rerun pass `-p crate_0 --lib`\n\n\
               error: 1 target failed:\n    `-p crate_0 --lib`\n";
        let (margin, _) = widest_margins(&log);
        if (-6..0).contains(&margin) {
            found = Some(log);
            break;
        }
        pad = (pad as i64 + margin + 3).max(0) as usize;
    }
    let log = found.expect("a pad that puts the floor a few bytes over the bound");
    let bytes = log.as_bytes();
    let read = parse(bytes);
    let plan = partition(&read, bytes, SUBJECT, true);
    let carried = floor(&read, bytes, &Frame::of(SUBJECT, &plan));
    let gap = read
        .units
        .iter()
        .position(|unit| {
            &log[unit.start..unit.end] == "\n"
                && log[unit.end..].starts_with("error: 1 target failed")
        })
        .expect("the blank line between the errors");
    assert_eq!(read.units[gap].kind, Kind::Other);
    assert!(carried[gap - 1] && carried[gap + 1] && !carried[gap]);
    let mut with = carried.clone();
    with[gap] = true;
    assert!(
        draw(
            &read,
            bytes,
            &choose_by_model(&read, &[]),
            &with,
            Some(&carried),
            &Frame::widest(SUBJECT, &plan)
        )
        .fits(),
        "the premise: carrying the blank line brings the floor under the bound"
    );
    assert!(plan.asked.is_empty(), "offered {:?}", plan.asked);
    assert_eq!(
        next(&read, bytes, &plan, SUBJECT, true),
        Next::Carry(choose_by_rule(&read))
    );
}

#[test]
fn a_model_arm_that_does_not_fit_is_refused_and_never_published() {
    // **Defence in depth.** `offer` asks a model only where the floor fits
    // beside the model arm's widest drawing, so the model arm always fits;
    // and `render` does not take that on trust. Given a plan that asks a
    // model about a floor with less room than the model's label, it
    // refuses, where before it returned a section over the bound.
    let near = tuned(padded_log, 1..60);
    let bytes = near.as_bytes();
    let read = parse(bytes);
    let plan = partition(&read, bytes, SUBJECT, true);
    assert!(plan.asked.is_empty(), "the premise: nothing is asked here");
    let asked_anyway = Plan {
        asked: plan.parts.clone(),
        ..plan.clone()
    };
    let refused = super::render(
        &read,
        bytes,
        &choose_by_model(&read, &[]),
        &asked_anyway,
        SUBJECT,
    );
    assert!(
        refused.is_none(),
        "published a model arm of {} bytes",
        refused.map_or(0, |rendered| rendered.content.len())
    );
    // The rule's projection of the same log is published, and one a model
    // was properly asked about is too.
    assert!(super::render(&read, bytes, &choose_by_rule(&read), &plan, SUBJECT).is_some());
    let log = red_shaped();
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes(), SUBJECT, true);
    assert_eq!(next(&read, log.as_bytes(), &plan, SUBJECT, true), Next::Ask);
    assert!(
        super::render(
            &read,
            log.as_bytes(),
            &choose_by_model(&read, &offered(&plan)),
            &plan,
            SUBJECT
        )
        .is_some()
    );
}

/// A failing cargo log whose twenty-four failures fill every excerpt a
/// projection holds **until the run identity between them joins them into
/// one**: while the floor is full it refuses the run's first identity and
/// a twenty-fifth failure, and it is then left with room for both. One
/// passing test sits between the last two failures.
fn bridged() -> String {
    let mut log = String::from(
        "     Running unittests src/lib.rs (target/debug/deps/crate_0-0123456789abcdef)\n\n\
         test tests::first ... ok\n",
    );
    for n in 0..24 {
        log.push_str(&format!("test tests::failing_{n} ... FAILED\n"));
        if n < 23 {
            log.push_str("running 1 test\n");
        }
    }
    log.push_str("test tests::between ... ok\ntest tests::failing_24 ... FAILED\n");
    for n in 0..700 {
        log.push_str(&format!("test tests::passing_{n} ... ok\n"));
    }
    log
}

/// The unit holding the first byte of `needle` in `text`.
fn unit_holding(read: &Read, text: &str, needle: &str) -> usize {
    let at = text.find(needle).expect("the needle");
    read.units
        .iter()
        .position(|unit| unit.start <= at && at < unit.end)
        .expect("tiled")
}

#[test]
fn run_identity_the_floor_left_out_is_never_offered_though_it_would_fit() {
    // **Run identity is the rule's to carry, never a model's to add**, and
    // that holds where the floor left some out that would now fit: its
    // first identity was refused while the failures held every excerpt,
    // and the identity after it then joined them into one.
    let log = bridged();
    let bytes = log.as_bytes();
    let read = parse(bytes);
    let plan = partition(&read, bytes, SUBJECT, true);
    assert_eq!(next(&read, bytes, &plan, SUBJECT, true), Next::Ask);
    let rule = render(&read, bytes, &choose_by_rule(&read), &plan, SUBJECT);
    let floor = units_in(&read, &carried(&rule.content));
    let top = unit_holding(&read, &log, "     Running unittests");
    assert_eq!(read.units[top].kind, Kind::Identity);
    assert!(
        !floor[top],
        "the premise: the floor left the run's identity out"
    );
    let mut with = floor.clone();
    with[top] = true;
    assert!(
        draw(
            &read,
            bytes,
            &choose_by_model(&read, &[]),
            &with,
            Some(&floor),
            &Frame::widest(SUBJECT, &plan)
        )
        .fits(),
        "the premise: it would fit beside the floor"
    );
    let offer = offered(&plan);
    assert!(!offer.contains(&top), "run identity was offered to a model");
    assert!(
        offer
            .iter()
            .all(|&index| read.units[index].kind == Kind::Other),
        "{offer:?}"
    );
}

#[test]
fn a_failure_the_floor_left_out_is_never_credited_to_the_model() {
    // **What a model adds is the model's, and nothing else is.** The floor
    // refused this log's last failure while it was full; a model that adds
    // the passing test beside it must not bring the failure in with it,
    // joined to its excerpt and listed as the model's.
    let log = bridged();
    let bytes = log.as_bytes();
    let read = parse(bytes);
    let plan = partition(&read, bytes, SUBJECT, true);
    assert_eq!(next(&read, bytes, &plan, SUBJECT, true), Next::Ask);
    let rule = render(&read, bytes, &choose_by_rule(&read), &plan, SUBJECT);
    let floor = units_in(&read, &carried(&rule.content));
    let last = unit_holding(&read, &log, "test tests::failing_24 ");
    assert_eq!(read.units[last].kind, Kind::Failure);
    assert!(!floor[last], "the premise: the floor left the failure out");
    let between = unit_holding(&read, &log, "test tests::between ");
    assert!(offered(&plan).contains(&between), "{:?}", plan.asked);
    for picks in answers(&plan) {
        let rendered = render(
            &read,
            bytes,
            &choose_by_model(&read, &picks),
            &plan,
            SUBJECT,
        );
        let excerpts = numbered(&rendered.content);
        let listed = added(&rendered.content).expect("the header names the model");
        for number in listed {
            let (start, end) = excerpts[number - 1];
            assert!(
                read.units
                    .iter()
                    .filter(|unit| unit.start >= start && unit.end <= end)
                    .all(|unit| unit.kind == Kind::Other),
                "{picks:?}: e{number} is listed as the model's and holds a failure:\n{}",
                rendered.content
            );
        }
        assert!(
            !units_in(&read, &carried(&rendered.content))[last],
            "{picks:?}: a failure the floor left out was carried after the model's picks"
        );
    }
}

// ---- what the family can cost ---------------------------------------------------------------

/// The largest request a part can send, as admission counts it.
fn worst_case(body: &Request) -> u64 {
    let serialized = body.serialize(Dialect::Responses);
    crate::budget::input_bound(&serialized, body.framed_messages(Dialect::Responses))
        .saturating_add(body.generation)
        .saturating_add(crate::budget::SAFETY_MARGIN_TOKENS)
}

/// The widest counts a part's preamble can carry: a named failure is at
/// least one byte of an input within capacity, and the room is at most a
/// whole projection.
fn widest_floor() -> Floor {
    Floor {
        tests: INPUT_BYTES,
        errors: INPUT_BYTES,
        bytes: PROJECTION_BYTES,
        excerpts: MAX_EXCERPTS,
    }
}

/// A part at its bounds: [`PART_UNITS`] candidates whose frames and texts,
/// escaped, come to [`PART_BYTES`] between them, with the widest ids,
/// labels, numbers, header and preamble a part can have.
fn at_the_bound() -> (Part, Vec<Candidate>) {
    let label = format!("record {}", "l".repeat(LABEL_BYTES));
    let wide = 999_999;
    let frame = |id: &str, text: &str| {
        body_bytes(&format!(
            "\n[{id}] lines {wide}-{wide}, {label}\n{text}\n[end {id}]\n"
        ))
    };
    let each = PART_BYTES / PART_UNITS;
    let candidates: Vec<Candidate> = (1..=PART_UNITS)
        .map(|n| {
            let id = format!("u{n}");
            let text = "x".repeat(each - frame(&id, ""));
            Candidate {
                id,
                kind: KIND_UNIT,
                path: format!("{}@{wide}-{wide}", "a".repeat(128)),
                start_line: wide,
                end_line: wide,
                text,
            }
        })
        .collect();
    let used: usize = candidates
        .iter()
        .map(|candidate| frame(&candidate.id, &candidate.text))
        .sum();
    assert!(
        used <= PART_BYTES && used > PART_BYTES - PART_UNITS,
        "{used}"
    );
    let part = Part {
        artifact: "a".repeat(128),
        format: Format::JsonLines.name(),
        size: INPUT_BYTES,
        number: MAX_PARTS,
        of: MAX_PARTS,
        labels: vec![label; PART_UNITS],
        floor: widest_floor(),
    };
    (part, candidates)
}

/// The figures READINESS publishes, so that a document cannot drift from
/// the arithmetic it quotes.
const PUBLISHED_PART: u64 = 41_017;
const PUBLISHED_PROJECTION: u64 = 492_204;

#[test]
fn a_parts_worst_call_is_computed_from_its_real_body_and_fits_one_request() {
    let (part, candidates) = at_the_bound();
    // A task larger than the protocol admits (256 characters, each at
    // most six bytes escaped), as discovery's arithmetic takes it.
    let task = "x".repeat(4096);
    let worst = worst_case(&ask("MiniMax-M3", &task, &part, &candidates));
    assert_eq!(
        worst, PUBLISHED_PART,
        "READINESS's figure for one part is not what it costs"
    );
    assert!(
        worst < crate::budget::PER_REQUEST_TOKENS,
        "a part can be refused by its own request ceiling: {worst}"
    );
}

#[test]
fn a_whole_projection_cannot_exhaust_a_job_even_beside_discovery() {
    // **Every part may be counted once and repaired once**, so three
    // sends apiece, as discovery's arithmetic has it; and a request whose
    // limit covers both a projection and discovery's flow must not be
    // refused by its own job's ceiling.
    let (part, candidates) = at_the_bound();
    let task = "x".repeat(4096);
    let sends = 2 + u64::from(crate::model::REPAIRS);
    let projection =
        worst_case(&ask("MiniMax-M3", &task, &part, &candidates)) * sends * MAX_PARTS as u64;
    assert_eq!(projection, PUBLISHED_PROJECTION);
    assert!(projection < crate::budget::PER_JOB_TOKENS, "{projection}");
    assert!(
        projection + DISCOVERY_FLOW < crate::budget::PER_JOB_TOKENS,
        "a projection beside discovery can exhaust a job: {}",
        projection + DISCOVERY_FLOW
    );
}

/// Discovery's own published flow, read from where it is computed.
const DISCOVERY_FLOW: u64 = crate::discovery::tests::PUBLISHED_FLOW;

#[test]
fn the_harness_stops_against_the_same_worst_case_this_module_computes() {
    // **A number that lives in two languages has a test across the
    // boundary**: the J2 harness checks a run's worst case before it
    // starts one, and reads it from here.
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    let harness = path.join("scripts").join("j2_run.py");
    let source = std::fs::read_to_string(&harness)
        .unwrap_or_else(|_| panic!("{} is not where the harness lives", harness.display()));
    let figure = |name: &str| -> Option<u64> {
        let at = source.find(name)?;
        let written: String = source[at + name.len()..]
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '_')
            .filter(|c| *c != '_')
            .collect();
        written.parse().ok()
    };
    assert_eq!(
        figure("WORST_CASE_PROJECTION_TOKENS = "),
        Some(PUBLISHED_PROJECTION),
        "the harness stops against a worst case this module does not compute"
    );
    assert_eq!(figure("MAX_PARTS = "), Some(MAX_PARTS as u64));
}

// ---- what planning and rendering draw ----------------------------------------------

/// What `work` returns, and how many projections it drew on the way.
///
/// **The counter is seen to count before it is read.** It is reset, one
/// projection is drawn, and it must then read exactly one, so a counter
/// that was never reset or never counts fails here rather than passing
/// every `drawn == 0` and `drawn <= bound` it is used for.
fn drawing<T>(work: impl FnOnce() -> T) -> (T, usize) {
    let bytes = b"{\"n\":0}\n";
    let read = parse(bytes);
    let plan = partition(&read, bytes, SUBJECT, false);
    DRAWS.with(|draws| draws.set(0));
    draw(
        &read,
        bytes,
        &choose_by_rule(&read),
        &all(&read),
        None,
        &Frame::of(SUBJECT, &plan),
    );
    assert_eq!(
        DRAWS.with(|draws| draws.get()),
        1,
        "the counter, reset, did not count the one projection drawn"
    );
    let value = work();
    (value, DRAWS.with(|draws| draws.get()) - 1)
}

#[test]
fn the_draw_counter_counts_exactly_the_projections_drawn() {
    // **A counter that never counts passes every bound it is read for**:
    // each is `drawn == 0` or `drawn <= bound`. So what it counts is held
    // exactly here, on each path that draws — `draw` itself, the rule's
    // floor, and `render` on a plan that carries its floor — against a
    // count worked out from the units.
    let text = cargo_log(3, 4, &[(0, 1), (2, 3)], 2);
    let bytes = text.as_bytes();
    let read = parse(bytes);
    let floored = read
        .units
        .iter()
        .filter(|unit| unit.kind != Kind::Other)
        .count();
    let others: Vec<usize> = (0..read.units.len())
        .filter(|&at| read.units[at].kind == Kind::Other)
        .collect();
    assert!(
        floored >= 4 && others.len() >= 2,
        "the premise: {floored} units for the floor, {} others",
        others.len()
    );
    let plan = partition(&read, bytes, SUBJECT, false);
    let frame = Frame::of(SUBJECT, &plan);
    let (_, drawn) = drawing(|| {
        draw(
            &read,
            bytes,
            &choose_by_rule(&read),
            &all(&read),
            None,
            &frame,
        )
    });
    assert_eq!(drawn, 1, "one projection drawn");
    let (rule, drawn) = drawing(|| floor(&read, bytes, &frame));
    assert_eq!(
        drawn, floored,
        "the floor draws once for each unit it tries"
    );
    let carrying = Plan {
        rule: Some(rule),
        ..plan.clone()
    };
    let (_, drawn) = drawing(|| render(&read, bytes, &choose_by_rule(&read), &carrying, SUBJECT));
    assert_eq!(drawn, 1, "the rule's arm on a plan that carries its floor");
    let (_, drawn) = drawing(|| {
        render(
            &read,
            bytes,
            &choose_by_model(&read, &others),
            &carrying,
            SUBJECT,
        )
    });
    assert_eq!(
        drawn,
        others.len() + 1,
        "the model arm draws once for each unit it adds, then once more"
    );
    let (_, drawn) = drawing(|| {
        render(
            &read,
            bytes,
            &Choice {
                by: By::NotText,
                chosen: vec![false; read.units.len()],
            },
            &plan,
            SUBJECT,
        )
    });
    assert_eq!(drawn, 1, "bytes that are not text");
}

/// A JSON document whose run identity is an array of `identity` zeros,
/// **a unit of about two bytes each**, which the rule's floor is drawn
/// once for apiece, beside one failing result and `passing` passing ones
/// of about 1.9 kilobytes a model could be offered.
fn identity_heavy(identity: usize, passing: usize) -> String {
    let results: Vec<String> = (0..passing)
        .map(|n| {
            format!(
                "    {{\"fixture\": \"core.case-{n}\", \"outcome\": \"pass\", \"detail\": \"{}\"}}",
                "b".repeat(1900)
            )
        })
        .chain(std::iter::once(
            "    {\"fixture\": \"core.broken\", \"outcome\": \"fail\"}".to_string(),
        ))
        .collect();
    format!(
        "{{\n  \"environment\": {{\"x\": [{}]}},\n  \"results\": [\n{}\n  ]\n}}\n",
        vec!["0"; identity].join(","),
        results.join(",\n")
    )
}

/// The two inputs `capacity_is_the_inputs_not_the_offers` is over
/// capacity with, and bytes that are not text.
fn over_capacity() -> Vec<Vec<u8>> {
    let five_parts = tuned(|pad| records(128, 40, 20, pad), 500..1000);
    let too_large = format!(
        "{{\n  \"environment\": \"{}\",\n  \"results\": [\n{}\n  ]\n}}\n",
        "v".repeat(INPUT_BYTES),
        (0..20)
            .map(|n| format!("    {{\"fixture\": \"core.case-{n}\", \"outcome\": \"pass\"}}"))
            .collect::<Vec<_>>()
            .join(",\n")
    );
    let binary: Vec<u8> = (0..4096u32).map(|n| (n % 251) as u8).collect();
    vec![five_parts.into_bytes(), too_large.into_bytes(), binary]
}

#[test]
fn nothing_is_drawn_to_plan_an_input_over_capacity() {
    // **Capacity is decided before anything is drawn**, and what is
    // counted is the drawing: an offer worked out and then thrown away
    // changes no output, and costs what the capacity check exists to
    // save — a minute of a debug build's time for 131,072 bytes of
    // one-line records.
    for bytes in over_capacity() {
        let read = parse(&bytes);
        for may_ask in [true, false] {
            let (plan, drawn) = drawing(|| partition(&read, &bytes, SUBJECT, may_ask));
            assert_eq!(
                drawn,
                0,
                "{drawn} projections drawn to plan {} bytes read as {:?}, may_ask {may_ask}",
                bytes.len(),
                read.format
            );
            assert!(plan.asked.is_empty(), "offered {:?}", plan.asked);
        }
    }
}

#[test]
fn a_request_that_may_not_ask_a_model_draws_nothing_to_plan() {
    // **Nothing is offered where nobody may be asked.** Working out an
    // offer draws the rule's floor, once for every failure and every unit
    // of run identity, and `render` draws it again: a baseline request
    // over 131,072 bytes of identity paid about 22 seconds of a debug
    // build's time for each. Each input here would be offered something
    // were a model allowed.
    for text in [
        red_shaped(),
        identity_heavy(3000, 20),
        tuned(padded_log, 1500..4000),
    ] {
        let bytes = text.as_bytes();
        let read = parse(bytes);
        let (plan, drawn) = drawing(|| partition(&read, bytes, SUBJECT, false));
        assert_eq!(drawn, 0, "{drawn} projections drawn to plan for the rule");
        assert!(plan.asked.is_empty(), "offered {:?}", plan.asked);
        assert!(
            !partition(&read, bytes, SUBJECT, true).asked.is_empty(),
            "the premise: a model that may be asked is offered something"
        );
    }
}

#[test]
fn both_arms_print_the_same_part_count_whether_or_not_a_model_may_be_asked() {
    // **Gate F**: the baseline, which may ask nobody, and the assisted
    // arm print the same number of parts, and it is the questions the
    // assisted arm asks. The harness reads it from the baseline to decide
    // what the assisted request may spend, so a baseline that planned
    // nothing must still count the questions — from the floor it draws
    // anyway — and print exactly the rule's projection a model-allowed
    // request prints when no question is needed.
    let forty: Vec<(usize, usize)> = (0..40).map(|test| (0, test * 2 + 1)).collect();
    let fixtures = [
        red_shaped(),
        identity_heavy(3000, 20),
        tuned(padded_log, 1..60),
        tuned(padded_log, 1500..4000),
        tuned(graded, 900..1100),
        cargo_log(1, 80, &forty, 20),
        cargo_log(1, 3, &[(0, 1)], 2),
    ];
    let mut asked = 0;
    for text in fixtures {
        let bytes = text.as_bytes();
        let read = parse(bytes);
        let baseline_plan = partition(&read, bytes, SUBJECT, false);
        let assisted_plan = partition(&read, bytes, SUBJECT, true);
        let Next::Carry(rule) = next(&read, bytes, &baseline_plan, SUBJECT, false) else {
            panic!("a request that may ask nobody was not carried");
        };
        let baseline = render(&read, bytes, &rule, &baseline_plan, SUBJECT);
        let parts = assisted_plan.asked.len();
        let count = format!(" in {parts} parts;");
        assert!(
            how(&baseline.content).contains(&count),
            "the baseline says {:?}; the assisted arm asks {parts}",
            how(&baseline.content)
        );
        match next(&read, bytes, &assisted_plan, SUBJECT, true) {
            Next::Ask => {
                asked += 1;
                let assisted = render(
                    &read,
                    bytes,
                    &choose_by_model(&read, &offered(&assisted_plan)),
                    &assisted_plan,
                    SUBJECT,
                );
                assert!(
                    how(&assisted.content).contains(&count),
                    "{}",
                    how(&assisted.content)
                );
            }
            Next::Carry(choice) => {
                assert_eq!(choice, rule);
                assert_eq!(
                    render(&read, bytes, &choice, &assisted_plan, SUBJECT),
                    baseline,
                    "a projection no question was needed for differs by who may be asked"
                );
            }
            Next::Insufficient => panic!("the fixture is over the projection's capacity"),
        }
    }
    assert!(asked >= 3, "the premise: most fixtures ask; {asked} did");
}

#[test]
fn a_request_draws_the_rules_floor_once() {
    // **The floor is drawn once a request**, whichever arm: `partition`
    // keeps the floor it worked out an offer from, and `render` draws
    // one only when it was given none. The floor is one drawing for each
    // failure and each unit of run identity, so drawn twice it is the
    // pre-existing quadratic cost paid twice.
    let text = identity_heavy(3000, 20);
    let bytes = text.as_bytes();
    let read = parse(bytes);
    let floored = read
        .units
        .iter()
        .filter(|unit| unit.kind != Kind::Other)
        .count();
    let others = read.units.len() - floored;
    assert!(
        floored > 4 * (2 * others + 8),
        "the premise: the floor is most of the drawing"
    );
    let (baseline, drawn) = drawing(|| {
        let plan = partition(&read, bytes, SUBJECT, false);
        let Next::Carry(rule) = next(&read, bytes, &plan, SUBJECT, false) else {
            panic!("a request that may ask nobody was not carried");
        };
        render(&read, bytes, &rule, &plan, SUBJECT)
    });
    assert!(
        drawn <= floored + others + 8,
        "the baseline drew {drawn}; the floor is {floored}, and {others} could be offered"
    );
    assert!(!baseline.content.is_empty());
    let (_, drawn) = drawing(|| {
        let plan = partition(&read, bytes, SUBJECT, true);
        assert_eq!(next(&read, bytes, &plan, SUBJECT, true), Next::Ask);
        render(
            &read,
            bytes,
            &choose_by_model(&read, &offered(&plan)),
            &plan,
            SUBJECT,
        )
    });
    assert!(
        drawn <= floored + 2 * others + 8,
        "the assisted arm drew {drawn}; the floor is {floored}, and {others} could be offered"
    );
}

#[test]
fn a_model_arm_over_the_excerpts_a_projection_holds_is_refused() {
    // **The refusal holds on excerpts as on bytes.** `render` draws the
    // model arm on the floor its plan carries, and does not take that
    // floor on trust: handed one of every other record, thirty excerpts
    // in a few kilobytes, it refuses what it would draw.
    let text: String = (0..60).map(|n| format!("{{\"n\":{n}}}\n")).collect();
    let bytes = text.as_bytes();
    let read = parse(bytes);
    assert_eq!(read.units.len(), 60);
    let plan = partition(&read, bytes, SUBJECT, true);
    let alternate: Vec<bool> = (0..read.units.len()).map(|at| at % 2 == 0).collect();
    let handed = Plan {
        asked: plan.parts.clone(),
        rule: Some(alternate.clone()),
        ..plan.clone()
    };
    let model = choose_by_model(&read, &[]);
    let drawn = draw(
        &read,
        bytes,
        &model,
        &alternate,
        Some(&alternate),
        &Frame::of(SUBJECT, &handed),
    );
    assert!(
        drawn.content.len() <= PROJECTION_BYTES && drawn.excerpts > MAX_EXCERPTS,
        "the premise: {} bytes, {} excerpts",
        drawn.content.len(),
        drawn.excerpts
    );
    let refused = super::render(&read, bytes, &model, &handed, SUBJECT);
    assert!(
        refused.is_none(),
        "published a model arm of {} excerpts",
        refused.map_or(0, |rendered| rendered.excerpts)
    );
}

#[test]
fn a_model_arm_of_exactly_the_excerpts_a_projection_holds_is_published_and_one_more_is_not() {
    // **The refusal's bound is the bound itself**: handed a floor of
    // exactly `MAX_EXCERPTS` separate records, `render` publishes the model
    // arm; handed one more, it refuses it. The test above hands thirty,
    // which a bound one too loose refuses as well.
    let text: String = (0..60).map(|n| format!("{{\"n\":{n}}}\n")).collect();
    let bytes = text.as_bytes();
    let read = parse(bytes);
    assert_eq!(read.units.len(), 60);
    let plan = partition(&read, bytes, SUBJECT, true);
    let model = choose_by_model(&read, &[]);
    for records in [MAX_EXCERPTS, MAX_EXCERPTS + 1] {
        let handed_floor: Vec<bool> = (0..read.units.len())
            .map(|at| at % 2 == 0 && at < 2 * records)
            .collect();
        let handed = Plan {
            asked: plan.parts.clone(),
            rule: Some(handed_floor),
            ..plan.clone()
        };
        let published = super::render(&read, bytes, &model, &handed, SUBJECT);
        if records == MAX_EXCERPTS {
            let published = published.expect("a model arm of exactly the bound was refused");
            assert_eq!(published.excerpts, MAX_EXCERPTS);
            assert!(published.content.len() <= PROJECTION_BYTES);
        } else {
            // The premise: one more record is over on excerpts and not on
            // bytes, so the refusal below is the excerpt bound's.
            let drawn = draw(
                &read,
                bytes,
                &model,
                handed.rule.as_deref().expect("the handed floor"),
                handed.rule.as_deref(),
                &Frame::of(SUBJECT, &handed),
            );
            assert!(
                drawn.content.len() <= PROJECTION_BYTES && drawn.excerpts == MAX_EXCERPTS + 1,
                "the premise: {} bytes, {} excerpts",
                drawn.content.len(),
                drawn.excerpts
            );
            assert!(
                published.is_none(),
                "published a model arm of {} excerpts",
                published.map_or(0, |rendered| rendered.excerpts)
            );
        }
    }
}

#[test]
fn a_compiler_error_whose_message_is_one_byte_is_named_by_it() {
    // One byte is a name, not an empty string: the error is named by its
    // message, not by what it renders.
    let lines = "{\"reason\":\"compiler-message\",\"message\":{\"message\":\"x\",\"level\":\"error\",\
                 \"rendered\":\"error[E0425]: cannot find value `x`\"}}\n\
                 {\"reason\":\"build-finished\",\"success\":false}\n";
    let bytes = lines.as_bytes();
    let read = parse(bytes);
    assert_eq!(read.format, Format::JsonLines);
    let named: Vec<(&str, Failing)> = read
        .named
        .iter()
        .map(|named| (&lines[named.start..named.end], named.kind))
        .collect();
    assert_eq!(named, [("x", Failing::Error)]);
}

#[test]
fn a_compiler_error_whose_message_is_empty_is_named_by_what_it_renders() {
    // An empty string names nothing: the header would list `  at bytes
    // N-N`. The error is named by what it renders, and by its `reason`
    // where that is empty too.
    let lines = "{\"reason\":\"compiler-message\",\"message\":{\"message\":\"\",\"level\":\"error\",\
                 \"rendered\":\"error[E0425]: cannot find value `y`\"}}\n\
                 {\"reason\":\"compiler-message\",\"message\":{\"message\":\"\",\"level\":\"error\",\
                 \"rendered\":\"\"}}\n";
    let bytes = lines.as_bytes();
    let read = parse(bytes);
    assert_eq!(read.format, Format::JsonLines);
    let named: Vec<(&str, Failing)> = read
        .named
        .iter()
        .map(|named| (&lines[named.start..named.end], named.kind))
        .collect();
    assert_eq!(
        named,
        [
            ("error[E0425]: cannot find value `y`", Failing::Error),
            ("compiler-message", Failing::Error)
        ]
    );
    let plan = partition(&read, bytes, SUBJECT, true);
    let rendered = render(&read, bytes, &choose_by_rule(&read), &plan, SUBJECT);
    assert!(
        !rendered.content.contains("\n   at bytes"),
        "{}",
        rendered.content
    );
}

/// A JSON document of exactly [`INPUT_BYTES`]: thirty passing results
/// with a note of `7 × shift` bytes each, and a run identity string
/// making up the rest, which moves where that identity is cut into units
/// and so how much room the rule's floor leaves.
fn exactly_input_bytes(shift: usize) -> String {
    let results: Vec<String> = (0..30)
        .map(|n| {
            format!(
                "    {{\"fixture\": \"core.case-{n}\", \"outcome\": \"pass\", \"note\": \"{}\"}}",
                "n".repeat(shift * 7)
            )
        })
        .collect();
    let rest = format!(
        "{{\n  \"environment\": \"\",\n  \"results\": [\n{}\n  ]\n}}\n",
        results.join(",\n")
    );
    rest.replacen(
        "\"environment\": \"\"",
        &format!(
            "\"environment\": \"{}\"",
            "v".repeat(INPUT_BYTES - rest.len())
        ),
        1,
    )
}

#[test]
fn an_input_of_exactly_the_input_bound_is_within_capacity_and_asked() {
    // **Too large is past the bound, not at it**, and `partition` decides
    // it as `next` and the call site do: an artifact of exactly 131,072
    // bytes within the parts is offered to a model like any other. Run
    // identity is never cut into parts, so it is what makes up the bytes.
    let text = (0..80)
        .map(exactly_input_bytes)
        .find(|text| {
            let read = parse(text.as_bytes());
            !partition(&read, text.as_bytes(), SUBJECT, true)
                .asked
                .is_empty()
        })
        .expect("a shift that leaves the floor room for a result");
    let bytes = text.as_bytes();
    assert_eq!(bytes.len(), INPUT_BYTES);
    assert!(!too_large(bytes.len()));
    let read = parse(bytes);
    let plan = partition(&read, bytes, SUBJECT, true);
    assert!(plan.parts.len() <= MAX_PARTS, "{}", plan.parts.len());
    assert_eq!(next(&read, bytes, &plan, SUBJECT, true), Next::Ask);
}

#[test]
fn bytes_over_the_input_bound_are_insufficient_before_they_are_not_text() {
    // **`next` decides in the order it documents**: capacity first, then
    // whether the bytes are text. The call site refuses a too-large
    // artifact before reading it, so only here is a binary over the bound
    // seen by `next`, which must call it over capacity and never carry
    // it as omitted whole.
    let binary: Vec<u8> = (0..=INPUT_BYTES as u32).map(|n| (n % 251) as u8).collect();
    assert!(too_large(binary.len()));
    let read = parse(&binary);
    assert_eq!(read.format, Format::Binary);
    for may_ask in [true, false] {
        let plan = partition(&read, &binary, SUBJECT, may_ask);
        assert_eq!(
            next(&read, &binary, &plan, SUBJECT, may_ask),
            Next::Insufficient
        );
    }
}
