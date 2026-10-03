"""Bounded read-only desktop observer transport; no UI actions or input injection."""

import json
from pathlib import Path
import subprocess
import threading
import time
import uuid

HERE = Path(__file__).resolve().parent
SOURCES = [HERE / f"desktop-observer-{part}.swift" for part in ("model", "platform", "main")]


def compile_observer(directory):
    """Compile the standalone observer into an operator-owned temporary directory."""
    directory = Path(directory)
    binary = directory / "DesktopObserver"
    subprocess.run(["swiftc", "-swift-version", "6", "-warnings-as-errors",
                    "-module-cache-path", str(directory / "module-cache"),
                    *map(str, SOURCES), "-o", str(binary)], check=True, capture_output=True, timeout=90)
    return binary


def execution_from_records(records, start_id, end_id):
    """Extract a complete marked interval on the observer's own monotonic clock."""
    ready = [r for r in records if r.get("type") == "ready"]
    if len(ready) != 1 or ready[0].get("schema") != 1:
        raise ValueError("missing or invalid observer readiness")
    marks = {}
    for record in records:
        if record.get("type") == "mark":
            if record.get("id") in marks:
                raise ValueError("duplicate observer marker")
            marks[record.get("id")] = record.get("ms")
    start, end = marks.get(start_id), marks.get(end_id)
    if not all(type(n) in (int, float) for n in (start, end)) or end <= start:
        raise ValueError("missing or reversed task markers")
    samples = [r for r in records if r.get("type") == "sample"]
    before = [r for r in samples if r["ms"] <= start]
    after = [r for r in samples if r["ms"] >= end]
    if not before or not after:
        raise ValueError("observer samples do not cover both task boundaries")
    lower, upper = before[-1]["ms"], after[0]["ms"]
    samples = [r for r in samples if lower <= r["ms"] <= upper]
    events = [r for r in records if type(r.get("ms")) in (int, float) and lower <= r["ms"] <= upper
              and r.get("type") in ("activation", "observation_error", "control_error")]
    return {"start_ms": start, "end_ms": end, "samples": samples, "events": events,
            "observer": ready[0]}


def observation_checks(execution, pids, pointer_policy="stationary"):
    """Assess collection completeness, retaining physical movement as unattributed."""
    from task_acceptance.trace import check_trace

    if pointer_policy not in ("stationary", "physical_concurrent"):
        raise ValueError("unknown pointer policy")
    samples = execution.get("samples", [])
    events = execution.get("events", [])
    if set(execution.get("observer", {}).get("receiver_pids", [])) != set(pids):
        return {"status": "unverified", "reason": "observer receiver scope does not match the task"}
    if any(e.get("type") in ("observation_error", "control_error") for e in events):
        return {"status": "unverified", "reason": "observer failed within the execution interval"}
    if any(s.get("session_locked") is not False for s in samples):
        return {"status": "unverified", "reason": "session locked or session state missing during execution"}
    if any(e.get("type") == "activation" and e.get("pid") in pids for e in events):
        return {"status": "failed", "reason": "owned receiver activation observed between samples"}
    if any(s.get("frontmost_pid") in pids or any(s.get("target_active", {}).get(str(pid)) is True for pid in pids)
           for s in samples):
        return {"status": "failed", "reason": "owned receiver became foreground or active"}
    if pointer_policy == "physical_concurrent":
        # Do not turn a user's intended mouse movement into automation interference,
        # or silently claim attribution by deleting the pointer observations.
        return {"status": "unverified", "reason": "physical pointer movement needs independent receiver/input attribution",
                "pointer_policy": pointer_policy}
    evidence = {"receiver_pids": pids, "execution": execution,
                "receipts": [{"command": "observer coverage check", "elapsed_ms": 0, "status": "observation"}]}
    status, reason, metrics = check_trace(evidence)
    return {"status": status, "reason": reason, "metrics": metrics, "pointer_policy": pointer_policy}


class DesktopObserver:
    """Own a finite helper, retain JSONL evidence, and acknowledge timestamped markers."""

    def __init__(self, binary, pids, output, interval_ms=20, duration_ms=600_000):
        self.binary, self.pids, self.output = Path(binary), list(pids), Path(output)
        self.configuration = {"pids": self.pids, "interval_ms": interval_ms, "duration_ms": duration_ms}
        self.records = []
        self.condition = threading.Condition()
        self.error = None
        self.process = None
        self.log = None
        self.thread = None

    def _collect(self):
        try:
            total = 0
            for line in self.process.stdout:
                total += len(line.encode("utf-8"))
                if total > 64 * 1024 * 1024:
                    raise ValueError("observer evidence exceeds 64 MiB budget")
                self.log.write(line)
                self.log.flush()
                if len(line) > 1_048_576:
                    raise ValueError("oversized observer output line")
                record = json.loads(line)
                if not isinstance(record, dict):
                    raise ValueError("observer record is not an object")
                with self.condition:
                    self.records.append(record)
                    self.condition.notify_all()
        except (OSError, ValueError) as error:
            with self.condition:
                self.error = str(error)
                self.condition.notify_all()
        finally:
            with self.condition:
                self.condition.notify_all()

    def _wait(self, predicate, timeout=5):
        deadline = time.monotonic() + timeout
        with self.condition:
            while True:
                if self.error:
                    raise RuntimeError(self.error)
                matches = [r for r in self.records if predicate(r)]
                if matches:
                    return matches[-1]
                if self.process.poll() is not None:
                    detail = self.process.stderr.read().strip()
                    raise RuntimeError(f"desktop observer exited before acknowledgement: {detail}")
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise RuntimeError("desktop observer acknowledgement timed out")
                self.condition.wait(min(remaining, 0.1))

    def __enter__(self):
        # Exclusive creation prevents erasing previous or unrelated evidence.
        self.log = self.output.open("x", encoding="utf-8")
        try:
            self.process = subprocess.Popen([str(self.binary), json.dumps(self.configuration)],
                                            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                            stderr=subprocess.PIPE, text=True, start_new_session=True)
            self.thread = threading.Thread(target=self._collect, daemon=True)
            self.thread.start()
            self._wait(lambda r: r.get("type") == "ready")
            self._wait(lambda r: r.get("type") in ("sample", "observation_error"))
            return self
        except BaseException:
            self.close()
            raise

    def mark(self, name=None):
        """Return a marker generated by the observer, not the caller's wall clock."""
        marker = name or uuid.uuid4().hex
        if not self.process or self.process.poll() is not None:
            raise RuntimeError("observer is not running")
        self.process.stdin.write(json.dumps({"command": "mark", "id": marker}) + "\n")
        self.process.stdin.flush()
        self._wait(lambda r: r.get("type") == "mark" and r.get("id") == marker)
        # The trailing sample is needed to cover a final marker without guessing.
        stamp = next(r["ms"] for r in self.records if r.get("type") == "mark" and r.get("id") == marker)
        self._wait(lambda r: r.get("type") == "sample" and r.get("ms", -1) >= stamp
                   or r.get("type") == "observation_error" and r.get("ms", -1) >= stamp)
        return marker

    def execution(self, start, end):
        with self.condition:
            return execution_from_records(list(self.records), start, end)

    def close(self):
        if self.process:
            if self.process.poll() is None:
                try:
                    self.process.stdin.write('{"command":"stop"}\n')
                    self.process.stdin.flush()
                    self.process.wait(timeout=3)
                except (BrokenPipeError, OSError, subprocess.TimeoutExpired):
                    self.process.kill()
                    self.process.wait(timeout=3)
            if self.thread:
                self.thread.join(timeout=3)
            for stream in (self.process.stdin, self.process.stdout, self.process.stderr):
                stream.close()
        if self.log:
            self.log.close()

    def __exit__(self, *_):
        self.close()
