# Wire fixtures: documented, observed, or guessed

**Three labels, because they are three different things**, and m4b had only one. A shape read from the provider's published API reference is not the same as a shape seen come back from the live service, and neither is the same as a shape somebody reasoned their way to. The file name carries the label:

| Suffix | Means |
|---|---|
| `*.documented-<date>.json` | Written from the provider's published documentation, read on that date. **Not seen from the live service.** |
| `*.observed-<date>.json` | Recorded from the live service on that date, then written out by hand as a fixture. |
| `*.unverified-fixture.json` | **Guessed.** Reasoned from public knowledge and the owner's recorded notes, with no documentation page and no observation behind it. |

m4b's fixtures were all of the third kind and were labelled only as "unverified". That label was honest about confidence and silent about *provenance*, and the difference cost a calibration run: the count **request**'s shape was a guess that nothing marked as one, and it was the guess that failed.

## Sources, with retrieval dates

Read on **2026-09-21**, from the pages indexed at `https://platform.minimax.io/docs/llms.txt`, under the reviewer's authorisation to read the provider's public documentation. Reading a public web page is not calling the provider, and no call was made.

| Page | Read | What was taken from it |
|---|---|---|
| [`api-reference/responses-input-tokens.md`](https://platform.minimax.io/docs/api-reference/responses-input-tokens.md) | 2026-09-21 | `POST /v1/responses/input_tokens` takes `{model, input, instructions?, tools?, tool_choice?, text?, reasoning?}`; `input` is a string or an array of `{type, role, content}` items; the answer is `{"object": "response.input_tokens", "input_tokens": N}` |
| [`api-reference/responses-create.md`](https://platform.minimax.io/docs/api-reference/responses-create.md) | 2026-09-21 | `POST /v1/responses`; `max_output_tokens`, `service_tier` (`standard` default, or `priority`), `stream`, `instructions`, `tools`, `tool_choice` (`none`/`auto`), `text.format.type` (`text` only), `reasoning.effort`; `status` of `completed`/`incomplete`/`failed`; reasoning as its own output item; `usage` of `input_tokens`, `input_tokens_details.cached_tokens`, `output_tokens`, `output_tokens_details.reasoning_tokens`, `total_tokens`; and that **reasoning cannot be disabled on the M2.x models** |
| [`api-reference/text/api/openapi-responses.json`](https://platform.minimax.io/docs/api-reference/text/api/openapi-responses.json) | 2026-09-21 | The request/response split, read from the machine-readable schema rather than from prose: `service_tier` is a request property and **`store` is not** |

**Two things were checked and not confirmed**, and are recorded as such rather than carried forward:

- **That the counting endpoint is unbilled or exempt from quota.** The page carries no statement about billing, quota or rate limits. So nothing here assumes one: the count is reserved, recorded and settled like any other send, and it stays that way until a live run shows otherwise.
- **That `store` is a request parameter.** It appears in the response schema and not in the request's property list. So CBR sends no `store`, and whether the provider retains repository text is an **open question for the owner** ([READINESS §7](../../../../../docs/work/m4/READINESS.md)) rather than something a default silently decides.

These pages were read through a summarising fetch, which is a second-hand reading of a first-hand source. That is why every shape below is labelled `documented` and not `observed`.

## The files

### Documented — read from the pages above, never seen from the service

| File | Shape |
|---|---|
| `responses-text.documented-2026-09-21.json` | A completed response with one `message` output item |
| `responses-incomplete-reasoning-only.documented-2026-09-21.json` | `status: incomplete`, a `reasoning` item, no text, and a usage whose `output_tokens` are entirely `reasoning_tokens` — what a sixteen-token limit can produce on an M2.x model |
| `responses-tool-call.documented-2026-09-21.json` | A `function_call` output item, whose `arguments` are a JSON **string** |
| `responses-input-tokens.documented-2026-09-21.json` | `{"object": "response.input_tokens", "input_tokens": N}` |

### Observed — recorded from the live service

| File | Recorded | Shape |
|---|---|---|
| `count-invalid-request.observed-2026-09-20.json` | Calibration run 1 ([record](../../../../../docs/work/m4/CALIBRATION.md)) | `{"error":{"message","code"}}`, with `code` a **string**. Neither `base_resp` nor Anthropic's `{"type":"error"}` appeared. |

It carries no repository text and no credential — 212 bytes of the provider's own validation message — and it is written out by hand here, so this file is a fixture rather than a dump of an exchange.

### Guessed — no documentation page, no observation

Everything named `*.unverified-fixture.json`: the chat-completions and Anthropic shapes, the `<think>` leaks, the `base_resp` error codes, and `count.unverified-fixture.json`.

They were written from the provider's public documentation *as it was understood at m4b* and from the four behaviours the owner recorded in [STACK §8.1](../../../../../docs/work/readiness/STACK.md). **Nothing was fetched from the provider to make them.** They are kept because the two chat dialects are still served, as secondary wires; they are still guesses, and the two exhaustion status codes in particular are a guess that degrades to "something failed" rather than to success.
