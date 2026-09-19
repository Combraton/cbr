//! The dependency evaluator's acceptance criterion (STACK section 7).
//!
//! A hand-rolled evaluator's worst failure is silent: it reports "unchanged"
//! for something that changed, and serves a stale answer with no error. No
//! example-based test finds that reliably, so the gate is a property harness:
//! random dependency graphs, random input mutations, and the requirement that
//! demand-driven evaluation **always** agrees with a from-scratch recompute.
//!
//! The generated programs exercise the parts that make agreement hard:
//!
//! - **Dynamic dependencies.** A node's dependencies depend on the values it
//!   read, so the recorded edges differ between revisions.
//! - **Early cutoff.** Many functions map their inputs onto a small range, so
//!   recomputation frequently produces the value it had, which is what
//!   backdating exists for.
//! - **Durability, and the guard.** Inputs carry durability and can change to
//!   a different one; a conditional node can switch from durable inputs to
//!   volatile ones, which is exactly when backdating a memo whose durability
//!   decreased would let a dependent skip verification it needs.
//! - **Untracked reads**, which must re-execute in every new revision.
//! - **Persistence.** The database is reopened between rounds; a memo must
//!   survive and stay correct.
//!
//! Every case is seeded, and a failure prints the seed, the program and the
//! operation log, so it is reproducible rather than a story about randomness.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use cbr_memory::evaluator::{self, Durability, EvalError, Functions, Session};
use rusqlite::Connection;

// ---- the generated program -------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
enum Operation {
    /// Sum of every dependency, modulo `modulus`: a small modulus makes equal
    /// recomputations, and so backdating, common.
    Sum { deps: Vec<String>, modulus: u64 },
    /// Reads `switch`; reads `even` when that value is even and `odd`
    /// otherwise. The edges recorded therefore change between revisions.
    Choice {
        switch: String,
        even: String,
        odd: String,
    },
    /// Sum of its dependencies plus a value the graph does not record.
    Untracked { deps: Vec<String>, modulus: u64 },
}

#[derive(Clone, Debug)]
struct Program {
    /// `(key, value, durability)` in creation order.
    inputs: Vec<(String, u64, Durability)>,
    /// `node:<index>` in dependency order: a node only names earlier nodes.
    nodes: Vec<Operation>,
}

impl Program {
    fn node_key(index: usize) -> String {
        evaluator::key("node", &index.to_string())
    }

    fn keys(&self) -> Vec<String> {
        (0..self.nodes.len()).map(Program::node_key).collect()
    }
}

/// A deterministic generator, so a seed reproduces a case exactly.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*: small, and its quality is irrelevant here as long as it
        // is reproducible.
        let mut state = self.0;
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        self.0 = state;
        state.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn durability(&mut self) -> Durability {
        match self.below(3) {
            0 => Durability::Low,
            1 => Durability::Medium,
            _ => Durability::High,
        }
    }
}

fn generate(seed: u64) -> Program {
    let mut rng = Rng(seed | 1);
    let input_count = 3 + rng.below(5);
    let inputs: Vec<(String, u64, Durability)> = (0..input_count)
        .map(|index| {
            (
                format!("input:{index}"),
                rng.next() % 16,
                rng.durability(),
            )
        })
        .collect();
    let node_count = 4 + rng.below(12);
    let mut nodes = Vec::new();
    for index in 0..node_count {
        let available: Vec<String> = inputs
            .iter()
            .map(|(key, _, _)| key.clone())
            .chain((0..index).map(Program::node_key))
            .collect();
        let pick = |rng: &mut Rng| available[rng.below(available.len())].clone();
        let operation = match rng.below(10) {
            0..=5 => {
                let count = 1 + rng.below(3);
                let deps = (0..count).map(|_| pick(&mut rng)).collect();
                Operation::Sum {
                    deps,
                    modulus: 1 + (rng.next() % 4),
                }
            }
            6..=8 => Operation::Choice {
                switch: pick(&mut rng),
                even: pick(&mut rng),
                odd: pick(&mut rng),
            },
            _ => {
                let deps = vec![pick(&mut rng)];
                Operation::Untracked {
                    deps,
                    modulus: 1 + (rng.next() % 4),
                }
            }
        };
        nodes.push(operation);
    }
    Program { inputs, nodes }
}

// ---- the two evaluations to compare ----------------------------------------

/// What the graph does not record: the harness changes it only when a
/// revision actually advances, so a correct evaluator agrees with the
/// from-scratch recompute; an untracked node that is *not* re-executed in a
/// new revision will not.
#[derive(Default)]
struct Untracked(Mutex<u64>);

impl Untracked {
    fn get(&self) -> u64 {
        *self.0.lock().expect("untracked value")
    }

    fn set(&self, value: u64) {
        *self.0.lock().expect("untracked value") = value;
    }
}

fn functions(program: &Program, untracked: Arc<Untracked>, executions: Arc<Mutex<u64>>) -> Functions {
    let program = program.clone();
    let mut functions = Functions::new();
    functions.register("node", move |session: &mut Session<'_>, argument: &str| {
        *executions.lock().expect("execution count") += 1;
        let index: usize = argument
            .parse()
            .map_err(|_| EvalError::Function(format!("not a node index: {argument}")))?;
        let operation = program
            .nodes
            .get(index)
            .ok_or_else(|| EvalError::Function(format!("no node {index}")))?;
        let value = match operation {
            Operation::Sum { deps, modulus } => {
                let mut total = 0u64;
                for dep in deps {
                    total = total.wrapping_add(number(&session.read(dep)?));
                }
                total % modulus
            }
            Operation::Choice { switch, even, odd } => {
                let chosen = if number(&session.read(switch)?) % 2 == 0 {
                    even
                } else {
                    odd
                };
                number(&session.read(chosen)?)
            }
            Operation::Untracked { deps, modulus } => {
                session.report_untracked();
                let mut total = untracked.get();
                for dep in deps {
                    total = total.wrapping_add(number(&session.read(dep)?));
                }
                total % modulus
            }
        };
        Ok(bytes(value))
    });
    functions
}

fn bytes(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn number(value: &[u8]) -> u64 {
    let mut buffer = [0u8; 8];
    buffer.copy_from_slice(&value[..8]);
    u64::from_le_bytes(buffer)
}

/// The from-scratch recompute: no memos, no revisions, straight off the
/// current input values. This is the answer the evaluator must always match.
fn recompute(program: &Program, inputs: &BTreeMap<String, u64>, untracked: u64, key: &str) -> u64 {
    fn value(
        program: &Program,
        inputs: &BTreeMap<String, u64>,
        untracked: u64,
        key: &str,
        memo: &mut BTreeMap<String, u64>,
    ) -> u64 {
        if let Some(known) = inputs.get(key) {
            return *known;
        }
        if let Some(known) = memo.get(key) {
            return *known;
        }
        let index: usize = key
            .strip_prefix("node:")
            .and_then(|text| text.parse().ok())
            .expect("a node key");
        let computed = match &program.nodes[index] {
            Operation::Sum { deps, modulus } => {
                let mut total = 0u64;
                for dep in deps {
                    total = total.wrapping_add(value(program, inputs, untracked, dep, memo));
                }
                total % modulus
            }
            Operation::Choice { switch, even, odd } => {
                let chosen = if value(program, inputs, untracked, switch, memo) % 2 == 0 {
                    even
                } else {
                    odd
                };
                value(program, inputs, untracked, chosen, memo)
            }
            Operation::Untracked { deps, modulus } => {
                let mut total = untracked;
                for dep in deps {
                    total = total.wrapping_add(value(program, inputs, untracked, dep, memo));
                }
                total % modulus
            }
        };
        memo.insert(key.to_string(), computed);
        computed
    }
    let mut memo = BTreeMap::new();
    value(program, inputs, untracked, key, &mut memo)
}

struct Database {
    directory: tempfile::TempDir,
    connection: Connection,
}

impl Database {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("temp dir");
        let connection = Connection::open(directory.path().join("memory.sqlite")).expect("opens");
        evaluator::migrate(&connection).expect("migrates");
        Self {
            directory,
            connection,
        }
    }

    /// Reopen, as a restart would. Memos must survive it.
    fn reopen(self) -> Self {
        drop(self.connection);
        let connection =
            Connection::open(self.directory.path().join("memory.sqlite")).expect("reopens");
        evaluator::migrate(&connection).expect("migrates");
        Self {
            directory: self.directory,
            connection,
        }
    }
}

// ---- the property ----------------------------------------------------------

#[test]
fn demand_driven_evaluation_always_agrees_with_a_from_scratch_recompute() {
    for seed in 1..=120u64 {
        let program = generate(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let untracked = Arc::new(Untracked::default());
        let executions = Arc::new(Mutex::new(0u64));
        let registry = functions(&program, untracked.clone(), executions.clone());
        let mut database = Database::new();
        let mut values: BTreeMap<String, u64> = BTreeMap::new();
        let mut log: Vec<String> = Vec::new();
        let mut rng = Rng(seed | 1);

        for (key, value, durability) in &program.inputs {
            evaluator::set_input(&database.connection, key, &bytes(*value), *durability)
                .expect("sets an input");
            values.insert(key.clone(), *value);
        }
        log.push(format!("inputs {values:?}"));

        for round in 0..8 {
            let keys = program.keys();
            let mut queried: Vec<String> = Vec::new();
            for key in &keys {
                if rng.below(2) == 0 {
                    queried.push(key.clone());
                }
            }
            queried.push(keys[rng.below(keys.len())].clone());
            for key in &queried {
                let evaluated = number(
                    &evaluator::evaluate(&database.connection, &registry, key)
                        .unwrap_or_else(|error| panic!("seed {seed}: {error}\n{log:#?}")),
                );
                let expected = recompute(&program, &values, untracked.get(), key);
                assert_eq!(
                    evaluated, expected,
                    "seed {seed}, round {round}, key {key}: the evaluator disagreed with a \
                     from-scratch recompute\nprogram {program:#?}\nlog {log:#?}"
                );
            }
            // Querying again in the same revision executes nothing.
            let before = *executions.lock().expect("count");
            for key in &queried {
                evaluator::evaluate(&database.connection, &registry, key).expect("re-evaluates");
            }
            assert_eq!(
                *executions.lock().expect("count"),
                before,
                "seed {seed}, round {round}: a second query in the same revision re-executed"
            );

            // Mutate. Sometimes the same value, sometimes a different
            // durability: both must leave the graph correct.
            let mut advanced = false;
            for _ in 0..1 + rng.below(3) {
                let (key, _, durability) = &program.inputs[rng.below(program.inputs.len())];
                let value = if rng.below(4) == 0 {
                    values[key]
                } else {
                    rng.next() % 16
                };
                let durability = if rng.below(3) == 0 {
                    rng.durability()
                } else {
                    *durability
                };
                let changed =
                    evaluator::set_input(&database.connection, key, &bytes(value), durability)
                        .expect("sets an input");
                values.insert(key.clone(), value);
                log.push(format!(
                    "round {round}: {key} = {value} ({durability:?}), changed {changed}"
                ));
                advanced |= changed;
            }
            if advanced {
                untracked.set(untracked.get() + 1);
            }
            if rng.below(3) == 0 {
                database = database.reopen();
                log.push(format!("round {round}: reopened"));
            }
        }

        // Finally: every node, against a database that never saw any of it.
        let cold = Database::new();
        for (key, _, durability) in &program.inputs {
            evaluator::set_input(&cold.connection, key, &bytes(values[key]), *durability)
                .expect("sets an input");
        }
        for key in program.keys() {
            let warm = number(
                &evaluator::evaluate(&database.connection, &registry, &key).expect("evaluates"),
            );
            let fresh =
                number(&evaluator::evaluate(&cold.connection, &registry, &key).expect("evaluates"));
            assert_eq!(
                warm, fresh,
                "seed {seed}, key {key}: a warm database disagreed with a cold one\n\
                 program {program:#?}\nlog {log:#?}"
            );
        }
    }
}

// ---- the properties the random harness cannot state directly ---------------

/// Early cutoff: a recomputation that produces the value it had stops there,
/// and its dependents are not re-executed.
#[test]
fn a_recomputed_value_that_did_not_change_does_not_re_execute_its_dependents() {
    let database = Database::new();
    let executions: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let mut functions = Functions::new();
    let recorded = executions.clone();
    functions.register("constant", move |session: &mut Session<'_>, _argument| {
        recorded.lock().expect("log").push("constant".into());
        // Whatever the input is, this is 7.
        let _ = session.read("input:a")?;
        Ok(bytes(7))
    });
    let recorded = executions.clone();
    functions.register("above", move |session: &mut Session<'_>, _argument| {
        recorded.lock().expect("log").push("above".into());
        Ok(bytes(number(&session.read("constant:x")?) + 1))
    });

    evaluator::set_input(&database.connection, "input:a", &bytes(1), Durability::Low)
        .expect("sets an input");
    let first = evaluator::evaluate(&database.connection, &functions, "above:y").expect("evaluates");
    assert_eq!(number(&first), 8);
    assert_eq!(
        *executions.lock().expect("log"),
        vec!["above".to_string(), "constant".to_string()]
    );

    executions.lock().expect("log").clear();
    evaluator::set_input(&database.connection, "input:a", &bytes(2), Durability::Low)
        .expect("sets an input");
    let second =
        evaluator::evaluate(&database.connection, &functions, "above:y").expect("evaluates");
    assert_eq!(number(&second), 8);
    assert_eq!(
        *executions.lock().expect("log"),
        vec!["constant".to_string()],
        "the constant is recomputed, its dependent is not"
    );
}

/// A node that read something untracked is re-executed in every revision,
/// however durable its recorded inputs are.
#[test]
fn an_untracked_read_is_never_validated_without_re_executing() {
    let database = Database::new();
    let executions = Arc::new(Mutex::new(0u64));
    let outside = Arc::new(Untracked::default());
    let mut functions = Functions::new();
    let counted = executions.clone();
    let read = outside.clone();
    functions.register("untracked", move |session: &mut Session<'_>, _argument| {
        *counted.lock().expect("count") += 1;
        session.report_untracked();
        let base = number(&session.read("input:durable")?);
        Ok(bytes(base + read.get()))
    });

    evaluator::set_input(
        &database.connection,
        "input:durable",
        &bytes(10),
        Durability::High,
    )
    .expect("sets an input");
    assert_eq!(
        number(&evaluator::evaluate(&database.connection, &functions, "untracked:x").expect("ok")),
        10
    );
    assert_eq!(*executions.lock().expect("count"), 1);

    // A new revision, even one that changes nothing this node read.
    outside.set(5);
    evaluator::set_input(
        &database.connection,
        "input:other",
        &bytes(1),
        Durability::High,
    )
    .expect("sets an input");
    assert_eq!(
        number(&evaluator::evaluate(&database.connection, &functions, "untracked:x").expect("ok")),
        15,
        "an untracked read must be re-executed in a new revision"
    );
    assert_eq!(*executions.lock().expect("count"), 2);
}

/// The reverse index: which derived results a change reaches.
#[test]
fn the_dependents_of_an_input_are_recorded_for_invalidation() {
    let database = Database::new();
    let mut functions = Functions::new();
    functions.register("double", |session: &mut Session<'_>, argument| {
        Ok(bytes(number(&session.read(&format!("input:{argument}"))?) * 2))
    });
    functions.register("sum", |session: &mut Session<'_>, _argument| {
        Ok(bytes(
            number(&session.read("double:a")?) + number(&session.read("double:b")?),
        ))
    });
    for name in ["a", "b"] {
        evaluator::set_input(
            &database.connection,
            &format!("input:{name}"),
            &bytes(3),
            Durability::Medium,
        )
        .expect("sets an input");
    }
    evaluator::evaluate(&database.connection, &functions, "sum:x").expect("evaluates");
    assert_eq!(
        evaluator::dependents(&database.connection, "input:a").expect("dependents"),
        vec!["double:a".to_string()]
    );
    assert_eq!(
        evaluator::dependents(&database.connection, "double:b").expect("dependents"),
        vec!["sum:x".to_string()]
    );
}

/// A cycle is refused, and refusing it leaves nothing half-written.
#[test]
fn a_cycle_is_refused_rather_than_served_from() {
    let database = Database::new();
    let mut functions = Functions::new();
    functions.register("loop", |session: &mut Session<'_>, argument| {
        let next = if argument == "a" { "loop:b" } else { "loop:a" };
        session.read(next)
    });
    let error = evaluator::evaluate(&database.connection, &functions, "loop:a")
        .expect_err("a cycle is an error");
    assert!(matches!(error, EvalError::Cycle(_)), "{error:?}");
    assert!(
        evaluator::memo(&database.connection, "loop:a")
            .expect("memo")
            .is_none()
    );
}
