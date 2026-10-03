"""Interactive task transport: the agent chooses commands, the observer records them."""
import base64
from contextlib import contextmanager
import json
import os
from pathlib import Path
import select
import socket
import sys
import time

from observed_cli import ObservedCLI
from task_acceptance.setup import write_json


class ScopeError(ValueError):
    """A request rejected before any CLI dispatch."""


def option(arguments, name):
    if arguments.count(name) != 1:
        raise ScopeError(f"one {name} is required")
    position = arguments.index(name)
    if position + 1 >= len(arguments):
        raise ScopeError(f"missing {name} value")
    return arguments[position + 1]


def validate_command(arguments, pid, window_id):
    """Constrain the diagnostic transport to one operator-owned receiver."""
    if not isinstance(arguments, list) or not 2 <= len(arguments) <= 40 or not all(isinstance(a, str) for a in arguments):
        raise ScopeError("command must be a bounded JSON string array")
    if sum(len(a.encode("utf-8")) for a in arguments) > 65_536:
        raise ScopeError("command exceeds 64 KiB")
    family, action = arguments[:2]
    if (family, action) == ("app", "inspect"):
        if option(arguments, "--pid") != str(pid):
            raise ScopeError("app PID outside owned scope")
    elif family == "app" and action in ("press", "set-value"):
        target = decode_target(option(arguments, "--target"))
        app = target.get("app")
        if target.get("kind") != "app_ax" or not isinstance(app, dict) or app.get("pid") != pid:
            raise ScopeError("app target outside owned scope")
    elif family == "window" and action in ("snapshot", "inspect"):
        if option(arguments, "--id") != str(window_id) or "--output" in arguments:
            raise ScopeError("window/output outside owned scope")
    elif family == "window" and action in ("click", "drag"):
        target = decode_target(option(arguments, "--target"))
        window = target.get("window", {})
        if target.get("kind") != "window_input" or not isinstance(window, dict) or window.get("owner_pid") != pid or window.get("window_id") != window_id:
            raise ScopeError("input target outside owned scope")
        if action == "click" and option(arguments, "--count") != "1":
            raise ScopeError("task click must be single; no repair replay")
    else:
        raise ScopeError("command is not permitted in this owned task session")
    return arguments


def decode_target(token):
    if len(token) > 16_384:
        raise ScopeError("oversized target")
    try:
        value = json.loads(base64.b64decode(token + "=" * (-len(token) % 4), altchars=b"-_", validate=True))
    except (ValueError, UnicodeError) as error:
        raise ScopeError("invalid scoped target") from error
    if not isinstance(value, dict):
        raise ScopeError("target is not an object")
    return value


class AgentSession:
    """Retain actual input/output and task boundaries without selecting agent actions."""
    def __init__(self, cli, observer, pid, window_id, output, metadata):
        self.commands = ObservedCLI(cli, observer)
        self.observer, self.pid, self.window_id = observer, pid, window_id
        self.output, self.metadata = Path(output), metadata
        self.records = []
        self.dispatches = {"click": 0, "drag": 0}
        self.recording_started = False
        self.termination_reason = "not_started"
        self.request_count = 0

    def invoke(self, arguments):
        arguments = validate_command(arguments, self.pid, self.window_id)
        if arguments[0] == "window" and arguments[1] in self.dispatches:
            action = arguments[1]
            if self.dispatches[action]:
                raise ScopeError("one task dispatch already attempted; no replay")
            self.dispatches[action] += 1
        record = {"arguments": arguments}
        self.records.append(record)
        try:
            record["result"] = self.commands.call(*arguments)
            return {"result": record["result"]}
        except RuntimeError as error:
            record["error"] = str(error)
            return {"error": str(error), "do_not_replay": True}
        finally:
            if self.recording_started:
                write_json(self.output, {**self.metadata, "status": "observing", "receiver_pids": [self.pid],
                                        "receipts": self.commands.receipts, "records": self.records,
                                        "dispatches": self.dispatches})

    @contextmanager
    def recording(self):
        with self.output.open("x", encoding="utf-8") as output:
            output.write('{"status":"observing"}\n')
        self.recording_started = True
        self.termination_reason = "observing"
        start = self.observer.mark("agent-task-start")
        try:
            yield
        except BaseException:
            self.termination_reason = "transport_error"
            raise
        finally:
            report = {**self.metadata, "receiver_pids": [self.pid], "receipts": self.commands.receipts,
                      "records": self.records, "dispatches": self.dispatches,
                      "termination_reason": self.termination_reason, "request_count": self.request_count}
            try:
                end = self.observer.mark("agent-task-end")
                report["execution"] = self.observer.execution(start, end)
            except (OSError, RuntimeError, ValueError) as error:
                report["observation_error"] = str(error)
            write_json(self.output, report)

    def request(self, value):
        if value == {"finish": True}:
            return {"finished": True}
        try:
            return self.invoke(value)
        except ScopeError as error:
            return {"error": str(error), "not_dispatched": True}

    def serve(self, source=sys.stdin, sink=sys.stdout, duration=800):
        with self.recording():
            deadline = time.monotonic() + duration
            print(json.dumps({"ready": True, "owned_pid": self.pid, "owned_window_id": self.window_id}), file=sink, flush=True)
            pending = bytearray()
            while self.request_count < 64 and time.monotonic() < deadline:
                if not select.select([source], [], [], min(1, max(0, deadline - time.monotonic())))[0]:
                    continue
                chunk = os.read(source.fileno(), 4096)
                if not chunk:
                    self.termination_reason = "stdin_partial" if pending else "stdin_closed"
                    return
                pending.extend(chunk)
                if len(pending) > 65_536:
                    raise ValueError("oversized task command")
                while b"\n" in pending and self.request_count < 64:
                    line, pending = pending.split(b"\n", 1)
                    self.request_count += 1
                    response = self.request(json.loads(line))
                    print(json.dumps(response, ensure_ascii=False), file=sink, flush=True)
                    if response.get("finished"):
                        self.termination_reason = "finished"
                        return
            self.termination_reason = "request_budget" if self.request_count == 64 else "deadline"

    def serve_socket(self, path, duration=800):
        """Use a same-machine socket; terminal session IDs are agent-local."""
        path = Path(path)
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as server:
            server.bind(str(path))  # Never replace an existing socket/file.
            try:
                path.chmod(0o600)
                server.listen(1)
                server.settimeout(1)
                with self.recording():
                    deadline = time.monotonic() + duration
                    print(json.dumps({"ready": True, "owned_pid": self.pid, "owned_window_id": self.window_id}), flush=True)
                    count = 0
                    while time.monotonic() < deadline and count < 64:
                        try:
                            connection, _ = server.accept()
                        except socket.timeout:
                            continue
                        count += 1
                        self.request_count = count
                        with connection:
                            connection.settimeout(5)
                            try:
                                value = json.loads(read_request(connection))
                            except (OSError, ValueError, UnicodeError) as error:
                                response = {"error": str(error), "not_dispatched": True}
                            else:
                                response = self.request(value)
                            connection.sendall(json.dumps(response, ensure_ascii=False).encode("utf-8"))
                            if response.get("finished"):
                                self.termination_reason = "finished"
                                return
                    self.termination_reason = "request_budget" if count == 64 else "deadline"
            finally:
                path.unlink(missing_ok=True)


def read_request(connection):
    """Bound an EOF-delimited local request without waiting forever for a newline."""
    content = bytearray()
    while True:
        chunk = connection.recv(min(4096, 65_537 - len(content)))
        if not chunk:
            return content.decode("utf-8")
        content.extend(chunk)
        if len(content) > 65_536:
            raise ValueError("task command exceeds 64 KiB")
