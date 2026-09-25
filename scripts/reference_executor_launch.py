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
  scripted executor test adapter of the pinned `launch-config.schema.json`.
  Only an executor is configured with it. CBR never serves `execution/1`.
- **CBR's `cbr-provider`**, for every other participant: the evidence,
  context and knowledge providers.

The reference provider is the protocol's own, built from the same verified
release extraction as the runner by `scripts/build_runner.py
--reference-executor`, which records its path in `runner.json`. A result from
this route is labelled **reference executor**: it shows CBR's context,
evidence and knowledge providers interoperating with a second implementation
acting as the executor. It is never evidence about a real adapter, and never
about PIO.

The script exits 2 without starting anything when an argument, the
configuration or a binary is missing; the runner then reports the participant
as having exited before listening.
"""

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
        binary, schemas = reference_executor()
        argv = [str(binary), "--data-dir", args["--data-dir"], "--config", args["--config"],
                "--schemas", str(schemas), "--socket", args["--socket"]]
        print(f"reference_executor_launch: {config.get('provider_id')} -> reference executor", file=sys.stderr)
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
