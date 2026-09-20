# The calibration

Two runs, each on the owner's explicit authorisation, one run each, no retries.

- [**Run 2, 2026-09-21**](#run-2-2026-09-21) — **the measurement. The bound held on every file.**
- [Run 1, 2026-09-20](#run-1-2026-09-20) — stopped at its first call; no count was obtained.

---

## Run 2, 2026-09-21

**The local byte bound was never exceeded. [READINESS §10](READINESS.md#10-the-calibration-and-what-stops-m4)'s stop condition did not fire, and M4 is not stopped.**

Every one of the six files was counted, and the completion ran. The run ended with the completion reported as truncated, which is the documented behaviour of a sixteen-token limit on an M2.x model and **not** a failure — the calibration's own handling of that is a defect recorded below.

### What was run

| | |
|---|---|
| Built from | `main` at `dabf033` (the merge of [PR #25](https://github.com/Combraton/cbr/pull/25), pinned to `4aaa31f`), **release** profile |
| Command | `cbr-provider --calibrate <table> --permit-model-network --model-run-ceiling 100000`, and nothing else — no probe, no pre-flight, no dry run |
| Configuration | `cbr-config/1`, provider `minimax`, dialect **`responses`**, model `MiniMax-M2.7-highspeed` |
| Data directory | fresh, outside this repository |
| Keychain | read once at construction; no prompt appeared |
| Calls | **8**: seven counts and one completion. All eight succeeded. |

### The table: the local estimate against the provider's count

| file | local estimate | of which input bound | provider | table ratio | input ratio |
|---|---:|---:|---:|---:|---:|
| `clock.rs` | 12,506 | 7,378 | 1,962 | 6.37 | 3.76 |
| `keychain.rs` | 22,342 | 17,214 | 4,077 | 5.48 | 4.22 |
| `wire/endpoint.rs` | 11,528 | 6,400 | 1,583 | 7.28 | 4.04 |
| `wire/json.rs` | 19,743 | 14,615 | 3,410 | 5.79 | 4.29 |
| `wire/redact.rs` | 22,324 | 17,196 | 4,174 | 5.35 | 4.12 |
| non-Latin text | 5,843 | 715 | 210 | 27.82 | 3.40 |

**No provider count exceeded its local estimate.** Largest table ratio 27.82 (the non-Latin text), smallest 5.35 (`wire/redact.rs`).

**The table's own ratio column is misleading, and the extra columns say why.** The local estimate is the input bound *plus* a fixed 5,128 tokens of reserved generation and margin, so for a small input the ratio measures the fixed reservation rather than the bound. Against the input bound alone the figures are **3.40 to 4.29**, which is what [READINESS §10](READINESS.md#10-the-calibration-and-what-stops-m4) predicted — *"loose by three to four times for prose"* — and the non-Latin text, whose table ratio is 27.82, has the **tightest** input bound of the six at 3.40. The table should carry the input column; that is a change for the next run rather than a rewrite of this record.

### The completion

| | |
|---|---|
| `status` | **`incomplete`**, `incomplete_details.reason: "max_output_tokens"` |
| `output` | **one `reasoning` item and nothing else**; `output_text` was `null` |
| `usage` | `input_tokens: 28`, `output_tokens: 16`, `total_tokens: 44` |
| Reasoning tokens | **not reported.** There is no `output_tokens_details` member at all. |

**The sixteen-token limit was spent entirely on reasoning**, exactly as the documentation says it can be on an M2.x model: reasoning cannot be disabled, reasoning tokens are output tokens, and sixteen of them bought no answer. The parser read it as truncated and kept its usage, which is right.

### The second comparison: does the count predict what is charged?

| | |
|---:|---|
| Counting endpoint's prediction for that request | **122** input tokens |
| What the provider then charged for it | **28** input tokens |
| Difference | the count **over-predicted by 4.4×** |

**No finding**, because the margin flags the dangerous direction — a bill *above* the prediction — and this is the opposite. But it is the more interesting result: **on this one sample the counting endpoint over-predicts the bill by more than four times.** That is conservative and therefore safe, and it means the count call buys conservatism CBR already has from its local bound, at the price of a call. One sample is not a measurement; it is a reason to make the comparison on every call of the first live run.

### The ledger, and whether a count is charged

| id | request | kind | tokens | estimate |
|---:|---|---|---:|---:|
| 1 | `clock.rs.count` | usage | 1,962 | 12,506 |
| 2 | `keychain.rs.count` | usage | 4,077 | 22,342 |
| 3 | `wire/endpoint.rs.count` | usage | 1,583 | 11,528 |
| 4 | `wire/json.rs.count` | usage | 3,410 | 19,743 |
| 5 | `wire/redact.rs.count` | usage | 4,174 | 22,324 |
| 6 | `non-latin.count` | usage | 210 | 5,843 |
| 7 | `completion.count` | usage | 122 | 5,376 |
| 8 | `completion` | usage | 44 | 1,162 |

**CBR's ledger charged 15,582 tokens. The provider reported usage for exactly one call, the completion, at 44.**

**The count response carries no usage member at all** — it is `{"object": "response.input_tokens", "input_tokens": N}` and nothing more. So the provider said nothing about what a count costs, and CBR settles a count at the figure it counted, which is the conservative reading of silence. The consequence is stark: **15,538 of the 15,582 tokens charged in this run were CBR charging itself for seven count calls the provider never priced.** If counts are free, CBR's envelope is being consumed by nearly 400× what the run actually spent.

That is an **observation for the owner**, not a change made here. The documentation says nothing about billing for this endpoint ([fixtures README](../../../crates/cbr-provider/src/wire/fixtures/README.md)), and *"unbilled because a page says so"* is what this run exists to avoid. What can be said is that the provider reports no usage, and that settling silence conservatively is expensive enough to be worth the owner's decision.

### The recorded exchanges, by field name and size

**The request** (the completion's, 240 bytes):

| member | type | bytes |
|---|---|---:|
| `input` | array | 91 |
| `instructions` | string | 31 |
| `max_output_tokens` | int | 2 |
| `model` | string | 24 |
| `service_tier` | string | 10 |
| `stream` | bool | 5 |

Its count call sent the same three of those the endpoint documents — `input`, `instructions`, `model` — and none of the other three. That is the change run 1 bought.

**The response** (956 bytes) carried 37 members. The ones CBR reads: `status` (10 bytes), `output` (one item: `id`, `type`, `status`, `summary`, `content`), `output_text` (null), `usage.input_tokens`, `usage.output_tokens`, `usage.total_tokens`, `usage.input_tokens_details.cached_tokens`, `error` (null), `incomplete_details.reason` (17 bytes). The rest is the request echoed back.

**The credential appears nowhere.** Scanning every byte of `cbr.sqlite` (237,568), its write-ahead log (0) and its shared-memory file (32,768) for `bearer`, `authorization`, `api_key`, an `sk-` prefix and a JWT prefix gives **six matches in the database and none elsewhere — all six are CBR's own source text**, because `keychain.rs` and `wire/redact.rs` are in the corpus and those modules discuss credentials by name. The service *name* `minimax_api_key` is a public constant in a public repository; no credential value is present. The `Authorization` header is not recorded at all.

**Every recorded request is byte-identical to the file it was read from**, checked by SHA-256 against each source on disk. The redactor did not touch the corpus — which is worth saying, because that corpus is five thousand lines about credentials and redaction, and an over-eager redactor would have shredded it.

### How the live responses differed from the documented fixtures

The fixtures were written on 2026-09-21 from the published API reference. This is what the service actually sent.

| # | Documented | Observed |
|---|---|---|
| 1 | `usage.output_tokens_details.reasoning_tokens` | **Absent.** There is no `output_tokens_details` member. Reasoning tokens are counted in `output_tokens` and are not broken out, so **CBR cannot tell how much of a completion was reasoning.** |
| 2 | `usage` of `input_tokens`, `output_tokens`, `total_tokens` | Confirmed, plus `input_tokens_details.cached_tokens` |
| 3 | An output item has `type`, `content`, `summary` | Also `id` and a per-item `status` |
| 4 | A response has 12 top-level members | **37.** The request is echoed back in full: `instructions`, `tools`, `tool_choice`, `temperature`, `top_p`, `text`, `reasoning`, `max_output_tokens`, `max_tool_calls`, `parallel_tool_calls`, `previous_response_id`, `conversation`, `store`, `service_tier`, `safety_identifier`, `truncation` |
| 5 | `service_tier` sent as `standard` | **Echoed back as `null`**, so the field is accepted and not reflected. Whether it was honoured is unobservable from the response. |
| 6 | `store` is a response property | Confirmed, and its value was **`false`** |
| 7 | `status`, `incomplete_details.reason`, `error: null`, `output_text`, `reasoning_text` | All confirmed exactly |
| 8 | `{"object": "response.input_tokens", "input_tokens": N}` | **Confirmed exactly**, on seven calls |

Nothing contradicted the documentation; four things it did not mention were found, and one thing it named was missing.

### A defect in the calibration itself

**The run reports `STOPPED` and exits non-zero, and it should not have.** A truncated completion is an ordinary outcome with a cost — the reviewer said so before the run, and the parser implements it — but the calibration turns any non-answer into a stop. So a run that obtained every measurement it exists for is recorded as having stopped, and the table says `**STOPPED.**` above six good rows.

Nothing was lost: the table, the ledger and the exchanges are all complete and are what this record is built from. It is fixed in m4c, with the completion's status carried into the report rather than collapsed into success or failure.

---

## Run 1, 2026-09-20

**The run stopped at its first call. No token count was obtained.** Run 2 obtained them. [READINESS §10](READINESS.md#10-the-calibration-and-what-stops-m4)'s stop condition — one provider count above its local estimate — did not fire, because no provider count came back at all.

One run, no retries, on the owner's explicit authorisation at the m4b review. It is not re-run without a further one.

## What was run

| | |
|---|---|
| Built from | `main` at `025ddfb` (the merge of [PR #24](https://github.com/Combraton/cbr/pull/24), pinned to `324b1cc`), release profile |
| Command | `cbr-provider --calibrate <table> --permit-model-network --model-run-ceiling 100000` |
| Configuration | `cbr-config/1`, provider `minimax`, dialect `openai`, model `MiniMax-M2.7-highspeed` |
| Data directory | fresh, outside this repository |
| Keychain | read once at construction; no prompt appeared |

## What happened

| | |
|---|---:|
| Calls made | **1** (one count; the run stops on the first failure) |
| Request bytes sent | 7,359 |
| Response bytes received | 212 |
| Files counted | **0 of 6** |
| Completion | **did not run** |
| Ledger rows | 1 |
| Tokens charged by CBR's ledger | **12,495** |
| Tokens the provider reported | none — it reported no usage |

The single ledger row settled as `unknown` at its estimate of 12,495. That is the designed conservative direction: the body went out, the provider said nothing about what it charged, so the reservation stands rather than being reconciled to zero. **The provider almost certainly charged nothing**, because the request was rejected at parameter validation — so this row over-counts, which is the direction the envelope is built to err in.

## The table

Empty, with the stop recorded in it:

```
| file | local | provider | ratio |
|---|---:|---:|---:|

**STOPPED.** clock.rs: the count did not happen: model_call_failed
```

**Largest ratio: none. Smallest ratio: none. Counts above their estimate: none obtained.** The question READINESS §10 exists to answer is still open.

## Why it stopped

The counting endpoint rejected the request:

> `Failed to parse or validate request parameters. Please check JSON field names, types and required fields: binding: expr_path=input, cause=missing required parameter` — `code: invalid_prompt`

**The endpoint exists and validates.** `POST /v1/responses/input_tokens` is the right address; the body CBR sent was the wrong shape. CBR sent a chat-completions body keyed on `messages`, and the endpoint requires a member named **`input`**.

## How the live service differed from the hand-written fixtures

Every fixture in `crates/cbr-provider/src/wire/fixtures/` was written by hand from public documentation and labelled unverified. This is what one call found.

| # | What was assumed | What was observed |
|---|---|---|
| 1 | **Unstated: that the count call takes a chat-completions body.** m4b named the count *response* as a guess and never named the *request* as one. It was one. | The endpoint requires `input`, not `messages`. A whole guess went unlabelled, which is worse than a labelled wrong one. |
| 2 | An error arrives as `base_resp.status_code` (MiniMax's native shape) or as Anthropic's `{"type":"error","error":{"type":…}}` | It arrived as **`{"error":{"message","code"}}`** — OpenAI's shape, with neither of the members the parser looks for |
| 3 | An error code is an integer (`1002`, `1008`) | `error.code` is a **string**: `"invalid_prompt"` |
| 4 | The response to a successful count carries one of `total_tokens`, `input_tokens`, `tokens`, `token_num` | **Unobserved.** No successful count was made, so this remains a guess. |
| 5 | The completion's response shape, the `<think>` behaviour, the usage members | **All unobserved.** The completion never ran. |

Consequence for the parser, which is a finding rather than a failure: `provider_failure` recognised neither observed member, so the call was classified as a failure by its **HTTP status** rather than by its body. It still came out as a failure — the conservative direction — but for the wrong reason, and a body-level error arriving inside an HTTP 200 would have been read as a successful, empty response.

## What the recorded exchanges look like, by field name and size

Recording and redaction worked. Nothing is pasted here but names and sizes.

**Request** (7,359 bytes, one row in `model_calls`):

| member | type | bytes |
|---|---|---:|
| `max_tokens` | int | 2 |
| `messages` | list | 7,266 |
| `messages[0]` | role `system` | 29 |
| `messages[1]` | role `user` | 6,924 |
| `model` | str | 24 |
| `reasoning_split` | bool | 4 |
| `stream` | bool | 5 |

**Response** (212 bytes):

| member | type | bytes |
|---|---|---:|
| `error.message` | str | 164 |
| `error.code` | str | 14 |

**The credential appears nowhere in the store.** Searching every byte of `cbr.sqlite`, its write-ahead log and its shared-memory file for `bearer` or `authorization`, case-insensitively, returns **0 matches** in each. The `Authorization` header is not recorded at all: the boundary stores the request body and the response body, and the header is neither.

## What this does and does not establish

- **It does not stop M4.** The stop condition is a count above its estimate, and there was no count.
- **It does not clear the bound either.** After one live call, no token count of CBR's has been compared with the provider's. The interim status line stands.
- **It does establish that the live path works up to the provider's parser**: the Keychain read, the permit, the pinned host, TLS, the request, the recording boundary and the ledger's conservative settlement all behaved as built, against a real server, on the first try.
- **It establishes that a fixture written from documentation is worth exactly what a guess is worth** — and that an *unlabelled* guess, like the count request's shape, is worth less, because nothing marked it as one.

## What a second run would need

Not requested here, and not run. It would need: the count request built to the shape the endpoint documents; the parser taught this error shape; and the owner's word again.
