#!/usr/bin/env python3
"""Launch one composition participant: CBR, or the protocol's reference executor.

The pinned conformance runner takes one participant descriptor per run and
starts every participant of a composition fixture from it (runner
`exec.rs`, `start_participant`), so it has no way to say "this participant is
a different implementation". A descriptor's launch argv is any program,
though, so this script is that program for
`conformance/participants/cbr-with-reference-executor-unix.json`. The runner
starts it once per participant with that participant's rendered launch
configuration, and it replaces itself with one of two binaries:

- **the reference provider**, when the configuration carries `executor`, the
  scripted executor test adapter of the pinned `launch-config.schema.json`,
  *and* the runner launched a named composition participant. Only an
  executor is configured with it. CBR never serves `execution/1`.
- **CBR's `cbr-provider`**, for every configuration without `executor`: the
  evidence, context and knowledge providers.

An `executor` configuration anywhere else is refused, never run on either
binary. The descriptor claims `execution/1` on the reference provider's
behalf, so a single-participant execution fixture launched through it would
otherwise pass on the reference provider under a CBR-named participant, the
misreporting RELEASE-SCOPE section 6 forbids. The signal is the three paths
the runner passes. Its `start_participant` (runner `exec.rs`) gives the
participant named N the data directory `<work>/participants/N/data`, the
configuration `<work>/participants/N/config.json` and the socket
`<work>/n-N/p.sock`; `start` and `expect_start_failure` give a fixture's
single participant `<work>/data` (or `data-G`), `<work>/config.json` and
`<work>/sK/p.sock`. All three must agree on one named participant.
`scripts/run_fixtures.py` also refuses this descriptor unless its filter
selects composition fixtures only.

The reference provider is the protocol's own, built from the same verified
release extraction as the runner by `scripts/build_runner.py
--reference-executor`, which records its path and SHA-256 in `runner.json`;
a binary that does not match the recorded digest is refused. A result from
this route is labelled **reference executor**: it shows CBR's context,
evidence and knowledge providers interoperating with a second implementation
acting as the executor. It is never evidence about a real adapter, and never
about PIO.

The script exits 2 without starting anything when an argument, the
configuration or a binary is missing, when the reference binary is not the
recorded one, or when an executor is launched outside a composition; the
runner then reports the participant as having exited before listening.
"""

import hashlib
import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CBR_PROVIDER = ROOT / "target" / "debug" / "cbr-provider"
RUNNER_IDENTITY = ROOT / "target" / "protocol-release" / "runner.json"
FLAGS = ("--data-dir", "--config", "--socket")


def refuse(message):
    print(f"reference_executor_launch: {message}", file=sys.stderr)
    raise SystemExit(2)


def parse(argv):
    if len(argv) != 2 * len(FLAGS) or [argv[i] for i in range(0, len(argv), 2)] != list(FLAGS):
        refuse(f"expected exactly {' X '.join(FLAGS)} X, got {argv}")
    return dict(zip(FLAGS, argv[1::2]))


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def composition_participant(args):
    """The participant's name when the runner launched a named composition participant, else None.

    Lexical, like the runner's own joins: every path must be the one the
    runner's start_participant gives the same participant under one work
    directory.
    """
    data = Path(args["--data-dir"])
    directory = data.parent
    name = directory.name
    work = directory.parent.parent
    if data.name != "data" or directory.parent.name != "participants" or not name:
        return None
    if Path(args["--config"]) != directory / "config.json":
        return None
    if Path(args["--socket"]) != work / f"n-{name}" / "p.sock":
        return None
    return name


def reference_executor():
    """The reference provider binary and its schemas, as build_runner.py recorded them."""
    if not RUNNER_IDENTITY.is_file():
        refuse("no runner identity: run scripts/build_runner.py --reference-executor")
    identity = json.loads(RUNNER_IDENTITY.read_text())
    recorded = identity.get("reference_executor")
    if not recorded:
        refuse("runner.json names no reference executor: run scripts/build_runner.py --reference-executor")
    binary = ROOT / recorded["binary"]
    schemas = ROOT / recorded["schemas"]
    if not binary.is_file() or not schemas.is_dir():
        refuse(f"reference executor missing at {binary.relative_to(ROOT)}")
    recorded_digest = recorded.get("sha256")
    if not recorded_digest:
        refuse("runner.json records no sha256 for the reference executor: re-run scripts/build_runner.py --reference-executor")
    found = sha256(binary)
    if found != recorded_digest:
        refuse(
            f"reference executor sha256 {found} is not the recorded {recorded_digest}: "
            "re-run scripts/build_runner.py --reference-executor"
        )
    return binary, schemas


def main():
    args = parse(sys.argv[1:])
    try:
        config = json.loads(Path(args["--config"]).read_text())
    except (OSError, ValueError) as error:
        refuse(f"cannot read the launch configuration: {error}")
    if not isinstance(config, dict):
        refuse("the launch configuration is not an object")

    if "executor" in config:
        name = composition_participant(args)
        if name is None:
            refuse(
                "an executor configuration outside a named composition participant: this launcher "
                "runs the reference executor only as a composition fixture's executor peer, and CBR "
                "never serves execution/1. Run it with --filter composition. only."
            )
        binary, schemas = reference_executor()
        argv = [str(binary), "--data-dir", args["--data-dir"], "--config", args["--config"],
                "--schemas", str(schemas), "--socket", args["--socket"]]
        print(
            f"reference_executor_launch: participant {name} ({config.get('provider_id')}) -> reference executor",
            file=sys.stderr,
        )
    else:
        if not CBR_PROVIDER.is_file():
            refuse("cbr-provider missing: run cargo build --workspace")
        argv = [str(CBR_PROVIDER), "--data-dir", args["--data-dir"], "--config", args["--config"],
                "--socket", args["--socket"]]
        print(f"reference_executor_launch: {config.get('provider_id')} -> cbr-provider", file=sys.stderr)
    sys.stderr.flush()
    os.execv(argv[0], argv)


if __name__ == "__main__":
    main()
