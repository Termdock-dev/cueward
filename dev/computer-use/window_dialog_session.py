"""Fresh-agent native task policy; actions always come from the Agent's broker request."""
import uuid

from agent_session import AgentSession, ScopeError, decode_target, option, validate_command
from task_acceptance.setup import write_json
from window_dialog_observer import binding_signature


def command_options(arguments):
    """Reject alternate/unknown flag spellings without interpreting text values as flags."""
    allowed = {("app", "inspect"): {"--pid", "--root", "--depth", "--limit"},
               ("app", "press"): {"--target"}, ("app", "set-value"): {"--target", "--value"},
               ("window", "snapshot"): {"--id"}, ("window", "inspect"): {"--id", "--depth", "--limit"}}
    if (not isinstance(arguments, list) or not 2 <= len(arguments) <= 40
            or not all(isinstance(a, str) for a in arguments)
            or sum(len(a.encode()) for a in arguments) > 65_536):
        raise ScopeError("invalid bounded dialog task command")
    flags = allowed.get(tuple(arguments[:2]))
    if flags is None or len(arguments[2:]) % 2:
        raise ScopeError("dialog task permits only scoped native AX/window flag-value pairs")
    values = {}
    for flag, value in zip(arguments[2::2], arguments[3::2]):
        if flag not in flags or flag in values:
            raise ScopeError("unknown, attached or duplicate dialog task option")
        values[flag] = value
    return values


def under(reference, root):
    return isinstance(reference, str) and (reference == root or reference.startswith(root + "."))


def effective_root(binding, artifact):
    if binding["dialog"] is not None:
        return binding["dialog"]["root"]
    return next(w["root"] for w in binding["windows"] if w["file"] == str(artifact))


class DialogSession(AgentSession):
    """Scope to independently bound documents/panel and actually returned AX targets."""
    def __init__(self, *args, bindings, artifact, **kwargs):
        super().__init__(*args, **kwargs)
        self.bindings, self.artifact = bindings, artifact
        self.tokens = {}
        self.signature = None
        self.phases = []
        self.auxiliary_receivers = []
        self.phase_path = self.output.with_name("window-observations.json")

    def bound_request(self, arguments):
        # Restrict syntax before native reads, so invalid requests are never dispatched.
        command_options(arguments)
        binding = self.bindings.read()
        for receiver in binding.get("auxiliary_receivers", []):
            if receiver not in self.auxiliary_receivers:
                self.auxiliary_receivers.append(receiver)
        self.window_ids = [w["window_id"] for w in binding["windows"]]
        if binding["dialog"] is not None:
            self.window_ids.append(binding["dialog"]["window_id"])
        validate_command(arguments, self.pid, self.window_ids)
        signature = binding_signature(binding)
        if signature != self.signature:
            self.tokens.clear()
            self.signature = signature
        roots = [w["root"]["ref"] for w in binding["windows"]]
        if binding["dialog"] is not None:
            roots.append(binding["dialog"]["root"]["ref"])
        if arguments[:2] == ["app", "inspect"]:
            if "--root" in arguments and not any(under(option(arguments, "--root"), root) for root in roots):
                raise ScopeError("root is outside independently bound native windows")
        elif arguments[0] == "app":
            token = option(arguments, "--target")
            if token not in self.tokens or not any(under(self.tokens[token]["ref"], root) for root in roots):
                raise ScopeError("target was not returned by a current owned broker observation")
        return binding

    def timestamp(self):
        marker = self.observer.mark("window-binding-" + uuid.uuid4().hex)
        with self.observer.condition:
            records = [r for r in self.observer.records if r.get("type") == "mark" and r.get("id") == marker]
        if len(records) != 1 or type(records[0].get("ms")) not in (int, float):
            raise RuntimeError("window phase has no independent execution timestamp")
        return records[0]["ms"]

    def capture(self, arguments, binding, stamp):
        if arguments[0] != "app":
            return
        root = effective_root(binding, self.artifact)
        if arguments[1] == "inspect":
            if "--root" not in arguments:
                return
            ref = option(arguments, "--root")
        else:
            ref = self.tokens.get(option(arguments, "--target"), {}).get("ref")
        if not under(ref, root["ref"]):
            return
        phase = "dialog" if binding["dialog"] is not None else "resumed" if any(
            p["phase"] == "dialog" for p in self.phases) else "initial"
        if any(p["phase"] == phase for p in self.phases):
            return
        if phase == "dialog" and not self.phases:
            raise RuntimeError("save dialog used without an initial document observation")
        self.phases.append({"phase": phase, "snapshot_id": uuid.uuid4().hex, "observed_ms": stamp,
                            "target_file": str(self.artifact), "windows": binding["windows"],
                            "active_root": root, "dialog": binding["dialog"]})
        write_json(self.phase_path, {"window_observations": self.phases})

    def register(self, arguments, result, before, after):
        if arguments[:2] != ["app", "inspect"] or binding_signature(before) != binding_signature(after):
            return
        if not isinstance(result, dict) or result.get("app", {}).get("pid") != self.pid:
            raise RuntimeError("CLI inspection identity differs from owned receiver")
        for window in before["windows"]:
            nodes = [n for n in result.get("nodes", []) if n.get("ref") == window["root"]["ref"]]
            for node in nodes:
                if (node.get("identifier") != window["ax_identifier"] or node.get("receiver_pid") != self.pid
                        or node.get("role") != "AXWindow"):
                    raise RuntimeError("CLI root/native identity observation disagrees")
        roots = [w["root"]["ref"] for w in before["windows"]]
        if before["dialog"] is not None:
            roots.append(before["dialog"]["root"]["ref"])
        for node in result.get("nodes", []):
            token = node.get("target")
            if not token or not any(under(node.get("ref"), r) for r in roots):
                continue
            decoded = decode_target(token)
            allowed = {self.pid}
            if before["dialog"] is not None and under(node.get("ref"), before["dialog"]["root"]["ref"]):
                allowed.update(r["pid"] for r in before.get("auxiliary_receivers", []))
            if (node.get("receiver_pid") not in allowed or decoded.get("ref") != node.get("ref")
                    or decoded.get("app", {}).get("pid") != self.pid or decoded.get("kind") != "app_ax"):
                raise RuntimeError("observed AX token does not bind an owned receiver/root")
            self.tokens[token] = node

    def invoke(self, arguments):
        binding = self.bound_request(arguments)
        stamp = self.timestamp()
        response = super().invoke(arguments)
        record = self.records[-1]
        if "result" in record:
            # Capture actual used pre-dispatch hierarchy before legitimate open/close changes.
            # Post-dispatch observation errors are never reported as not_dispatched.
            try:
                after = self.bindings.read()
                if arguments[:2] == ["app", "inspect"]:
                    if binding_signature(binding) == binding_signature(after):
                        self.register(arguments, record["result"], binding, after)
                        self.capture(arguments, binding, stamp)
                else:
                    self.capture(arguments, binding, stamp)
                if binding_signature(binding) != binding_signature(after):
                    self.tokens.clear()
                    self.signature = binding_signature(after)
            except ScopeError as error:
                raise RuntimeError("CLI returned but scoped evidence was invalid; do not replay") from error
        return response
