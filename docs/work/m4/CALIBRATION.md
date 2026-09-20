# The calibration — run 1, 2026-09-20

**The run stopped at its first call. No token count was obtained, so the local byte bound is neither confirmed nor falsified.** [READINESS §10](READINESS.md#10-the-calibration-and-what-stops-m4)'s stop condition — one provider count above its local estimate — did not fire, because no provider count came back at all.

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
