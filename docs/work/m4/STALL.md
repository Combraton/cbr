# The index build's stall, measured before and after m4c

[READINESS §8](READINESS.md#8-bounded-runtime) sets the bounded runtime's
first job: take long work out of the preparation tick, where M3 measured
it holding everything else. [VERIFICATION](../../VERIFICATION.md) records
the same thing as a known limit. This is the measurement that says
whether m4c did it.

## What is being measured, and why it is not the build's wall time

`tick_context` runs at the start of **every** request, under the
provider's processing lock. So a long build inside it does not merely
make its own job slow; it makes every job on that provider slow, from
every connection. The build costs what it costs either way — m4c moves
where that cost is paid, not how much it is — so the build's wall time is
the wrong number to watch. The right one is **how long an unrelated job
waits**.

The instrument is `crates/cbr-cli/tests/stall.rs`: two clients on one
provider.

- One submits a context request over the repository and polls for its
  packet, timing every poll. Its **worst poll** is the stall seen from
  the inside.
- The other, on its own connection, makes an unrelated read every 100 ms
  for as long as the build lasts. Its **worst latency** is the stall seen
  by everybody else, and is the figure that matters.
- **Time to first packet** is printed beside them because it is the
  number that should *not* improve.

Each `cbr` call is a fresh process on a fresh connection, so every figure
carries the client's own startup. That floor is measured against an idle
provider and reported rather than subtracted: **4 ms**, on every run
below.

## The measurements

One run each, on the same machine, the same trees and the same debug
build, taking the provider at `64dfa89` for *before* and at `59e4b68`
for *after*.

| Repository | Tracked files | Head | Worst poll | Bystander's worst wait | Time to first packet |
| --- | --- | --- | --- | --- | --- |
| CBR | 1,120 | before | 8.511s | **8.506s** | 8.512s |
| CBR | 1,120 | after | 0.144s | **0.131s** | 8.240s |
| brian2 | 549 | before | 11.152s | **11.073s** | 11.152s |
| brian2 | 549 | after | 0.204s | **0.228s** | 10.421s |
| Knowscroll-v2 | 145 | before | 3.426s | **3.400s** | 3.426s |
| Knowscroll-v2 | 145 | after | 0.152s | **0.151s** | 3.529s |

The stall falls by **65×**, **49×** and **23×**. Time to first packet is
unchanged within the noise of a single run in each direction, which is
the expected result: the same work is done, and the provider stops
holding everything else while it does it.

Before m4c the bystander got **2 or 3 calls through** for the whole run,
because its second call blocked behind the build and returned only when
the build had finished. After m4c it got **32 to 94 through**, none of
them waiting longer than a quarter of a second.

## What this does not establish

- **One run each, not a distribution.** These are single measurements on
  a machine the owner is also using; treat the ratio, not the third
  decimal.
- **A debug build.** Every figure would be smaller under `--release`.
  Both sides are debug, so the comparison holds and the absolute numbers
  do not transfer.
- **Tracked files are not M3's blob counts.** M3 recorded 1,066 blobs for
  CBR, 553 for brian2 and 145 for Knowscroll; the trees have moved since,
  and `git ls-files` counts paths rather than objects. The three trees
  are the same three repositories at their present heads.
- **Nothing here is about a model call.** No model runs in this harness,
  and the provider is started with none configured. The stall that has
  been measured away is the index build's; a model call is longer and
  less predictable, and the same pool bounds it, but that is not what
  these numbers show.
