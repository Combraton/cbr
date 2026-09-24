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
    let finished = "{\"reason\":\"build-finished\",\"success\":true}\n";
    let read_ok = parse(format!("{{\"reason\":\"x\"}}\n{finished}").as_bytes());
    assert_eq!(
        read_ok.units[1].kind,
        Kind::Identity,
        "a finished build is the run's identity"
    );
    assert!(read.named.is_empty(), "nothing here says what it is called");
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
    let plan = partition(&read_plain, log.as_bytes());
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
    let noisy_plan = partition(&read_noisy, noisy.as_bytes());
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
    let plan = partition(&read, records.as_bytes());
    let sizes: Vec<usize> = plan.parts.iter().map(Vec::len).collect();
    assert_eq!(sizes, [PART_UNITS, 1]);
}

#[test]
fn identity_is_never_offered_to_a_model() {
    let log = cargo_log(2, 3, &[(0, 1)], 2);
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes());
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
    // partitioning has to cut, and the projection says so.
    let log = cargo_log(1, 2, &[(0, 0)], 1400);
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes());
    assert!(plan.parts.len() >= 3, "{}", plan.parts.len());
    assert_eq!(plan.split.len(), 1, "{:?}", plan.split);
    let (first, last, from, to) = plan.split[0];
    assert_eq!(read.units[first].group, read.units[last].group);
    assert!(to > from, "a cut group spans parts: {:?}", plan.split[0]);
    for (part, units) in plan.parts.iter().enumerate() {
        let holds = units.iter().any(|&index| index >= first && index <= last);
        assert_eq!(holds, part >= from && part <= to, "part {part}");
    }
    let chosen = choose_by_model(&read, &[first]);
    let rendered = render(&read, log.as_bytes(), &chosen, &plan, SUBJECT);
    assert!(
        rendered.content.contains(&format!(
            "one block cut across parts {}-{}",
            from + 1,
            to + 1
        )),
        "{}",
        rendered.content
    );
    // The deterministic rule showed no part to anybody, so it has nothing
    // unresolved to say.
    let ruled = render(
        &read,
        log.as_bytes(),
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
            if partition(&read, text.as_bytes()).parts.len() > parts {
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
    let plan_at = partition(&read_at, at_bound.as_bytes());
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
    let plan_over = partition(&read_over, over.as_bytes());
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
    let plan = partition(&read, log.as_bytes());
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
    let plan = partition(&read, log.as_bytes());
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
    let plan = partition(&read, log.as_bytes());
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
    let plan = partition(&read, log.as_bytes());
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
fn bytes_that_are_not_text_are_one_declared_omission() {
    let bytes = vec![0x80_u8; 5000];
    let read = parse(&bytes);
    let plan = partition(&read, &bytes);
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
    let plan = partition(&read, log.as_bytes());
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
    let plan = partition(&read, log.as_bytes());
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
    let plan = partition(&read, log.as_bytes());
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
    let plan = partition(&read, log.as_bytes());
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
    let plan = partition(&read, log.as_bytes());
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
    let plan = partition(&read, log.as_bytes());
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
    };
    let nothing = Choice {
        by: By::Model,
        chosen: vec![false; read.units.len()],
    };
    let carried = vec![false; read.units.len()];
    let header = draw(
        &read,
        log.as_bytes(),
        &nothing,
        &carried,
        &Frame::of(subject, &plan),
    );
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
    "sha256:861831dbd4d8144c75c7b822c25ed516a69b7fdf456a10d931202b84ae1c5a44";

#[test]
fn the_projection_a_fixed_fixture_renders_has_not_changed_without_its_format() {
    let failing: Vec<(usize, usize)> = (0..6).map(|test| (1, test * 7)).collect();
    let log = cargo_log(3, 45, &failing, 40);
    let read = parse(log.as_bytes());
    let plan = partition(&read, log.as_bytes());
    let anchors = [("git_tree".to_string(), "t".repeat(40))];
    let subject = Subject {
        capture: &anchors,
        ..SUBJECT
    };
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
    let plan = partition(&read, log.as_bytes());
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
    let plan = partition(&read, log.as_bytes());
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
    let plan = partition(&read, log.as_bytes());
    let (candidates, labels) = candidates(&read, log.as_bytes(), SUBJECT.artifact, &plan.parts[0]);
    let part = Part {
        artifact: SUBJECT.artifact.to_string(),
        format: read.format.name(),
        size: read.size,
        number: 1,
        of: 1,
        labels,
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

// ---- what the family can cost ---------------------------------------------------------------

/// The largest request a part can send, as admission counts it.
fn worst_case(body: &Request) -> u64 {
    let serialized = body.serialize(Dialect::Responses);
    crate::budget::input_bound(&serialized, body.framed_messages(Dialect::Responses))
        .saturating_add(body.generation)
        .saturating_add(crate::budget::SAFETY_MARGIN_TOKENS)
}

/// A part at its bounds: [`PART_UNITS`] candidates whose frames and texts,
/// escaped, come to [`PART_BYTES`] between them, with the widest ids,
/// labels, numbers and header a part can have.
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
    };
    (part, candidates)
}

/// The figures READINESS publishes, so that a document cannot drift from
/// the arithmetic it quotes.
const PUBLISHED_PART: u64 = 40_680;
const PUBLISHED_PROJECTION: u64 = 488_160;

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
