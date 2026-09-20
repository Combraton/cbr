# Unverified wire fixtures

**Every file in this directory is hand-written and has never been compared with the live service.**

They were written from the provider's public documentation and from the four
behaviours the owner recorded in [STACK §8.1](../../../../../docs/work/readiness/STACK.md).
**Nothing was fetched from the provider to make them**, and no call has been
made: m4b makes none ([READINESS §1](../../../../../docs/work/m4/READINESS.md)).

So a test that passes against these files proves that CBR's parser reads
**what CBR believes the provider sends**. It does not prove the provider sends
it. The first comparison with reality is the calibration
([READINESS §10](../../../../../docs/work/m4/READINESS.md)), which is its own
step after this work is reviewed and merged, and only on the owner's word.

The file names, the Rust constants that include them and the module that reads
them all say `unverified`, so that the label travels with the bytes rather than
living only here.

| Confidence | Files |
|---|---|
| Documented shapes, stable across both vendors' published APIs | the completion and tool-call fixtures of both dialects |
| Recorded by the owner as a provider behaviour, shape inferred | the `<think>` leak fixtures |
| **Least verified — the shape is a guess** | `count.unverified-fixture.json`. The member a count arrives under is not something this session can check; the parser accepts a small closed set of names and refuses a body carrying none of them, which leaves the local estimate standing. The calibration replaces the guess with the answer. |
