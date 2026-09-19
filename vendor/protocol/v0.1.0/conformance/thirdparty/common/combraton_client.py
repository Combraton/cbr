"""Minimal Combraton Protocol 0.1 client pieces, standard library only.

Written from docs/spec/bindings/STREAM.md (framing, JSON-RPC mapping, Unix
socket form), docs/spec/bindings/ENCODING.md (value domain, RFC 8785 canonical
form, digest strings), docs/spec/profiles/CORE.md (sections 4-6, 10-12, 15,
18) and the schemas under schemas/core/1 and schemas/stream/1.

Nothing here is shared with any other implementation.
"""

import base64
import datetime
import hashlib
import json
import os
import re
import secrets
import socket
import sys

DEFAULT_FRAME_LIMIT = 1048576  # STREAM section 1.5, before negotiation
IDENTIFIER_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:~-]{0,127}$")
INSTANT_RE = re.compile(
    r"^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(\.\d+)?(Z|[+-]00:00)$"
)
MAX_SAFE = 2**53 - 1


# ---------------------------------------------------------------------------
# Diagnostics. Credentials never reach a log line (CORE section 18.1).
# ---------------------------------------------------------------------------

class Log:
    def __init__(self, name):
        self.name = name
        self._secrets = []

    def add_secret(self, value):
        if isinstance(value, str) and value:
            self._secrets.append(value)

    def __call__(self, message):
        text = str(message)
        for s in self._secrets:
            if s in text:
                text = text.replace(s, "<redacted>")
        # Collapse identical consecutive lines, such as a retry at every poll.
        if text == getattr(self, "_last", None):
            self._repeats = getattr(self, "_repeats", 0) + 1
            return
        lines = []
        if getattr(self, "_repeats", 0):
            lines.append("[%s] (previous line repeated %d times)\n" % (self.name, self._repeats))
        self._last, self._repeats = text, 0
        lines.append("[%s] %s\n" % (self.name, text))
        try:
            sys.stderr.write("".join(lines))
            sys.stderr.flush()
        except Exception:
            pass


# ---------------------------------------------------------------------------
# ENCODING section 1: strict value domain on parse.
# ---------------------------------------------------------------------------

class DecodeError(Exception):
    pass


def _no_dupes(pairs):
    obj = {}
    for k, v in pairs:
        if k in obj:
            raise DecodeError("duplicate member name")
        obj[k] = v
    return obj


def _reject_float(text):
    raise DecodeError("non-integer number")


def _reject_constant(text):
    raise DecodeError("non-JSON constant")


def _parse_int(text):
    if text.startswith("-0") or (len(text) > 1 and text[0] == "0"):
        raise DecodeError("leading zero or -0")
    value = int(text)
    if value > MAX_SAFE or value < -MAX_SAFE:
        raise DecodeError("integer out of range")
    return value


def _check_strings(value):
    if isinstance(value, str):
        for ch in value:
            cp = ord(ch)
            if 0xD800 <= cp <= 0xDFFF:
                raise DecodeError("surrogate in string")
            if 0xFDD0 <= cp <= 0xFDEF or (cp & 0xFFFE) == 0xFFFE:
                raise DecodeError("noncharacter in string")
    elif isinstance(value, dict):
        for k, v in value.items():
            _check_strings(k)
            _check_strings(v)
    elif isinstance(value, list):
        for v in value:
            _check_strings(v)


def strict_loads(data):
    """Parse one JSON text under ENCODING section 1."""
    if isinstance(data, (bytes, bytearray)):
        try:
            text = bytes(data).decode("utf-8", errors="strict")
        except UnicodeDecodeError:
            raise DecodeError("invalid UTF-8")
    else:
        text = data
    try:
        value = json.loads(
            text,
            object_pairs_hook=_no_dupes,
            parse_float=_reject_float,
            parse_int=_parse_int,
            parse_constant=_reject_constant,
        )
    except DecodeError:
        raise
    except ValueError as e:
        raise DecodeError("not a JSON text: %s" % e.__class__.__name__)
    _check_strings(value)
    return value


# ---------------------------------------------------------------------------
# ENCODING section 2: canonical form (RFC 8785 restricted to integers).
# ---------------------------------------------------------------------------

_SHORT_ESCAPES = {0x08: "\\b", 0x09: "\\t", 0x0A: "\\n", 0x0C: "\\f", 0x0D: "\\r"}


def _canon_string(s):
    out = ['"']
    for ch in s:
        cp = ord(ch)
        if ch == '"':
            out.append('\\"')
        elif ch == "\\":
            out.append("\\\\")
        elif cp < 0x20:
            out.append(_SHORT_ESCAPES.get(cp) or "\\u%04x" % cp)
        else:
            if 0xD800 <= cp <= 0xDFFF:
                raise ValueError("unpaired surrogate cannot be canonicalized")
            out.append(ch)
    out.append('"')
    return "".join(out)


def _utf16_key(name):
    # Sorting UTF-16BE bytes orders by UTF-16 code units as unsigned integers.
    return name.encode("utf-16-be")


def _canon(value, out):
    if value is None:
        out.append("null")
    elif value is True:
        out.append("true")
    elif value is False:
        out.append("false")
    elif isinstance(value, int):
        if value > MAX_SAFE or value < -MAX_SAFE:
            raise ValueError("integer outside the value domain")
        out.append(str(value))
    elif isinstance(value, float):
        raise ValueError("non-integer numbers are outside the value domain")
    elif isinstance(value, str):
        out.append(_canon_string(value))
    elif isinstance(value, (list, tuple)):
        out.append("[")
        for i, v in enumerate(value):
            if i:
                out.append(",")
            _canon(v, out)
        out.append("]")
    elif isinstance(value, dict):
        out.append("{")
        for i, k in enumerate(sorted(value.keys(), key=_utf16_key)):
            if not isinstance(k, str):
                raise ValueError("member names must be strings")
            if i:
                out.append(",")
            out.append(_canon_string(k))
            out.append(":")
            _canon(value[k], out)
        out.append("}")
    else:
        raise ValueError("value outside the JSON domain: %r" % type(value))


def canonical_bytes(value):
    out = []
    _canon(value, out)
    return "".join(out).encode("utf-8")


# ---------------------------------------------------------------------------
# ENCODING section 3/4: digest strings.
# ---------------------------------------------------------------------------

_ALGORITHMS = {"sha256": (hashlib.sha256, 64), "sha512": (hashlib.sha512, 128)}


def digest_of_bytes(data, algorithm="sha256"):
    fn, _ = _ALGORITHMS[algorithm]
    return "%s:%s" % (algorithm, fn(bytes(data)).hexdigest())


def digest_matches_bytes(expected, data):
    """True only when `expected` is a well-formed digest we can compute and
    it equals the digest of exactly `data`."""
    if not isinstance(expected, str) or ":" not in expected:
        return False
    algorithm, _, hexpart = expected.partition(":")
    spec = _ALGORITHMS.get(algorithm)
    if spec is None or len(hexpart) != spec[1]:
        return False
    return digest_of_bytes(data, algorithm) == expected


def command_digest(operation, subject, preconditions, requires, payload,
                   required_extensions=None):
    intent = {
        "operation": operation,
        "subject": subject,
        "preconditions": preconditions,
        "requires": requires,
        "payload": payload,
        "extensions": required_extensions or {},
    }
    return digest_of_bytes(canonical_bytes(intent), "sha256")


def make_identifier(*parts):
    """A Core identifier derived deterministically from its parts. Falls back
    to a digest of the parts when they do not fit the identifier grammar."""
    text = ".".join(str(p) for p in parts)
    if IDENTIFIER_RE.match(text):
        return text
    return "id-" + hashlib.sha256(text.encode("utf-8")).hexdigest()[:48]


# ---------------------------------------------------------------------------
# Clock file (harness interface: RFC 3339 UTC written by the runner).
# ---------------------------------------------------------------------------

def parse_instant(text):
    if not isinstance(text, str):
        return None
    m = INSTANT_RE.match(text.strip())
    if not m:
        return None
    try:
        dt = datetime.datetime(
            int(m.group(1)), int(m.group(2)), int(m.group(3)),
            int(m.group(4)), int(m.group(5)), int(m.group(6)),
            tzinfo=datetime.timezone.utc,
        )
    except ValueError:
        return None
    frac = m.group(7)
    if frac:
        micro = int((frac[1:] + "000000")[:6])
        dt = dt.replace(microsecond=micro)
    return dt


def format_instant(dt):
    """CORE instant: second precision UTC."""
    return dt.astimezone(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


class ClockFile:
    """Reads 'now' from the controlled clock file. A missing or malformed
    file keeps the last good instant; the client never uses its own clock."""

    def __init__(self, path):
        self.path = path
        self.last = None

    def now(self):
        try:
            with open(self.path, "r", encoding="utf-8") as f:
                dt = parse_instant(f.read())
            if dt is not None:
                self.last = dt
        except OSError:
            pass
        return self.last


# ---------------------------------------------------------------------------
# STREAM: framed JSON-RPC over a Unix-domain socket.
# ---------------------------------------------------------------------------

class TransportError(Exception):
    """The connection failed or the peer broke the binding. Nothing about a
    command in flight is known (STREAM section 4)."""


class ProtocolError(Exception):
    """A JSON-RPC error response with Core error data (CORE section 12)."""

    def __init__(self, operation, rpc_code, data):
        self.operation = operation
        self.rpc_code = rpc_code
        self.data = data if isinstance(data, dict) else {}
        self.code = self.data.get("code")
        self.retry = self.data.get("retry")
        self.details = self.data.get("details") if isinstance(self.data.get("details"), dict) else {}
        hint = ""
        for key in ("path", "reason", "limit", "features", "computed", "received"):
            if key in self.details:
                hint += " %s=%s" % (key, json.dumps(self.details[key], sort_keys=True))
        Exception.__init__(self, "%s refused: %s%s" % (operation, self.code, hint))


class Connection:
    def __init__(self, path, timeout=5.0):
        self.path = path
        self.timeout = timeout
        self.sock = None
        self.buffer = bytearray()
        self.receive_limit = DEFAULT_FRAME_LIMIT
        self.send_limit = DEFAULT_FRAME_LIMIT
        self.next_id = 1

    def open(self):
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.settimeout(self.timeout)
        try:
            s.connect(self.path)
        except OSError as e:
            s.close()
            raise TransportError("connect failed: %s" % e.__class__.__name__)
        self.sock = s

    def close(self):
        if self.sock is not None:
            try:
                self.sock.close()
            except OSError:
                pass
        self.sock = None
        self.buffer = bytearray()

    def _send_frame(self, obj):
        data = json.dumps(obj, ensure_ascii=False, separators=(",", ":"),
                          allow_nan=False).encode("utf-8")
        if b"\n" in data:
            raise TransportError("serialized frame contains a line feed")
        if len(data) > self.send_limit:
            raise TransportError("frame exceeds the peer's frame limit")
        try:
            self.sock.sendall(data + b"\n")
        except OSError as e:
            raise TransportError("write failed: %s" % e.__class__.__name__)

    def _read_frame(self):
        while True:
            nl = self.buffer.find(b"\n")
            if nl >= 0:
                frame = bytes(self.buffer[:nl])
                del self.buffer[: nl + 1]
                if len(frame) > self.receive_limit:
                    raise TransportError("received frame exceeds limit")
                if frame.strip(b" \t\r") == b"":
                    continue
                return frame
            if len(self.buffer) > self.receive_limit + 1:
                raise TransportError("received frame exceeds limit")
            try:
                chunk = self.sock.recv(65536)
            except socket.timeout:
                raise TransportError("read timed out")
            except OSError as e:
                raise TransportError("read failed: %s" % e.__class__.__name__)
            if not chunk:
                raise TransportError("connection closed by peer")
            self.buffer.extend(chunk)

    def call(self, method, params):
        if self.sock is None:
            raise TransportError("not connected")
        rid = self.next_id
        self.next_id += 1
        self._send_frame({"jsonrpc": "2.0", "id": rid, "method": method, "params": params})
        while True:
            raw = self._read_frame()
            try:
                msg = strict_loads(raw)
            except DecodeError as e:
                raise TransportError("peer sent an invalid frame: %s" % e)
            if not isinstance(msg, dict) or msg.get("jsonrpc") != "2.0":
                raise TransportError("peer sent a non JSON-RPC 2.0 object")
            if "id" not in msg:
                continue  # provider notification; this client subscribes to nothing
            if msg["id"] is None and "error" in msg:
                data = msg["error"].get("data") if isinstance(msg.get("error"), dict) else None
                raise TransportError("frame-level error from peer: %s" %
                                     (data.get("code") if isinstance(data, dict) else "?"))
            if msg["id"] != rid:
                continue
            if "error" in msg:
                err = msg["error"] if isinstance(msg["error"], dict) else {}
                raise ProtocolError(method, err.get("code"), err.get("data"))
            result = msg.get("result")
            if not isinstance(result, dict):
                raise TransportError("response result is not an object")
            return result


# ---------------------------------------------------------------------------
# Documented dependencies, used only when core.feature_dependencies answers
# method_not_found (CORE section 4.3). Sources:
#   EVIDENCE section 1, CONTEXT section 1, KNOWLEDGE section 1:
#       profile -> core/1 with core.events
#   VERIFICATION section 1: core/1 with core.events, core.capabilities
#   EXECUTION section 1: core/1 with core.events, core.capabilities, core.effects
#   EXECUTION section 13.3: execution.claim_revalidation -> execution.context_revalidation
# ---------------------------------------------------------------------------

DOCUMENTED_DEPENDENCIES = [
    {"profile": "evidence", "major": 1, "trigger": {"kind": "profile"},
     "requires": [{"profile": "core", "major": 1, "features": ["core.events"]}]},
    {"profile": "context", "major": 1, "trigger": {"kind": "profile"},
     "requires": [{"profile": "core", "major": 1, "features": ["core.events"]}]},
    {"profile": "knowledge", "major": 1, "trigger": {"kind": "profile"},
     "requires": [{"profile": "core", "major": 1, "features": ["core.events"]}]},
    {"profile": "verification", "major": 1, "trigger": {"kind": "profile"},
     "requires": [{"profile": "core", "major": 1,
                   "features": ["core.events", "core.capabilities"]}]},
    {"profile": "execution", "major": 1, "trigger": {"kind": "profile"},
     "requires": [{"profile": "core", "major": 1,
                   "features": ["core.events", "core.capabilities", "core.effects"]}]},
    {"profile": "execution", "major": 1,
     "trigger": {"kind": "feature", "feature": "execution.claim_revalidation"},
     "requires": [{"profile": "execution", "major": 1,
                   "features": ["execution.context_revalidation"]}]},
]


class NegotiationError(Exception):
    pass


def close_over_dependencies(wanted, dependencies):
    """wanted: {profile: {"major": int, "features": set}}. Adds every
    dependency of every requested profile and feature, to a fixed point."""
    changed = True
    while changed:
        changed = False
        for entry in dependencies:
            if not isinstance(entry, dict):
                continue
            p = wanted.get(entry.get("profile"))
            if p is None or p["major"] != entry.get("major"):
                continue
            trig = entry.get("trigger") or {}
            if trig.get("kind") == "profile":
                applies = True
            elif trig.get("kind") == "feature":
                applies = trig.get("feature") in p["features"]
            else:
                # An unknown trigger kind cannot be proven not to apply.
                applies = True
            if not applies:
                continue
            for req in entry.get("requires") or []:
                name, major = req.get("profile"), req.get("major")
                q = wanted.get(name)
                if q is None:
                    q = {"major": major, "features": set()}
                    wanted[name] = q
                    changed = True
                elif q["major"] != major:
                    raise NegotiationError(
                        "dependency needs %s/%s but %s/%s is requested"
                        % (name, major, name, q["major"]))
                for f in req.get("features") or []:
                    if f not in q["features"]:
                        q["features"].add(f)
                        changed = True
    return wanted


# ---------------------------------------------------------------------------
# A Core session: authenticate, feature dependencies, negotiate; then queries
# and commands with real command identity.
# ---------------------------------------------------------------------------

class Session:
    def __init__(self, log, label, socket_path, credential, profiles,
                 caller_name, caller_version="0.1.0", receive_limit=4 * 1048576,
                 timeout=5.0):
        """profiles: {name: {"major": 1, "features": [...]}}; every listed
        profile and feature is required. core is always included."""
        self.log = log
        self.label = label
        self.socket_path = socket_path
        self._credential = credential
        log.add_secret(credential)
        self.profiles = profiles
        self.caller = {"name": caller_name, "version": caller_version}
        self.receive_limit = receive_limit
        self.timeout = timeout
        self.conn = None
        self.principal = None
        self.negotiation = None
        self.generation = None
        self.limits = None
        self._nonce = secrets.token_hex(6)
        self._messages = 0
        self._generations = {}  # command_id -> dedupe_generation first issued

    # -- lifecycle ---------------------------------------------------------

    @property
    def connected(self):
        return self.conn is not None and self.negotiation is not None

    def close(self):
        if self.conn is not None:
            self.conn.close()
        self.conn = None
        self.negotiation = None

    def ensure(self):
        if not self.connected:
            self.connect()

    def connect(self):
        self.close()
        conn = Connection(self.socket_path, timeout=self.timeout)
        conn.open()
        self.conn = conn
        try:
            self._handshake()
        except Exception:
            self.close()
            raise

    def _message_id(self):
        self._messages += 1
        return "m-%s-%d" % (self._nonce, self._messages)

    def _handshake(self):
        # CORE section 18.2: authenticate first on a shared transport.
        auth = self.conn.call("core.authenticate", {
            "operation": "core.authenticate",
            "message_id": self._message_id(),
            "payload": {"credential": self._credential},
        })
        self.principal = auth.get("principal")
        if not isinstance(self.principal, str):
            raise TransportError("authenticate result names no principal")

        wanted = {}
        for name, spec in self.profiles.items():
            wanted[name] = {"major": spec["major"], "features": set(spec.get("features", []))}
        wanted.setdefault("core", {"major": 1, "features": set()})

        # CORE section 4.3: query dependencies, fall back to the documents.
        try:
            deps = self.conn.call("core.feature_dependencies", {
                "operation": "core.feature_dependencies",
                "message_id": self._message_id(),
                "payload": {},
            })
            dependencies = deps.get("dependencies")
            if not isinstance(dependencies, list):
                raise TransportError("feature_dependencies result has no list")
            source = "provider"
        except ProtocolError as e:
            if e.code != "method_not_found":
                raise
            dependencies = DOCUMENTED_DEPENDENCIES
            source = "documents"
        close_over_dependencies(wanted, dependencies)

        request = []
        order = ["core"] + sorted(n for n in wanted if n != "core")
        for name in order:
            spec = wanted[name]
            request.append({
                "name": name,
                "majors": [spec["major"]],
                "required": True,
                "required_features": sorted(spec["features"]),
                "optional_features": [],
            })
        result = self.conn.call("core.negotiate", {
            "operation": "core.negotiate",
            "message_id": self._message_id(),
            "payload": {
                "caller": self.caller,
                "receive_limits": {"max_frame_bytes": self.receive_limit},
                "profiles": request,
            },
        })
        selected = {s.get("name"): s for s in result.get("selected", []) if isinstance(s, dict)}
        for entry in request:
            s = selected.get(entry["name"])
            if s is None or s.get("major") not in entry["majors"]:
                raise NegotiationError("profile %s was not selected" % entry["name"])
            missing = set(entry["required_features"]) - set(s.get("features") or [])
            if missing:
                raise NegotiationError("features not selected: %s" % sorted(missing))
        self.negotiation = result
        self.selected = selected
        self.limits = result.get("limits") or {}
        window = result.get("dedupe_window") or {}
        self.generation = window.get("current", 0)
        # STREAM section 1.5: limits after negotiation.
        self.conn.receive_limit = self.receive_limit
        self.conn.send_limit = max(DEFAULT_FRAME_LIMIT,
                                   int(self.limits.get("max_frame_bytes", DEFAULT_FRAME_LIMIT)))
        self.log("%s: session as %s negotiated %s (dependencies from %s)" % (
            self.label, self.principal,
            ", ".join("%s/%d[%s]" % (e["name"], e["majors"][0], ",".join(e["required_features"]))
                      for e in request),
            source))

    def feature_selected(self, feature):
        profile = feature.split(".", 1)[0]
        s = (self.selected or {}).get(profile)
        return bool(s) and feature in (s.get("features") or [])

    # -- operations -------------------------------------------------------

    def query(self, operation, payload, grant=None):
        self.ensure()
        params = {"operation": operation, "message_id": self._message_id(), "payload": payload}
        if grant is not None:
            params["grant"] = grant
        try:
            return self.conn.call(operation, params)
        except TransportError:
            self.close()
            raise

    def command(self, operation, command_id, subject, preconditions, payload,
                grant=None, requires=None):
        """Sends a command with real command identity. A retransmission of
        the same command_id keeps its content and first generation."""
        self.ensure()
        requires = list(requires or [])
        digest = command_digest(operation, subject, preconditions, requires, payload)
        generation = self._generations.setdefault(command_id, self.generation)
        params = {
            "operation": operation,
            "message_id": self._message_id(),
            "command_id": command_id,
            "dedupe_generation": generation,
            "subject": subject,
            "preconditions": preconditions,
            "requires": requires,
            "command_digest": digest,
            "payload": payload,
        }
        if grant is not None:
            params["grant"] = grant
        try:
            result = self.conn.call(operation, params)
        except TransportError:
            self.close()
            raise
        ack = result.get("acknowledgment")
        if not isinstance(ack, dict) or ack.get("command_id") != command_id \
                or ack.get("command_digest") != digest:
            raise TransportError("%s acknowledgment does not match the command" % operation)
        return result


# ---------------------------------------------------------------------------
# Evidence producer and reader helpers (EVIDENCE sections 3-5).
# ---------------------------------------------------------------------------

def b64decode_strict(text):
    if not isinstance(text, str):
        raise ValueError("data_base64 is not a string")
    return base64.b64decode(text.encode("ascii"), validate=True)


def publish_artifact(session, grant, artifact_id, content, descriptor_fields,
                     id_prefix, chunk_bytes=None, append_bytes=None, retries=3,
                     log=None):
    """Prepare, append and seal one artifact. `content` is what the descriptor
    declares; `append_bytes` (default: content) is what is actually appended.
    Command IDs are deterministic, so after a lost connection the whole
    sequence is retransmitted and replays (CORE section 6.2, EVIDENCE
    section 4 'Interrupted uploads'). Returns the seal outcome."""
    if append_bytes is None:
        append_bytes = content
    subject = {"kind": "evidence.artifact", "id": artifact_id}
    payload = dict(descriptor_fields)
    payload["digest"] = digest_of_bytes(content)
    payload["size"] = len(content)
    attempt = 0
    while True:
        attempt += 1
        try:
            prep = session.command(
                "evidence.upload.prepare", make_identifier(id_prefix, "prepare", artifact_id),
                subject, [{"subject": subject, "revision": 0}], payload, grant=grant)
            revision = prep["acknowledgment"]["revision"]
            outcome = prep.get("outcome") or {}
            limit = outcome.get("chunk_limit")
            if not isinstance(limit, int):
                raise TransportError("prepare outcome has no chunk_limit")
            step = limit
            if isinstance(chunk_bytes, int) and chunk_bytes > 0:
                step = min(step, chunk_bytes)
            if step <= 0 and len(append_bytes) > 0:
                raise TransportError("provider allows no bytes per append")
            offset = 0
            while offset < len(append_bytes):
                chunk = append_bytes[offset: offset + step]
                app = session.command(
                    "evidence.upload.append",
                    make_identifier(id_prefix, "append", artifact_id, offset),
                    subject, [{"subject": subject, "revision": revision}],
                    {"offset": offset, "data_base64": base64.b64encode(chunk).decode("ascii")},
                    grant=grant)
                revision = app["acknowledgment"]["revision"]
                offset += len(chunk)
            seal = session.command(
                "evidence.seal", make_identifier(id_prefix, "seal", artifact_id),
                subject, [{"subject": subject, "revision": revision}], {}, grant=grant)
            return seal
        except TransportError as e:
            if log:
                log("publish %s: transport failure (%s), attempt %d" % (artifact_id, e, attempt))
            if attempt >= retries:
                raise
        except ProtocolError as e:
            if e.retry == "same_command" and attempt < retries:
                if log:
                    log("publish %s: %s, retransmitting" % (artifact_id, e.code))
                continue
            raise


def fetch_exact(session, grant, reference_artifact_id, digest, max_bytes=262144,
                max_total=64 * 1048576, advertised=None):
    """Fetch every byte of a sealed artifact in as many calls as needed and
    return the bytes exactly as received. Raises on anything that is not a
    complete, consistent, available response. Does not check the digest:
    the caller hashes the returned bytes itself. When `advertised` is a list,
    the digest each response advertised is appended to it (used only by a
    deliberately broken mutant)."""
    subject = {"kind": "evidence.artifact", "id": reference_artifact_id}
    data = bytearray()
    offset = 0
    size = None
    while True:
        result = session.query("evidence.fetch", {
            "artifact": subject, "digest": digest, "offset": offset, "max_bytes": max_bytes,
        }, grant=grant)
        if advertised is not None:
            advertised.append(result.get("digest"))
        availability = result.get("availability") or {}
        if availability.get("state") != "available":
            raise FetchError("artifact not available: %s" % availability.get("state"))
        if result.get("artifact") != subject or result.get("offset") != offset:
            raise FetchError("fetch response names another artifact or offset")
        r_size = result.get("size")
        if not isinstance(r_size, int) or r_size < 0 or r_size > max_total:
            raise FetchError("fetch response size unusable")
        if size is None:
            size = r_size
        elif size != r_size:
            raise FetchError("fetch response size changed between calls")
        chunk = b64decode_strict(result.get("data_base64"))
        next_offset = result.get("next_offset")
        if next_offset != offset + len(chunk):
            raise FetchError("next_offset inconsistent with the bytes received")
        data.extend(chunk)
        offset = next_offset
        if offset >= size:
            if offset != size:
                raise FetchError("received more bytes than the artifact size")
            return bytes(data)
        if not chunk:
            raise FetchError("fetch made no progress")


class FetchError(Exception):
    pass


def test_descriptor(media_type, source_kind, source_id, principal, now):
    """The descriptor values the harness interface fixes for artifacts a
    third-party client publishes ('Descriptor values')."""
    return {
        "media_type": media_type,
        "producer": {"principal": principal},
        "source": {"kind": source_kind, "id": source_id},
        "scope": "thirdparty",
        "capture": {"captured_at": format_instant(now), "anchors": []},
        "coverage": {"completeness": "complete", "covered": ["content"], "gaps": []},
        "retention_class": "standard",
    }
