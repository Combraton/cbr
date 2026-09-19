# minimal-executor: an independent execution kernel (S-A)

A contracts-only third-party client for Protocol 0.1 (M6, owner decision M6-Q2). Python 3, standard library only. Run it as `python3 kernel.py --config <file> [--mutant ignores-required-boundary]`.

## Role, and what it is not

- **What it is.** An execution consumer, or kernel. It owns its dispatch boundary. For each work item it decides, from public protocol facts, whether a deterministic fake execution may be dispatched. Publishing the `dispatch.<work_id>` record **is** that dispatch.
- **What it is not.**
  - It is not an `execution/1` provider. It serves no operation, listens on no socket and claims no Execution conformance.
  - It is not an applicability engine. It decides nothing about semantic truth. It reads what the Context provider reports at the read.

## Sessions and negotiation

The kernel opens one Unix-socket session per provider per purpose. Each session runs `core.authenticate`, then `core.feature_dependencies`, then `core.negotiate`. When the query answers `method_not_found`, the kernel uses the dependencies stated in the profile documents instead. Every profile and feature listed is requested as required.

| Purpose | Provider | Profiles and features requested |
|---|---|---|
| Packet facts | `context.provider` | `context/1` with `context.required_before_start`, `context.advisory`, `context.claims`; `core/1` with its dependencies (`core.events`) and `core.grants` |
| Packet bytes | each provider named by a packet reference's `artifact.provider` | `evidence/1`; `core/1` with `core.events` and `core.grants` |
| Records | `records.provider` | `evidence/1`; `core/1` with `core.events` and `core.grants` |

`core.grants` is requested because every profile operation carries the configured grant, and CORE §5 allows a `grant` field only when `core.grants` is negotiated (DIVERGENCES TP-1).

## Behavior

At every poll (`poll_ms`, default 25), the kernel reads "now" from `clock_file`. Each undispatched item whose `dispatch_at` has passed is evaluated. The first rule that applies gives the state:

1. **`unsatisfied`: wrong bytes.** The SHA-256 of the exact bytes received with `evidence.fetch` differs from the reference digest, or the provider refuses the fetch with `artifact_digest_mismatch`. Such bytes are never held.
2. **`unknown`: unreadable.** The bytes cannot be fetched in full for any other reason (unreachable provider, unavailable bytes, an inconsistent response), `context.packet.inspect` fails, or its `reference` is not exactly the bound packet reference.
3. **`stale`.** `invalidated_items` names an item that is not advisory.
4. **`unknown`.** `unverified_items` names an item that is not advisory, or a required item's result is not `satisfied`.
5. **`current`** otherwise.

An item whose obligation the facts do not state counts as required. `required_before_start` work is dispatched only when the state is `current`. `advisory` work with `proceed_with_gap` is always dispatched, with `gap` true unless the state is `current`. Any other obligation or fallback is withheld. A check record `check.<work_id>.<n>` is published whenever the state or the decision differs from the item's previous check. Records are canonical JSON with the interface's fixed descriptor values (scope `thirdparty`, retention class `standard`, `anchors: []`, coverage `complete` over `["content"]` with no gaps), published with `evidence.upload.prepare`, `evidence.upload.append` and `evidence.seal`, with command identities derived from the artifact ID. A publication interrupted by a lost connection is retransmitted as the same commands, which replay.

**Order.** Within one poll, due items are evaluated in work-list order, from one reading of the clock file. Each item's check record and any dispatch record are published before the next item is evaluated. When publication fails, the kernel moves on; at later polls it retries that item's queued records, with the same bytes and command identities, and evaluates the item again only after they are published. A later item's dispatch record therefore shows that earlier items due at the same instant were decided.

Credentials are read only from the configuration file. They are never put on the command line, in the environment or in a log line; the logger also redacts them.

## Mutants

All three are deliberately broken kernels, for fixtures to detect:
- **`--mutant ignores-required-boundary`** evaluates and publishes honest check records, including `decision: "withhold"`. It then dispatches every due item whatever its state. It must fail at the step that expects no dispatch.
- **`--mutant ignores-claim-invalidation`** keeps rules 1 and 2: it verifies the bytes and treats unreadable or mismatched facts as `unknown`. It ignores `invalidated_items` and `unverified_items`, so required work whose claim lost its permitted use, or can no longer be verified, looks `current` and is dispatched. The publication-time item results of rule 4 are still checked (TP-24). It must fail where the fixture expects no `dispatch.w-req`.
- **`--mutant trusts-advertised-digest`** does not hash the fetched packet bytes. Bytes the provider returned for the reference's digest count as held, so altered bytes served under the sealed digest look current. It must fail where the fixture expects `w-altered` to be `unsatisfied` and withheld.

## Written from

- `docs/spec/SPEC.md`
- `docs/spec/bindings/STREAM.md` (§1–§4, §6) and `docs/spec/bindings/ENCODING.md`
- `docs/spec/profiles/CORE.md` (§3–§6, §10–§12, §15, §18)
- `docs/spec/profiles/EVIDENCE.md` (§1, §3–§6)
- `docs/spec/profiles/CONTEXT.md` (§1, §2, §5, §6, §14)
- `docs/spec/profiles/EXECUTION.md` §13 (for context only; no Execution shapes are used)
- `docs/decisions/003`, `006`
- `schemas/core/1`, `schemas/evidence/1`, `schemas/context/1`, `schemas/stream/1`
- `conformance/thirdparty/README.md` and `docs/work/release-0.1/M6.md` (mixed-implementation composition)

The shared client code is in `../common/combraton_client.py`.

## Coverage

- **Demonstrated** (fixture `composition.thirdparty-kernel-enforces-required-claim-boundary`, version 3). A second implementation enforces a required dispatch boundary from CONTEXT §14 read-time facts. It checks packet bytes by its own digest: altered bytes served under the sealed digest (`w-altered`) and a reference the provider refuses (`w-tampered`) are both `unsatisfied` and withheld. Advisory work proceeds with its gap, and unavailable knowledge never counts as valid. It publishes its records as Evidence through public operations.
- **Not claimed.**
  - `execution/1` provider conformance;
  - the EXECUTION §13.1/§13.3 wire shapes (`context.blocked`, queue reasons, `context.checks` in `execution.inspect`);
  - admission, capacity release and transition boundaries;
  - applicability conditions the executor would observe itself;
  - restart recovery: clients are not durable, and fixtures never restart them (interface "Not durable");
  - an outage of the Context provider or of a packet's Evidence provider (recorded `unknown`, but no fixture stops those providers);
  - the Context, Knowledge and Evidence providers, which are the reference.
