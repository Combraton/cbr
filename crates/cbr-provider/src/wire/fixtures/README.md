# Wire fixtures: what is observed, and what is still a guess

**One file here is observed. Every other file is hand-written and has never been compared with the live service.** The file name says which: `*.observed-<date>.json` was recorded from the live service on that date; `*.unverified-fixture.json` was not.

## Observed

| File | Recorded | What it is |
|---|---|---|
| `count-invalid-request.observed-2026-09-20.json` | Calibration run 1, 2026-09-20 ([record](../../../../../docs/work/m4/CALIBRATION.md)) | What `POST /v1/responses/input_tokens` answers when the request body is the wrong shape. Written out by hand from what was recorded, so this file is a fixture rather than a dump of an exchange; the exchange itself is not committed. |

It carries no repository text and no credential — it is 212 bytes of the provider's own validation message — and it is the only thing one live call established.

**What that one call changed:** an error from this provider's OpenAI-compatible surface arrives as `{"error":{"message","code"}}`, with `code` a **string**. Neither `base_resp.status_code` nor Anthropic's `{"type":"error"}` appeared, and the parser looks for both and not for this. That is recorded as a finding in the calibration record and is fixed with its own test, not here.

## Still unverified

The rest were written from the provider's public documentation and from the four
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
| **Least verified — the shape is a guess** | `count.unverified-fixture.json`. The member a count arrives under is not something this session can check; the parser accepts a small closed set of names and refuses a body carrying none of them, which leaves the local estimate standing. **Calibration run 1 did not replace this guess**: no successful count was made, because the count *request* was the wrong shape. |

**An unlabelled guess is worse than a labelled one.** This table graded the count *response* as a guess and said nothing about the count *request*, which was equally a guess and turned out to be wrong. The request shape is what ended run 1.
