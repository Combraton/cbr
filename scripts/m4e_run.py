#!/usr/bin/env python3
"""The m4e run, as code, reviewed before it is ever run.

The first live run is the one thing in M4 that spends the owner's quota
on realistic requests and sends a third party's repository to a
provider. So it is not a sequence of commands typed on the day: it is
this file, reviewed at a head, with a dry-run mode that proves it end to
end against the fake transport and never opens a socket.

    python3 scripts/m4e_run.py --manifest M --out D --dry-run
    python3 scripts/m4e_run.py --manifest M --out D --live \\
        --permit-model-network
    python3 scripts/m4e_run.py --ambiguity DATA_DIR

**The live run is not authorised by this file existing.** `--live` needs
`--permit-model-network` typed as well, and a run planned in READINESS
before it: on the owner's word at the time, as each of M4's was, and
since 2026-09-25 under the owner's standing grant, with its estimate and
ceiling written into m5 READINESS section 10 first. The flag carries that
decision; it does not make it.

What it does, per run in the manifest:

  1. launches a provider over a data directory under `--out`, bounded
     by what is left of the run ceiling rather than by the whole of it,
  2. registers the repository and submits one context request,
  3. polls until it settles, and writes the packet where the reviewer
     can score it,
  4. relaunches the same store with `--replay-model` and rebuilds the
     same question offline, comparing sealed sections -- **the replay
     gate**,
  5. reports what was retained, how much of it discovery's own two steps
     sealed, what replayed, and every question the records disagree
     about, with the answers that disagree.

What it refuses, before anything starts:

  * a repository whose **origin** is not one the owner's word in
    READINESS section 7 covers, read from the checkout rather than
    taken from the id the manifest typed;
  * **in live mode only**, an origin that `api.github.com` does not say
    is public -- asked once per distinct origin, unauthenticated, with
    no token, no `gh`, no proxy from the environment and no redirect
    followed. Only HTTP 200 with `"private": false` admits one;
    a 404, a 403, a rate limit, a timeout or an unreadable body are all
    refusals, because *unknown* belongs on the same side as *private*.
    **A dry run sends nothing off this machine, and no test in the suite
    makes this call**; only its parser is tested, on canned bodies;
  * a checkout that is not at the commit the manifest pins, when it
    pins one;
  * an investigation budget the items would exhaust before discovery
    was reached, which would produce a baseline packet reading as "the
    model did not help";
  * an output directory inside this repository, because a pilot's
    packet must never be committed;
  * `--live` without `--permit-model-network`;
  * a run ceiling above the 5,000,000 hard cap of READINESS section 9,
    or a stop above its 2,250,000.

And it stops **before** the run that could cross 2,250,000 -- the
estimate plus half -- rather than after the one that did: the worst case
of a run, its flow and a selection question per want, is a computed
number, so the arithmetic is done before the tokens are.

The report holds digests, paths, spans, counts and costs. **It holds no
repository text**, which is the licence rule for the pilots and is
checked by a test rather than remembered.
"""

import argparse
import base64
import contextlib
import hashlib
import json
import os
import re
import shutil
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parent

# READINESS section 9. The estimate is what m4e expects to spend; the
# stop is the estimate plus half, at which the run halts whatever state
# it is in; the ceiling is the hard cap, enforced by the ledger through
# `--model-run-ceiling` rather than by this script's intention.
ESTIMATE_TOKENS = 1_500_000
STOP_TOKENS = 2_250_000
RUN_CEILING_TOKENS = 5_000_000

# READINESS section 3: the most a request's two-step discovery flow can
# hold, and the most one item's selection question can, each question its
# first send and its widest repair (`model::question_worst`), in whichever
# dialect a launch configures frames the most. Both are
# computed against the real constants by `discovery::tests` and
# `selection::tests`, which read them back from here, rather than estimated
# here. A run asks at most one selection question per want and then the
# flow, so what one run can cost is `worst_case_of(run)`, and the run stops
# *before* a run that could cross the stop, not after one that did.
#
# They are what admission can reserve on the serving path. What a
# provider bills past a reservation is settled on top of them, and
# `model::question_worst` states when that can happen.
WORST_CASE_FLOW_TOKENS = 518_628
WORST_CASE_SELECTION_TOKENS = 167_753


def worst_case_of(run):
    """What one run can cost: its flow, and a selection question per want.

    **Every run asks its items first**, at most one selection question for
    each thing it wants, under the same job and before discovery. A stop
    that priced the flow alone started runs that their own item questions
    could carry past it.
    """
    return WORST_CASE_FLOW_TOKENS + len(run["wants"]) * WORST_CASE_SELECTION_TOKENS


def priced(run):
    """The words a stop gives for what a run was priced at."""
    wants = len(run["wants"])
    questions = "question" if wants == 1 else "questions"
    return (
        f"{worst_case_of(run)} worst case of one flow and {wants} "
        f"selection {questions}"
    )

# READINESS section 7: the owner's word covers these three and no
# others, and all three are public.
#
# **A repository is what its origin says it is, not what the manifest
# called it.** An id is a label somebody typed, so a check against the
# label alone admits any checkout in the filesystem under any name --
# `{"id": "brian2", "path": <a private tree>}` would have passed. The
# origins are pinned here and `check` reads each checkout's own before
# anything is launched.
PUBLIC_REPOSITORIES = {
    "cbr": ("github.com/combraton/cbr",),
    # The pilot is the owner's fork; a fresh clone of the project
    # upstream is the same public bytes and is admitted too.
    "brian2": ("github.com/legend101zz/brian2", "github.com/brian-team/brian2"),
    "knowscroll": ("github.com/legend101zz/knowscroll-v2",),
}

# The three models M4 may name (READINESS section 2). m4e runs the
# highspeed extraction model beside the synthesis one, which is what
# "MiniMax-M2.7-highspeed beside MiniMax-M3" means: the same question,
# twice, so the difference is the model and nothing else.
MODELS = ("MiniMax-M2.7-highspeed", "MiniMax-M2.7", "MiniMax-M3")

# The visibility check of READINESS section 7. Pinning an origin proves
# a checkout is the repository it claims to be; it says nothing about
# whether that repository is public, which is a live fact that can change
# under a table committed weeks earlier -- and did: at the round-40
# review one of the three pinned origins was private on the API while
# this file called it public.
GITHUB_HOST = "github.com/"
GITHUB_API = "https://api.github.com/repos/"
GITHUB_TIMEOUT_SECONDS = 10

PRINCIPAL = "owner"
DRY_RUN_CREDENTIAL = "ccred1.owner.m4e-dry-run"

SETTLE_SECONDS = 900


class Refused(Exception):
    """A condition that stops the run before anything is spent."""


def refuse(why):
    raise Refused(why)


# ---- what a checkout says it is -----------------------------------------


def normalised_origin(url):
    """The spellings of one remote, as one string.

    `https://github.com/x/y.git`, `git@github.com:x/y` and
    `ssh://git@github.com/x/y/` are the same repository, and a pin that
    only matched one of them would refuse the owner's own clone.
    """
    text = url.strip()
    for prefix in ("https://", "http://", "ssh://", "git://"):
        if text.startswith(prefix):
            text = text[len(prefix) :]
    if text.startswith("git@"):
        text = text[len("git@") :].replace(":", "/", 1)
    host, _, rest = text.partition("/")
    if "@" in host:
        host = host.split("@", 1)[1]
    text = f"{host}/{rest}" if rest else host
    if text.endswith(".git"):
        text = text[: -len(".git")]
    return text.rstrip("/").lower()


class _NoRedirects(urllib.request.HTTPRedirectHandler):
    """Follow nothing.

    A redirect is a second request to an address the answer chose, and
    the point of this check is that one named repository answered. A
    blocked redirect surfaces as its own status and is refused like any
    other non-200.
    """

    def redirect_request(self, request, fp, code, message, headers, newurl):
        return None


def ask_github(name, timeout=GITHUB_TIMEOUT_SECONDS):
    """One **unauthenticated** GET of `api.github.com/repos/<owner>/<name>`.

    Returns `(status, body)`, or `(None, why)` when nothing came back at
    all -- a timeout, a DNS failure, a TLS failure, a blocked redirect.

    **No credential is read, built or sent on this path.** No token, no
    `gh`, no environment variable, no `.netrc`: the opener is built from
    an empty `ProxyHandler`, so not even a proxy comes from the
    environment, and the only headers are an `Accept` and a `User-Agent`.
    A public repository answers this call from anywhere; a check that
    needed the owner's credential would be asking a different question --
    *can I see it* rather than *can anyone*.
    """
    opener = urllib.request.build_opener(
        urllib.request.ProxyHandler({}),
        _NoRedirects(),
    )
    request = urllib.request.Request(
        f"{GITHUB_API}{name}",
        method="GET",
        headers={"Accept": "application/vnd.github+json", "User-Agent": "cbr-m4e-run"},
    )
    try:
        with opener.open(request, timeout=timeout) as answer:
            return answer.status, answer.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as answered:
        # A 404, a 403, a rate limit, a blocked redirect: an answer, and
        # not one that admits anything.
        return answered.code, answered.read().decode("utf-8", "replace")
    except Exception as nothing:
        return None, f"{type(nothing).__name__}: {nothing}"


def not_public(name, status, body):
    """Why `name` may not be read, or `None` when it is public.

    **One answer admits a repository**: HTTP 200 carrying `"private"`
    exactly `false`. Everything else is a refusal by name -- a 404
    (which is also what a private repository returns to a caller with no
    credential), a 403, a rate limit, a body that is not JSON, a body
    with no `private` member, a `private` that is not the boolean
    `false`. The failure this exists to prevent is sending a third
    party's private text to a provider, so *unknown* has to land on the
    same side as *private*.
    """
    if status != 200:
        return (
            f"api.github.com answered HTTP {status} for {name}. Only a 200 "
            "saying the repository is public admits it; a 404 is also what a "
            "private repository answers to a caller with no credential, and "
            "this path deliberately has none."
        )
    try:
        parsed = json.loads(body)
    except (ValueError, TypeError):
        return (
            f"api.github.com answered 200 for {name} with a body that is not "
            "JSON, so nothing said the repository is public."
        )
    if not isinstance(parsed, dict) or "private" not in parsed:
        return (
            f"api.github.com answered 200 for {name} with no `private` "
            "member, so nothing said the repository is public."
        )
    if parsed["private"] is not False:
        return (
            f"{name} is not public: the API says `private` is "
            f"{parsed['private']!r}. Sending a private repository's text to a "
            "provider needs the owner's explicit word, and READINESS section 7 "
            "is not it."
        )
    return None


def refuse_unless_public(checkouts):
    """Ask, once per distinct origin, before any provider is launched.

    **This is the only thing in the harness that opens a socket to
    anywhere but the provider**, and it is reached in live mode alone: a
    dry run sends nothing off the machine, and no test in the suite
    exercises this call.
    """
    for origin in sorted({checkout["origin"] for checkout in checkouts.values()}):
        if not origin.startswith(GITHUB_HOST):
            refuse(
                f"{origin} is not on {GITHUB_HOST.rstrip('/')}, so this cannot "
                "ask whether it is public. Every pinned origin is, and a new "
                "host needs its own answer to that question."
            )
        name = origin[len(GITHUB_HOST) :]
        status, body = ask_github(name)
        if status is None:
            refuse(
                f"nothing came back from api.github.com for {name} ({body}). "
                "Whether it is public is then unknown, and unknown is refused "
                "for the same reason private is."
            )
        why = not_public(name, status, body)
        if why:
            refuse(why)


def git_says(path, arguments, why):
    finished = subprocess.run(
        ["git", "-C", str(path), *arguments],
        capture_output=True,
        text=True,
        check=False,
    )
    if finished.returncode != 0:
        refuse(f"{why}: git {' '.join(arguments)} said: {finished.stderr.strip()}")
    return finished.stdout.strip()


# ---- what may be run ----------------------------------------------------


def check(manifest, out, live, permit, ceiling, stop):
    """Every refusal, before a provider is launched or a byte is sent.

    Returns the output directory and, per run, what its checkout said it
    was -- which is read here, before anything starts, so that a run over
    a repository nobody authorised costs nothing at all.
    """
    if live and not permit:
        refuse(
            "--live needs --permit-model-network as well. A live run reads "
            "the owner's credential and opens a socket to a provider, and "
            "having built the harness is not permission to use it."
        )
    if ceiling > RUN_CEILING_TOKENS:
        refuse(
            f"--run-ceiling {ceiling} is above m4e's hard cap of "
            f"{RUN_CEILING_TOKENS}. Raising it is a change to RUN_CEILING_TOKENS "
            "here, which J2's harness reads too, with its reason recorded in "
            "m5 READINESS section 10; never a flag."
        )
    if stop > STOP_TOKENS:
        refuse(
            f"--stop-tokens {stop} is above the {STOP_TOKENS} of READINESS "
            "section 9, which is the estimate plus half. Like the ceiling, a "
            "flag can lower it and not raise it."
        )
    out = Path(out).resolve()
    if out == REPOSITORY or REPOSITORY in out.parents:
        refuse(
            f"--out {out} is inside {REPOSITORY}. A pilot's packet holds a "
            "third party's repository text and must never be committed; "
            "the reviewer scores it where it is written, outside the tree."
        )
    runs = manifest.get("runs")
    if not isinstance(runs, list) or not runs:
        refuse("the manifest names no runs")
    seen = set()
    checkouts = {}
    for run in runs:
        for member in ("id", "repository", "model", "task", "selector", "wants"):
            if member not in run:
                refuse(f"a run is missing {member!r}: {run.get('id', run)!r}")
        if run["id"] in seen:
            refuse(f"two runs share the id {run['id']!r}")
        seen.add(run["id"])
        if run["model"] not in MODELS:
            refuse(
                f"{run['id']}: {run['model']!r} is not one of the three "
                f"models M4 may name: {', '.join(MODELS)}"
            )
        identifier = run["repository"].get("id")
        if identifier not in PUBLIC_REPOSITORIES:
            refuse(
                f"{run['id']}: {identifier!r} is not one of the repositories "
                "the owner's word covers, which are the three public ones: "
                f"{', '.join(PUBLIC_REPOSITORIES)}. A private repository "
                "needs the owner's explicit word before a single byte of it "
                "is sent."
            )
        path = Path(run["repository"].get("path", "")).expanduser()
        if not (path / ".git").exists():
            refuse(f"{run['id']}: {path} is not a git checkout")

        # **The id is a label; the origin is the repository.** Checked in
        # both modes, because a dry run reads the same bytes off the same
        # disk as a live one -- the only thing `--dry-run` changes is
        # where the answers come from.
        origin = normalised_origin(
            git_says(
                path,
                ["remote", "get-url", "origin"],
                f"{run['id']}: {path} has no origin remote, so nothing but "
                "the manifest says which repository it is",
            )
        )
        permitted = PUBLIC_REPOSITORIES[identifier]
        if origin not in permitted:
            refuse(
                f"{run['id']}: {path} has origin {origin!r}, which is not "
                f"{identifier!r}. That id is {' or '.join(permitted)}. The "
                "owner's word covers repositories, not the names a manifest "
                "gives them."
            )
        head = git_says(path, ["rev-parse", "HEAD"], f"{run['id']}: {path} has no HEAD")
        wanted = run["repository"].get("commit")
        if wanted and head != wanted:
            refuse(
                f"{run['id']}: {path} is at {head}, and the manifest pins "
                f"{wanted}. A pilot question was sealed against a tree; "
                "answering it over a different one measures something else."
            )
        checkouts[run["id"]] = {"origin": origin, "head": head}

        # **Discovery must have room after the items.** Items are asked
        # first and a flow that cannot finish is not started, so a run
        # whose budget the items exhaust never asks the two questions m4e
        # exists to measure -- and produces a baseline packet that reads
        # as "the model did not help".
        wants = run["wants"]
        if not isinstance(wants, list) or not wants:
            refuse(f"{run['id']}: names no wants")
        investigation = int(run.get("investigation", 5))
        if investigation < len(wants) + 2:
            refuse(
                f"{run['id']}: investigation {investigation} with "
                f"{len(wants)} wants leaves no room for discovery's two "
                "questions. Items are asked first, so this run would spend "
                "its budget before discovery was reached and report nothing "
                f"about it. It needs at least {len(wants) + 2}."
            )

    # **Last, and only when something will actually be sent.** Every
    # refusal above reads the manifest and the disk; this one opens a
    # socket, so it is worth making only once the cheap answers are in --
    # and worth making at all only in live mode, where a third party's
    # bytes leave the machine.
    #
    # Pinning an origin says a checkout *is* the repository it claims to
    # be. It does not say that repository is public, which is a live fact
    # about an account somebody else controls; at the round-40 review one
    # of the three pinned origins was private on the API while this file
    # called it public.
    if live:
        refuse_unless_public(checkouts)
    return out, checkouts


# ---- driving a provider -------------------------------------------------


def binaries(directory):
    provider = Path(directory) / "cbr-provider"
    client = Path(directory) / "cbr"
    for binary in (provider, client):
        if not binary.exists():
            refuse(f"{binary} is not built; cargo build --workspace first")
    return provider, client


def config_of(run, live, dry_answers, replay=False):
    """The configuration body a launch gets.

    A live run is a **production** configuration naming a
    `model_runtime`, which is the member a credential read comes from. A
    dry run is a **conformance** configuration carrying the fake, which
    a production launch refuses outright -- so the two can never be
    confused for one another by a flag.

    **In live mode the rebuild gets the very same body**, and the live
    run of 2026-09-22 is why. The replay used to be launched under a
    conformance configuration, whose `provider_id` defaults to
    `conformance-provider`; every citation in every rebuilt packet then
    carried a different provider from the live one, and the gate
    reported six differences the harness had created itself.
    `--replay-model` with a production configuration and no permit is
    `ServeFromRecords`: no transport, no credential, same provider id.

    A dry run cannot do the same, and the asymmetry is the provider's
    rule rather than a choice here: `--replay-model` needs a configured
    `model_runtime`, and a *serving* launch carrying one without
    `--permit-model-network` is refused. Both launches of a dry run are
    conformance either way, so both name the same provider and the gate
    is not misled.
    """
    if live:
        return {
            "format": "cbr-config/1",
            "principal": PRINCIPAL,
            "authority_principals": [PRINCIPAL],
            "model_runtime": {
                "provider": "minimax",
                "dialect": "responses",
                "model": run["model"],
            },
        }
    body = {
        "format": "combraton-conformance-config/1",
        "principal": PRINCIPAL,
        "authority_principals": [PRINCIPAL],
        "credentials": [{"credential": DRY_RUN_CREDENTIAL}],
        "context": {"compile": True},
        "model": {
            "dialect": "responses",
            "model": run["model"],
            "answers": dry_answers,
            "usage": 5000,
            "counting": "when_it_could_admit",
        },
    }
    if replay:
        # **The fake goes, and a configured model takes its place.** A
        # launch has one transport: `model.fake` beside `--replay-model`
        # is refused by name, because a rebuild a fake could answer is
        # not a rebuild. The `model_runtime` is here only so that
        # `--replay-model` is not refused for having no model
        # configured; nothing is called, since the decision is
        # `ServeFromRecords` before any transport is built.
        del body["model"]
        body["model_runtime"] = {
            "provider": "minimax",
            "dialect": "responses",
            "model": run["model"],
        }
    return body


def configuration(work, run, live, dry_answers, replay=False):
    """Where that body is written, and what a launch is pointed at.

    In live mode both launches are pointed at the same path holding the
    same bytes: one configuration, two launches, nothing that can drift
    between them.

    **Returns the path and the body together**, because a launch needs
    both -- the path to be pointed at and the body to know which
    credential it admits -- and a caller that fetched them separately
    could pair one launch's path with another's body. That is the shape
    of the defect run 2 stopped on, one level down.
    """
    body = config_of(run, live, dry_answers, replay)
    path = work / ("cbr-replay.json" if replay and not live else "cbr.json")
    path.write_text(json.dumps(body))
    return path, body

def launch(provider, work, run, config, extra):
    sockets = work / "s"
    sockets.mkdir(exist_ok=True)
    os.chmod(sockets, 0o700)
    endpoint = sockets / "cbr.sock"
    if endpoint.exists():
        endpoint.unlink()
    child = subprocess.Popen(
        [
            str(provider),
            "--data-dir",
            str(work / "data"),
            "--config",
            str(config),
            "--socket",
            str(endpoint),
            "--register-repository",
            f"{run['repository']['id']}={Path(run['repository']['path']).expanduser()}",
            *extra,
        ],
        stdin=subprocess.PIPE,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )
    started = time.monotonic()
    while True:
        try:
            with socket.socket(socket.AF_UNIX) as probe:
                probe.connect(str(endpoint))
            break
        except OSError:
            if child.poll() is not None:
                stderr = child.stderr.read().decode(errors="replace")
                refuse(f"the provider refused to start: {stderr.strip()}")
            if time.monotonic() - started > 60:
                child.kill()
                refuse("the provider never listened")
            time.sleep(0.05)
    return child, endpoint


def stop(child):
    child.kill()
    child.wait()


def admits(config):
    """The credential a launch under `config` will accept, or `None` when
    the provider issues its own at start.

    **A configuration and the credential that authenticates against it
    are one decision.** They were two until run 2 of 2026-09-22 failed
    on it: the rebuild took the credential from whether the *run* was
    live, while m4f had changed which configuration the rebuild launches
    under. A production configuration names no `credentials` member, so
    the dry-run credential it was handed was one the provider had never
    heard of, and the whole run stopped at `authentication_failed`.
    """
    credentials = config.get("credentials") or []
    return credentials[0]["credential"] if credentials else None


def credential_file(work, config):
    """Where that credential is, for a launch under `config`.

    **Derived from the configuration rather than passed beside it**, so
    there is nothing left to keep in step: a caller that has the
    configuration it is launching under cannot present the wrong
    credential for it.

    **This is the only function that decides a launch's credential.**
    There was briefly a second one, written so a test could assert the
    rule without a temporary directory, and the test asserted that one
    instead -- so a defect here, in the function every launch actually
    goes through, passed the suite that was meant to hold it. A rule
    tested at a restatement is not tested at the door.
    """
    admitted = admits(config)
    if admitted is None:
        # A production launch issues its own credential at start, which
        # is the same path `scripts/debug_launch.sh` prints.
        return work / "data" / "credentials" / PRINCIPAL
    path = work / "credential"
    path.write_text(admitted)
    return path


def cbr(client, endpoint, credential, arguments):
    return subprocess.run(
        [str(client), *arguments, "--socket", str(endpoint), "--credential-file", str(credential)],
        capture_output=True,
        text=True,
        check=False,
    )


def submit_and_settle(client, endpoint, credential, run, request):
    arguments = [
        "context",
        request,
        "--repo",
        str(Path(run["repository"]["path"]).expanduser()),
        "--repo-id",
        run["repository"]["id"],
        "--selector",
        run["selector"],
        "--task",
        run["task"],
        "--capacity",
        str(run.get("capacity", 262144)),
        "--investigation",
        str(run.get("investigation", 5)),
    ]
    for want in run["wants"]:
        arguments += ["--want", want]
    submitted = cbr(client, endpoint, credential, arguments)
    if submitted.returncode != 0:
        refuse(f"{run['id']}: submit failed: {submitted.stderr.strip()}")
    started = time.monotonic()
    while True:
        polled = cbr(client, endpoint, credential, ["request", request])
        if polled.returncode != 0:
            refuse(f"{run['id']}: inspect failed: {polled.stderr.strip()}")
        inspected = json.loads(polled.stdout)
        if inspected.get("state") != "preparing":
            return inspected
        if time.monotonic() - started > SETTLE_SECONDS:
            refuse(f"{run['id']}: {request} never left preparing")
        time.sleep(0.25)


def packet_digest(inspected):
    published = inspected.get("packets") or []
    if not published:
        return None
    return published[-1].get("reference", {}).get("artifact", {}).get("digest")


def sealed_sections(printed):
    """The sections of a sealed packet, out of a `context.packet.inspect`.

    **Not the packet's digest**, and the reason is worth stating where
    the comparison is made: a sealed packet names its own request and its
    own job, so two requests can never have equal bytes and a digest
    comparison between them would always differ. Byte identity across
    *two stores asking the same request* is the suite's own gate
    (`derivation_replay.rs`); what this gate asks of a real question is
    whether the rebuild reproduced the same content from the records.
    """
    excerpt = printed.get("excerpt", {}).get("data_base64")
    if not excerpt:
        return None
    packet = json.loads(base64.b64decode(excerpt))
    return packet.get("sections")


ABSENT = object()


def shown(value):
    """A value the report may carry, or its shape when it may not.

    The report holds digests, paths, spans, counts and costs and **no
    repository text**, which is the licence rule for the pilots. A
    differing leaf could be either: `provider` is an identifier, and an
    excerpt is somebody else's source. So a short identifier-shaped
    string is printed as it is, and everything else becomes its length
    and a digest -- which still says *that* it differs and *by how
    much*, without carrying a byte of it.
    """
    if value is ABSENT:
        return "<absent>"
    if value is None or isinstance(value, (bool, int, float)):
        return value
    text = value if isinstance(value, str) else json.dumps(value, sort_keys=True)
    if isinstance(value, str) and len(text) <= 64 and re.fullmatch(r"[A-Za-z0-9_.:/@-]*", text):
        return text
    digest = hashlib.sha256(text.encode("utf-8")).hexdigest()[:16]
    return f"<{len(text)} chars, sha256:{digest}>"


def differing_leaves(live, rebuilt, limit=32):
    """Every leaf at which two sealed section trees differ.

    **So that the next difference is diagnosable from the report.** The
    live run of 2026-09-22 reported `sections_identical: false` six
    times and said nothing else, and the one differing leaf in every
    store turned out to be `citations[].evidence.provider` -- a fact
    that took a reader opening six packets to find and that this would
    have printed.
    """
    found = []

    def walk(one, other, path):
        if len(found) >= limit:
            return
        if isinstance(one, dict) and isinstance(other, dict):
            for key in sorted(set(one) | set(other)):
                walk(one.get(key, ABSENT), other.get(key, ABSENT), f"{path}.{key}")
        elif isinstance(one, list) and isinstance(other, list):
            for at in range(max(len(one), len(other))):
                walk(
                    one[at] if at < len(one) else ABSENT,
                    other[at] if at < len(other) else ABSENT,
                    f"{path}[{at}]",
                )
        elif one != other:
            found.append(
                {"at": path.lstrip(".") or "<root>", "live": shown(one), "rebuilt": shown(other)}
            )

    walk(live, rebuilt, "")
    return found


def inspect_packet(client, endpoint, credential, run, request):
    fetched = cbr(
        client,
        endpoint,
        credential,
        ["packet", request, "--excerpt", "10000000"],
    )
    if fetched.returncode != 0:
        refuse(f"{run['id']}: packet failed: {fetched.stderr.strip()}")
    return fetched.stdout


def write_packet(client, endpoint, credential, out, run, request, digest):
    """Write the sealed packet where the reviewer can score it.

    Outside the repository, which `check` has already insisted on: the
    bytes are a pilot repository's text and the licence rule for the
    pilots is that only digests, paths, spans, counts and costs are
    committed.
    """
    if digest is None:
        return None, None
    destination = out / f"{run['id']}.packet.json"
    printed = inspect_packet(client, endpoint, credential, run, request)
    destination.write_text(printed)
    return str(destination), sealed_sections(json.loads(printed))


# ---- what the store says it spent, and what it retained -----------------


@contextlib.contextmanager
def evidence(data):
    """**The store, opened so that reading it changes nothing on disk.**

    The store is the evidence the report summarises, so reading it must not
    change it -- and SQLite's defaults do. Measured, on scratch stores:

    - a read-write connection checkpoints on close: the log is folded into
      the database, and the log and the shared-memory file are deleted;
    - `mode=ro` reads the log but rewrites the shared-memory file, where a
      reader keeps its marks, and on a store with no log it creates both;
    - `immutable=1` changes nothing and does not read the log.

    A store is found in three states, and each is read its own way:

    - **a log and a shared-memory file** -- a provider killed while a
      session held the store: `mode=ro` with a read-only shared memory,
      which reads the log and writes nothing;
    - **no log** -- the last session's connection closed cleanly, which is
      the usual case, because a provider opens a connection per session and
      holds none between them: `immutable=1`, with nothing for it to miss;
    - **a log and no shared-memory file** -- `stop` landed inside that
      close, after SQLite unlinked the shared memory and before it deleted
      the log. Any open in place would create a shared-memory file or miss
      the log, so a **private copy** of the database and its log is read
      instead, and SQLite recovers from the copied log exactly as it would
      from the original. The copy is transient and deleted on return; the
      store itself is never opened.
    """
    database = (Path(data) / "cbr.sqlite").resolve()
    log = Path(f"{database}-wal")
    shared = Path(f"{database}-shm")
    if log.exists() and shared.exists():
        connection = sqlite3.connect(f"{database.as_uri()}?mode=ro&readonly_shm=1", uri=True)
    elif not log.exists():
        connection = sqlite3.connect(f"{database.as_uri()}?immutable=1", uri=True)
    else:
        with tempfile.TemporaryDirectory(prefix="cbr-read-") as copy:
            shutil.copyfile(database, Path(copy) / "cbr.sqlite")
            shutil.copyfile(log, Path(copy) / "cbr.sqlite-wal")
            connection = sqlite3.connect(Path(copy) / "cbr.sqlite")
            try:
                yield connection
            finally:
                connection.close()
        return
    try:
        yield connection
    finally:
        connection.close()


# **The kinds the ledger counts as spend**: `Ledger::SPENT` in
# crates/cbr-provider/src/budget.rs, which
# `m4e_harness::the_harness_counts_what_the_ledger_counts` holds this to.
SPENT_KINDS = ("reservation", "usage", "unknown", "provider_exhausted", "not_sent")

# **The rows after which a store admits nothing again** (`bound_unsound`,
# `overrun`) or makes no serving count again (`count_unsound`). A run whose
# store recorded one ends the sequence: the next run's new store would start
# on the assumption that just failed (m5-settle).
STOP_KINDS = ("bound_unsound", "count_unsound", "overrun")


def spend(data):
    """Every charge in the ledger, and their total.

    Only the ledger's own spend kinds. Every other row is a note about a
    call -- which path admitted it, an anomaly, a bill above a
    reservation -- and its tokens are not a second charge: an `overrun`
    row carries the bill its settled row already counts.
    """
    with evidence(data) as connection:
        rows = connection.execute(
            "SELECT kind, tokens FROM model_ledger ORDER BY id"
        ).fetchall()
    charges = [(kind, tokens) for kind, tokens in rows if kind in SPENT_KINDS]
    return sum(tokens for _, tokens in charges), charges


def stops(data):
    """The stop kinds a store recorded, sorted; empty when it recorded none."""
    with evidence(data) as connection:
        rows = connection.execute(
            "SELECT DISTINCT kind FROM model_ledger WHERE kind IN (?, ?, ?) ORDER BY kind",
            STOP_KINDS,
        ).fetchall()
    return [kind for (kind,) in rows]


def stopped_after(run_id, result):
    """The report's `stopped` line when `result`'s store recorded a stop."""
    if not result.get("stops"):
        return None
    return (
        f"{run_id} recorded {', '.join(result['stops'])}: its store admits "
        "nothing again, and no further run is started"
    )


def object_path(data, digest):
    hexed = digest.split(":", 1)[-1]
    return data / "objects" / "sha256" / hexed[0:2] / hexed[2:4] / hexed[4:]


def records(data):
    """Every sealed, unpurged derivation record, read from the store."""
    with evidence(data) as connection:
        rows = connection.execute(
            "SELECT id, value FROM subjects "
            "WHERE kind = 'evidence.artifact' AND id LIKE 'der.%' ORDER BY id"
        ).fetchall()
    found = []
    for identifier, value in rows:
        artifact = json.loads(value)
        if artifact.get("state") != "sealed" or artifact.get("purge") is not None:
            continue
        path = object_path(data, artifact["descriptor"]["digest"])
        if not path.exists():
            continue
        found.append((identifier, json.loads(path.read_text())))
    return found


def discovery_records(data):
    """The sealed records of discovery's own two steps.

    **Reported per run so that a reviewer can see the question was
    asked.** Discovery is advisory and items come first, so "the model
    did not widen anything" and "the model was never asked" produce
    packets that read alike. The count tells them apart: two is the
    terms step and the choice, one is a flow that stopped at the terms,
    and none is a run that never reached discovery at all.
    """
    return [
        identifier
        for identifier, record in records(data)
        if record.get("question", {}).get("selector", "").startswith("discovery.")
    ]


def ambiguity(data):
    """**The replay gate's own report: what could not be replayed, and why.**

    A rebuild answers a question only when every record covering it
    agrees, and declines otherwise. In a live run that state is ordinary
    rather than exotic: a call that timed out and a successful rerun of
    the same question leave two records that disagree, and the question
    is then unreplayable for ever.

    So this does not decide anything -- the provider's rebuild already
    did that, and the digests above say whether it worked. This
    *explains* it: it groups the records by the question they answer and
    names every question whose records differ, with the answers that
    differ. A gate that quietly reported "not retained" for these would
    be hiding the one failure the operator can actually act on.
    """
    questions = {}
    for identifier, record in records(data):
        digest = record.get("question", {}).get("digest", "")
        answer = json.dumps(record.get("answer"), sort_keys=True)
        questions.setdefault(digest, []).append(
            {
                "record": identifier,
                "selector": record.get("question", {}).get("selector", ""),
                "answer": answer,
                "made_at": record.get("made_at", ""),
            }
        )
    report = []
    for digest, holders in sorted(questions.items()):
        answers = sorted({holder["answer"] for holder in holders})
        if len(answers) == 1:
            continue
        report.append(
            {
                "question": digest,
                "selector": holders[0]["selector"],
                "records": sorted(holder["record"] for holder in holders),
                "answers": answers,
                "why": (
                    "two records covering this question answer it differently, "
                    "so a rebuild declines rather than choosing which history "
                    "to reproduce"
                ),
            }
        )
    return {"questions": len(questions), "ambiguous": report}


# ---- the remedy, stated where the operator will look --------------------

REMEDY = (
    "An ambiguous question is replayable again only once one of its records "
    "is gone. Purging one is an authority's act: `evidence.purge` on the "
    "record's artifact, by a principal in `authority_principals`, in a "
    "session that negotiated `evidence.retention_control`. It destroys "
    "evidence, so it is the owner's decision and never this harness's -- "
    "and the record to purge is the failed call, never the successful one, "
    "because purging the answer that worked would leave a history nobody "
    "made. See READINESS section 6."
)


# ---- one run ------------------------------------------------------------


def one_run(run, out, provider, client, live, ceiling, checkout):
    # **Under `--out`, which `check` has already put outside this
    # repository.** A live run's store *is* the evidence -- the ledger
    # the spend is read from and the records the replay gate reads --
    # and a temp directory is internal disk that a reboot empties.
    work = out / "work" / run["id"]
    work.mkdir(parents=True, exist_ok=True)
    result = {
        "id": run["id"],
        "model": run["model"],
        "repository": run["repository"]["id"],
        "origin": checkout["origin"],
        "head": checkout["head"],
        # What this launch was actually given, which is the whole of
        # item 2: the ledger counts one store, so a run that was handed
        # the full cap would be the cap all over again.
        "launch_ceiling": ceiling,
        # What the stop priced this run at before starting it, so a
        # reader can set the run's spend beside the bound it was held to.
        "worst_case_tokens": worst_case_of(run),
        "data": str(work / "data"),
    }
    config, config_body = configuration(work, run, live, run.get("dry_answers", []))
    # **The ceiling goes to every launch, in both modes.**
    # `Ledger::run_spend` sums the store it was opened over, and
    # every run opens a new one, so a ceiling passed once per launch
    # without subtracting what is already spent bounds each run and
    # not the run. A dry run is given it too, so the mechanism the
    # live run depends on is the one the suite exercises.
    extra = ["--model-run-ceiling", str(ceiling)]
    if live:
        extra = ["--permit-model-network", *extra]
    child, endpoint = launch(provider, work, run, config, extra)
    credential = credential_file(work, config_body)
    try:
        inspected = submit_and_settle(client, endpoint, credential, run, "live")
        digest = packet_digest(inspected)
        result["items"] = [
            {
                "item": item.get("item_id"),
                "result": item.get("result"),
                "reason": item.get("reason", ""),
            }
            for item in inspected.get("items", [])
        ]
        result["packet"] = digest
        result["packet_file"], live_sections = write_packet(
            client, endpoint, credential, out, run, "live", digest
        )
    finally:
        stop(child)

    data = work / "data"
    total, charges = spend(data)
    result["tokens"] = total
    result["charges"] = [{"kind": kind, "tokens": tokens} for kind, tokens in charges]
    result["stops"] = stops(data)
    result["records"] = len(records(data))
    found = discovery_records(data)
    result["discovery_records"] = len(found)
    if len(found) != 2:
        result["discovery_note"] = (
            f"discovery sealed {len(found)} records rather than the two "
            "its flow is: this packet may be a baseline that no model "
            "widened. See READINESS section 3."
        )

    # **The replay gate.** The same store, no transport, no
    # credential, no permit: every question answered from what was
    # retained or not at all.
    # **The same configuration the live launch used.** In live mode
    # this is literally the same file; see `configuration`.
    replay_config, replay_config_body = configuration(
        work, run, live, run.get("dry_answers", []), replay=True
    )
    child, endpoint = launch(
        provider,
        work,
        run,
        replay_config,
        # The rebuild has no transport and spends nothing; it carries
        # the ceiling so that the launch every run makes is the same
        # launch, bounded the same way.
        ["--replay-model", "--model-run-ceiling", str(ceiling)],
    )
    credential = credential_file(work, replay_config_body)
    try:
        rebuilt = submit_and_settle(client, endpoint, credential, run, "replay")
        rebuilt_sections = (
            sealed_sections(
                json.loads(inspect_packet(client, endpoint, credential, run, "replay"))
            )
            if packet_digest(rebuilt)
            else None
        )
        result["replay"] = {
            "packet": packet_digest(rebuilt),
            "items": [
                {
                    "item": item.get("item_id"),
                    "result": item.get("result"),
                    "reason": item.get("reason", ""),
                }
                for item in rebuilt.get("items", [])
            ],
        }
    finally:
        stop(child)
    # **What the gate actually compares.** The sections, because the
    # packet digest covers the request's own id and two requests can
    # never be byte-identical; byte identity across two stores is the
    # suite's gate, not this one's.
    # **One answer, reported two ways.** A gate that reports only that
    # two things are unequal makes its reader do the work the gate was
    # for -- the first live run said "differs" six times and named
    # nothing, and the one differing leaf took six packets opened by
    # hand to find. So the leaves are what is computed, and "identical"
    # is derived from them: the two cannot disagree with each other.
    differences = differing_leaves(live_sections, rebuilt_sections)
    result["replay"]["differences"] = differences
    result["replay"]["sections_identical"] = live_sections is not None and not differences
    result["replay"].update(ambiguity(work / "data"))
    # **Nothing here deletes a store.** The ledger and the records
    # are what the report is a summary of, so they outlive the thing
    # that summarised them, in both modes and under `--out`.
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description="the m4e run")
    parser.add_argument("--manifest")
    parser.add_argument("--out")
    parser.add_argument(
        "--ambiguity",
        help=(
            "report what in an existing data directory cannot be replayed, "
            "and stop. The operator's question after a run, answerable "
            "without running anything again."
        ),
    )
    parser.add_argument("--binaries", default=str(REPOSITORY / "target" / "debug"))
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--live", action="store_true")
    parser.add_argument("--permit-model-network", action="store_true")
    parser.add_argument("--run-ceiling", type=int, default=RUN_CEILING_TOKENS)
    parser.add_argument(
        "--stop-tokens",
        type=int,
        default=STOP_TOKENS,
        help=(
            "lower the stop of READINESS section 9. Like the ceiling, a flag "
            "can only lower it, and it is what the next run's worst case is "
            "checked against before that run is started."
        ),
    )
    options = parser.parse_args(argv)

    if options.ambiguity:
        found = ambiguity(Path(options.ambiguity))
        print(json.dumps({**found, "remedy": REMEDY}, indent=2, sort_keys=True))
        return 0

    if not options.manifest or not options.out:
        print("m4e_run: --manifest and --out are needed for a run", file=sys.stderr)
        return 2

    if options.live == options.dry_run:
        print(
            "m4e_run: choose one of --dry-run and --live. A run that is "
            "neither is a run nobody asked for.",
            file=sys.stderr,
        )
        return 2

    try:
        manifest = json.loads(Path(options.manifest).read_text())
        out, checkouts = check(
            manifest,
            options.out,
            options.live,
            options.permit_model_network,
            options.run_ceiling,
            options.stop_tokens,
        )
        provider, client = binaries(options.binaries)
        out.mkdir(parents=True, exist_ok=True)

        print(f"m4e_run: {'LIVE' if options.live else 'dry run'}, "
              f"{len(manifest['runs'])} runs, estimate {ESTIMATE_TOKENS:,} tokens, "
              f"stop at {options.stop_tokens:,}, ceiling {options.run_ceiling:,}")

        report = {
            "mode": "live" if options.live else "dry-run",
            "estimate_tokens": ESTIMATE_TOKENS,
            "stop_tokens": options.stop_tokens,
            "run_ceiling_tokens": options.run_ceiling,
            "worst_case_flow_tokens": WORST_CASE_FLOW_TOKENS,
            "worst_case_selection_tokens": WORST_CASE_SELECTION_TOKENS,
            "remedy_for_an_ambiguous_question": REMEDY,
            "runs": [],
        }
        spent = 0
        for run in manifest["runs"]:
            # **The stop is checked before a run, against what that run
            # could cost.** Checked only afterwards it was a report of
            # an overspend rather than a bound on one: the run that
            # crossed the stop had already crossed it. The worst case is
            # a computed number (READINESS section 3), so the arithmetic
            # can be done before the tokens are.
            remaining = options.run_ceiling - spent
            worst = worst_case_of(run)
            if spent + worst > options.stop_tokens:
                report["stopped"] = (
                    f"{run['id']} was not started: {spent} spent plus the "
                    f"{priced(run)} would cross the stop of {options.stop_tokens}"
                )
                print(f"m4e_run: STOPPED. {report['stopped']}", file=sys.stderr)
                break
            if remaining < worst:
                report["stopped"] = (
                    f"{run['id']} was not started: {remaining} left under the "
                    f"ceiling of {options.run_ceiling} is less than the "
                    f"{priced(run)}"
                )
                print(f"m4e_run: STOPPED. {report['stopped']}", file=sys.stderr)
                break
            result = one_run(
                run,
                out,
                provider,
                client,
                options.live,
                # **What is left, not the cap.** The ledger counts one
                # store and every run opens a new one, so the cap for
                # the whole of m4e is only a cap if each launch is given
                # the cap less what the launches before it spent.
                remaining,
                checkouts[run["id"]],
            )
            report["runs"].append(result)
            spent += result.get("tokens", 0)
            print(
                f"  {result['id']}: {result.get('tokens', 0):,} tokens, "
                f"{result.get('records', 0)} records "
                f"({result.get('discovery_records', 0)} from discovery), "
                f"ceiling {result.get('launch_ceiling', 0):,}, "
                f"replay {'identical' if result.get('replay', {}).get('sections_identical') else 'DIFFERS'}, "
                f"{len(result.get('replay', {}).get('ambiguous', []))} ambiguous"
            )
            if stopped_after(run["id"], result):
                report["stopped"] = stopped_after(run["id"], result)
                print(f"m4e_run: STOPPED. {report['stopped']}", file=sys.stderr)
                break
        report["tokens"] = spent
        report["ambiguous_questions"] = sum(
            len(run.get("replay", {}).get("ambiguous", [])) for run in report["runs"]
        )

        (out / "report.json").write_text(json.dumps(report, indent=2, sort_keys=True))
        print(f"m4e_run: {spent:,} tokens over {len(report['runs'])} runs; "
              f"{report['ambiguous_questions']} ambiguous questions; "
              f"report {out / 'report.json'}")
        if report["ambiguous_questions"]:
            print(f"m4e_run: {REMEDY}", file=sys.stderr)
        return 0
    except Refused as refusal:
        print(f"m4e_run: {refusal}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
