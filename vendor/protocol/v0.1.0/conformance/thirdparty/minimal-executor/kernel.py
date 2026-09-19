#!/usr/bin/env python3
"""minimal-executor: an independent execution kernel (client only).

Not an execution/1 provider: it serves no operation and claims no Execution
conformance. It evaluates its own dispatch boundary for each work item from
public Context and Evidence operations, and publishes check and dispatch
records as sealed Evidence artifacts.

Written from docs/spec (CORE, STREAM, ENCODING, EVIDENCE, CONTEXT section 14),
schemas/** and conformance/thirdparty/README.md only.
"""

import argparse
import os
import re
import sys
import threading

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(os.path.dirname(HERE), "common"))

import combraton_client as cc  # noqa: E402

CALLER = "thirdparty-minimal-executor"
MUTANTS = {"ignores-required-boundary", "trusts-advertised-digest", "ignores-claim-invalidation"}
CONFIG_FORMAT = "combraton-thirdparty-kernel-config/1"
CHECK_FORMAT = "combraton-thirdparty-kernel-check/1"
DISPATCH_FORMAT = "combraton-thirdparty-kernel-dispatch/1"
CONTEXT_FEATURES = ["context.required_before_start", "context.advisory", "context.claims"]

log = cc.Log("kernel")


class ConfigError(Exception):
    pass


def load_config(path):
    try:
        with open(path, "rb") as f:
            cfg = cc.strict_loads(f.read())
    except (OSError, cc.DecodeError) as e:
        raise ConfigError("cannot read configuration: %s" % e.__class__.__name__)
    if not isinstance(cfg, dict) or cfg.get("format") != CONFIG_FORMAT:
        raise ConfigError("configuration format is not %s" % CONFIG_FORMAT)
    for key in ("principal", "clock_file", "providers", "context", "packet_evidence",
                "records", "work"):
        if key not in cfg:
            raise ConfigError("configuration lacks %s" % key)
    providers = cfg["providers"]
    if not isinstance(providers, dict):
        raise ConfigError("providers is not an object")
    for pid, entry in providers.items():
        if not isinstance(entry, dict) or not isinstance(entry.get("socket"), str) \
                or not isinstance(entry.get("credential"), str):
            raise ConfigError("provider %s needs socket and credential" % pid)
        log.add_secret(entry["credential"])
    for use in ("context", "records"):
        u = cfg[use]
        if not isinstance(u, dict) or u.get("provider") not in providers \
                or not isinstance(u.get("grant"), str):
            raise ConfigError("%s needs a known provider and a grant" % use)
    if not isinstance(cfg["packet_evidence"], dict):
        raise ConfigError("packet_evidence is not an object")
    if not isinstance(cfg["work"], list):
        raise ConfigError("work is not a list")
    seen = set()
    for item in cfg["work"]:
        if not isinstance(item, dict):
            raise ConfigError("work item is not an object")
        wid = item.get("work_id")
        if not isinstance(wid, str) or not re.match(r"^[a-z0-9-]{1,48}$", wid) or wid in seen:
            raise ConfigError("invalid or duplicate work_id")
        seen.add(wid)
        if cc.parse_instant(item.get("dispatch_at")) is None:
            raise ConfigError("work %s has no valid dispatch_at" % wid)
        binding = item.get("binding")
        if not isinstance(binding, dict) or not isinstance(binding.get("packet"), dict):
            raise ConfigError("work %s has no packet binding" % wid)
    poll = cfg.get("poll_ms", 25)
    if not isinstance(poll, int) or poll <= 0:
        raise ConfigError("poll_ms must be a positive integer")
    return cfg


class Kernel:
    def __init__(self, cfg, mutant=None):
        self.cfg = cfg
        self.mutant = mutant
        self.clock = cc.ClockFile(cfg["clock_file"])
        self.poll = cfg.get("poll_ms", 25) / 1000.0
        self.providers = cfg["providers"]
        self.sessions = {}
        self.items = []
        for w in cfg["work"]:
            self.items.append({
                "work": w,
                "due": cc.parse_instant(w["dispatch_at"]),
                "dispatched": False,
                "n": 0,
                "last": None,        # (state, decision) of the previous check
                "pending": [],       # records to publish, in order
            })

    # -- sessions: one per provider per purpose ---------------------------

    def session(self, purpose, provider_id, profiles):
        key = (purpose, provider_id)
        s = self.sessions.get(key)
        if s is None:
            entry = self.providers.get(provider_id)
            if entry is None:
                raise KeyError(provider_id)
            s = cc.Session(log, "%s@%s" % (purpose, provider_id), entry["socket"],
                           entry["credential"], profiles, CALLER)
            self.sessions[key] = s
        return s

    # Every profile operation carries a grant, which needs core.grants (TP-1).
    def context_session(self):
        return self.session("context", self.cfg["context"]["provider"],
                            {"core": {"major": 1, "features": ["core.grants"]},
                             "context": {"major": 1, "features": CONTEXT_FEATURES}})

    def packet_session(self, provider_id):
        return self.session("packet-evidence", provider_id,
                            {"core": {"major": 1, "features": ["core.grants"]},
                             "evidence": {"major": 1}})

    def records_session(self):
        return self.session("records", self.cfg["records"]["provider"],
                            {"core": {"major": 1, "features": ["core.grants"]},
                             "evidence": {"major": 1}})

    def close(self):
        for s in self.sessions.values():
            s.close()

    # -- evaluation -------------------------------------------------------

    def evaluate(self, binding):
        """Returns (state, reasons) by the first rule that applies."""
        packet = binding.get("packet") or {}
        artifact = packet.get("artifact") if isinstance(packet.get("artifact"), dict) else {}
        provider_id = artifact.get("provider")
        art_subject = artifact.get("artifact") if isinstance(artifact.get("artifact"), dict) else {}
        digest = artifact.get("digest")
        packet_subject = packet.get("packet") if isinstance(packet.get("packet"), dict) else {}
        revision = packet.get("revision")

        # Fetch the exact bytes at the artifact's provider and hash them.
        #  1. Wrong bytes (own hash differs, or artifact_digest_mismatch): unsatisfied.
        #  2. Bytes not fetched for any other reason: unknown.
        grant = self.cfg["packet_evidence"].get(provider_id) if isinstance(provider_id, str) else None
        if provider_id not in self.providers or not isinstance(grant, str):
            return "unknown", ["packet bytes not fetched: no connection or grant for their provider"]
        if art_subject.get("kind") != "evidence.artifact" or not isinstance(art_subject.get("id"), str) \
                or not isinstance(digest, str):
            return "unknown", ["packet bytes not fetched: reference names no artifact and digest"]
        advertised = []
        try:
            data = cc.fetch_exact(self.packet_session(provider_id), grant, art_subject["id"], digest,
                                  advertised=advertised)
        except cc.ProtocolError as e:
            if e.code == "artifact_digest_mismatch":
                return "unsatisfied", ["packet bytes refused: artifact_digest_mismatch"]
            return "unknown", ["packet bytes not fetched: %s" % e.code]
        except (cc.TransportError, cc.FetchError, cc.NegotiationError, ValueError) as e:
            return "unknown", ["packet bytes not fetched: %s" % e]
        if self.mutant == "trusts-advertised-digest":
            # Deliberately broken: bytes returned for the reference's digest count as held.
            if any(d != digest for d in advertised):
                return "unsatisfied", ["provider advertised another digest"]
        elif not cc.digest_matches_bytes(digest, data):
            return "unsatisfied", ["fetched packet bytes do not match the reference digest"]

        # 2. Facts unreadable, or naming another artifact or digest.
        if packet_subject.get("kind") != "context.packet" or not isinstance(packet_subject.get("id"), str) \
                or not isinstance(revision, int):
            return "unknown", ["packet reference names no packet revision"]
        try:
            facts = self.context_session().query(
                "context.packet.inspect",
                {"packet": packet_subject["id"], "revision": revision, "max_bytes": 1},
                grant=self.cfg["context"]["grant"])
        except cc.ProtocolError as e:
            return "unknown", ["packet facts unreadable: %s" % e.code]
        except (cc.TransportError, cc.NegotiationError) as e:
            return "unknown", ["packet facts unreadable: %s" % e]
        reference = facts.get("reference")
        if not isinstance(reference, dict) or cc.canonical_bytes(reference) != cc.canonical_bytes(packet):
            return "unknown", ["packet facts name a different packet reference"]

        obligations = {}
        results = {}
        for it in facts.get("items") or []:
            if isinstance(it, dict) and isinstance(it.get("item_id"), str):
                obligations[it["item_id"]] = it.get("obligation")
                results[it["item_id"]] = it.get("result")

        def not_advisory(item_id):
            # An item whose obligation is not stated is not known to be advisory.
            return obligations.get(item_id) != "advisory"

        # Deliberately broken mutant: read-time claim facts are ignored.
        ignore_claims = self.mutant == "ignores-claim-invalidation"
        invalidated = [] if ignore_claims else (facts.get("invalidated_items") or [])
        unverified_facts = [] if ignore_claims else (facts.get("unverified_items") or [])

        # 3. Stale.
        stale = [e.get("item_id") for e in invalidated
                 if isinstance(e, dict) and not_advisory(e.get("item_id"))]
        if stale:
            return "stale", ["invalidated required item: %s" % i for i in stale]

        # 4. Unknown.
        unverified = [e.get("item_id") for e in unverified_facts
                      if isinstance(e, dict) and not_advisory(e.get("item_id"))]
        unsatisfied = [i for i, ob in obligations.items()
                       if ob != "advisory" and results.get(i) != "satisfied"]
        if unverified or unsatisfied:
            return "unknown", (["unverified required item: %s" % i for i in unverified]
                               + ["required item not satisfied: %s" % i for i in unsatisfied])

        # 5. Current.
        return "current", []

    @staticmethod
    def decide(binding, state):
        obligation = binding.get("obligation")
        if obligation == "required_before_start":
            return ("dispatch" if state == "current" else "withhold"), False, None
        if obligation == "advisory":
            if binding.get("fallback") == "proceed_with_gap":
                return "dispatch", state != "current", None
            return "withhold", False, "advisory fallback not supported by this kernel"
        return "withhold", False, "obligation not supported by this kernel"

    # -- records ----------------------------------------------------------

    def descriptor(self, work_id, now, principal):
        return cc.test_descriptor("application/json", "thirdparty.kernel", work_id, principal, now)

    def publish_pending(self, item):
        """Publishes queued records in order. Returns True when none remain."""
        while item["pending"]:
            rec = item["pending"][0]
            session = self.records_session()
            try:
                session.ensure()
                fields = self.descriptor(item["work"]["work_id"], rec["now"], session.principal)
                seal = cc.publish_artifact(
                    session, self.cfg["records"]["grant"], rec["artifact"], rec["bytes"], fields,
                    "tp-kernel", log=log)
            except (cc.ProtocolError, cc.TransportError, cc.NegotiationError) as e:
                log("record %s not published yet: %s" % (rec["artifact"], e))
                return False
            outcome = seal.get("outcome") or {}
            if outcome.get("state") != "sealed" or outcome.get("digest") != cc.digest_of_bytes(rec["bytes"]):
                log("record %s: seal outcome does not match the published bytes" % rec["artifact"])
                return False
            log("published %s" % rec["artifact"])
            item["pending"].pop(0)
            if rec.get("on_published"):
                rec["on_published"]()
        return True

    def step(self, item, now):
        work = item["work"]
        wid = work["work_id"]
        binding = work["binding"]
        state, reasons = self.evaluate(binding)
        decision, gap, note = self.decide(binding, state)
        if note:
            reasons = reasons + [note]
        dispatch = decision == "dispatch"
        if self.mutant == "ignores-required-boundary":
            # Deliberately broken: honest check record, dispatch regardless.
            dispatch = True
            gap = state != "current"
        key = (state, decision)
        check_n = item["n"]
        if key != item["last"]:
            item["n"] += 1
            check_n = item["n"]
            item["last"] = key
            record = {
                "format": CHECK_FORMAT,
                "work_id": wid,
                "boundary": "dispatch",
                "checked_at": cc.format_instant(now),
                "packet": binding.get("packet"),
                "state": state,
                "decision": decision,
                "reasons": reasons,
            }
            item["pending"].append({"artifact": "check.%s.%d" % (wid, check_n),
                                    "bytes": cc.canonical_bytes(record), "now": now})
            log("check %s.%d: state %s, decision %s" % (wid, check_n, state, decision))
        if dispatch:
            record = {
                "format": DISPATCH_FORMAT,
                "work_id": wid,
                "dispatched_at": cc.format_instant(now),
                "obligation": binding.get("obligation"),
                "gap": bool(gap),
                "check": check_n,
            }

            def mark():
                item["dispatched"] = True
                log("dispatched %s (gap %s)" % (wid, bool(gap)))

            item["pending"].append({"artifact": "dispatch.%s" % wid,
                                    "bytes": cc.canonical_bytes(record), "now": now,
                                    "on_published": mark})
            item["dispatch_queued"] = True

    def run(self, stop):
        log("started with %d work items%s" % (
            len(self.items), " (mutant %s)" % self.mutant if self.mutant else ""))
        while not stop.is_set():
            now = self.clock.now()
            if now is not None:
                for item in self.items:
                    if stop.is_set():
                        break
                    if item["dispatched"]:
                        continue
                    if item["pending"]:
                        self.publish_pending(item)
                        continue
                    if item.get("dispatch_queued"):
                        continue
                    if now < item["due"]:
                        continue
                    self.step(item, now)
                    self.publish_pending(item)
            stop.wait(self.poll)
        self.close()
        log("input ended; exiting")


def watch_stdin(stop):
    try:
        while True:
            chunk = sys.stdin.buffer.read1(65536) if hasattr(sys.stdin.buffer, "read1") \
                else sys.stdin.buffer.read(1)
            if not chunk:
                break
    except Exception:
        pass
    stop.set()


def main(argv):
    parser = argparse.ArgumentParser(add_help=True)
    parser.add_argument("--config", required=True)
    parser.add_argument("--mutant")
    args = parser.parse_args(argv)
    if args.mutant is not None and args.mutant not in MUTANTS:
        log("unknown mutant")
        return 2
    try:
        cfg = load_config(args.config)
    except ConfigError as e:
        log(str(e))
        return 2
    stop = threading.Event()
    threading.Thread(target=watch_stdin, args=(stop,), daemon=True).start()
    Kernel(cfg, args.mutant).run(stop)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
