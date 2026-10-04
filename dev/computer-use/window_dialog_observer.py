"""Independent native/AX window binding; never chooses or delivers task actions."""
import json
from pathlib import Path
import re
import subprocess

from native_agent_task import receiver_state
from task_acceptance.setup import write_json

HERE = Path(__file__).resolve().parent


def owned_inventory(state, pid, artifact):
    """Require two stable receiver-native file identities, not names or AX indices."""
    expected = {str(artifact), str(artifact.with_name("bystander.txt"))}
    windows = state.get("owned_windows")
    if state.get("pid") != pid or not isinstance(windows, list) or len(windows) != 2:
        raise ValueError("two owned document window observations required")
    files, ids, identifiers = set(), set(), set()
    for window in windows:
        if not isinstance(window, dict):
            raise ValueError("invalid owned window observation")
        native, identifier, file = (window.get(k) for k in ("window_id", "ax_identifier", "file"))
        if (window.get("pid") != pid or type(native) is not int or native <= 0 or native in ids
                or not isinstance(identifier, str) or not identifier or identifier in identifiers
                or file not in expected or file in files):
            raise ValueError("owned native IDs/files/identifiers changed or ambiguous")
        ids.add(native); identifiers.add(identifier); files.add(file)
    target = next(w for w in windows if w["file"] == str(artifact))
    if state.get("window_id") != target["window_id"] or state.get("file") != str(artifact):
        raise ValueError("target document state differs from owned native inventory")
    return windows


def binding_signature(value):
    """Only identities/hierarchy, not text or a guessed fixed root position."""
    return json.dumps({"windows": value["windows"], "dialog": value["dialog"]}, sort_keys=True)


def scoped_dialog(state, windows):
    """A panel cannot grant a New Folder write outside the exact document boundary."""
    dialog = state.get("dialog")
    if dialog is None:
        return None
    target = next(w for w in windows if w["file"] == state["file"])
    owner = dialog.get("owner_window", {}) if isinstance(dialog, dict) else {}
    if (not isinstance(dialog, dict) or dialog.get("kind") != "save"
            or dialog.get("can_create_directories") is not False
            or dialog.get("owner_binding") != "NSDocument.prepareSavePanel"
            or owner.get("pid") != target["pid"] or owner.get("window_id") != target["window_id"]):
        raise RuntimeError("save panel side-effect scope or owner is not independently restricted")
    return dialog


def compile_binding_observer(output):
    output = Path(output)
    binary = output / "WindowDialogAX"
    result = subprocess.run(["swiftc", "-swift-version", "6", "-parse-as-library", "-warnings-as-errors",
                             str(HERE / "window-dialog-receivers.swift"),
                             str(HERE / "window-dialog-ax.swift"), "-o", str(binary)],
                            capture_output=True, timeout=90)
    (output / "binding-compiler.stdout.log").write_bytes(result.stdout)
    (output / "binding-compiler.stderr.log").write_bytes(result.stderr)
    result.check_returncode()
    return binary


def validate_native_result(observed, request):
    """Fail closed before tokens: native panel receivers are not app-PID guesses."""
    pid = request["pid"]
    roots = []
    for window in observed["windows"]:
        root = window.get("root", {})
        if (root.get("receiver_pid") != pid or root.get("role") != "AXWindow"
                or not re.fullmatch(r"w[0-9]+", root.get("ref", "")) or root["ref"] in roots):
            raise RuntimeError("invalid or foreign native document root")
        roots.append(root["ref"])
    dialog, auxiliary = observed.get("dialog"), observed.get("auxiliary_receivers", [])
    if request["dialog"] is None:
        if dialog is not None or auxiliary:
            raise RuntimeError("unexpected panel or auxiliary receiver")
        return
    if not isinstance(dialog, dict):
        raise RuntimeError("expected native save panel observation missing")
    expected = request["dialog"]
    owner = {"pid": pid, "window_id": expected["owner_window"]["window_id"]}
    if (dialog.get("kind") != "save" or dialog.get("window_id") != expected["window_id"]
            or dialog.get("mode") != expected["mode"] or dialog.get("owner_window") != owner):
        raise RuntimeError("native save panel purpose/identity/owner changed")
    proof = dialog.get("native_binding", {})
    if dialog["mode"] == "sheet":
        receivers = proof.get("receiver_pids", [])
        root = dialog.get("root", {})
        target = next(w for w in observed["windows"] if w["file"] == request["target_file"])
        if (proof.get("source") != "native_ax_window_binding" or proof.get("window_id") != dialog["window_id"]
                or proof.get("owner_window") != owner or proof.get("owner_window_cf_equal") is not True
                or not isinstance(receivers, list) or not receivers
                or not all(type(p) is int and p > 0 for p in receivers) or len(set(receivers)) != len(receivers)
                or root.get("receiver_pid") not in receivers or root.get("role") != "AXSheet"
                or dialog.get("parent_root") != target["root"]
                or not root.get("ref", "").startswith(target["root"]["ref"] + ".")):
            raise RuntimeError("native sheet attachment or actual receiver binding is invalid")
        wanted = [{"pid": p, "window_id": dialog["window_id"], "owner_window": owner,
                   "source": "native_ax_window_binding"} for p in receivers if p != pid]
        if auxiliary != wanted:
            raise RuntimeError("auxiliary receiver scope differs from independently bound panel")
    elif auxiliary:
        raise RuntimeError("standalone service panel binding is not implemented")


class WindowBindings:
    """Keep raw requests/results, and reject a changed identity around each read."""
    def __init__(self, binary, process, artifact, state_path, output, initial):
        self.binary, self.process, self.artifact = Path(binary), process, Path(artifact)
        self.state_path, self.output = Path(state_path), Path(output)
        self.windows = owned_inventory(initial, process.pid, self.artifact)
        self.window_id = initial["window_id"]
        self.count = 0
        self.output.mkdir(exist_ok=False)

    def read(self):
        self.count += 1
        prefix = self.output / str(self.count)
        before = receiver_state(self.state_path, self.process, require_inactive=False,
                                fresh=True, expected_window_id=self.window_id)
        windows = owned_inventory(before, self.process.pid, self.artifact)
        if windows != self.windows:
            raise RuntimeError("document identities changed since owned setup")
        request = {"pid": self.process.pid, "windows": windows, "target_file": str(self.artifact),
                   "dialog": scoped_dialog(before, windows)}
        write_json(prefix.with_suffix(".request.json"), request)
        write_json(prefix.with_suffix(".receiver-before.json"), before)
        result = subprocess.run([str(self.binary)], input=json.dumps(request), capture_output=True, text=True, timeout=10)
        prefix.with_suffix(".stdout.json").write_text(result.stdout)
        prefix.with_suffix(".stderr.log").write_text(result.stderr)
        write_json(prefix.with_suffix(".status.json"), {"exit": result.returncode})
        if result.returncode:
            raise RuntimeError("native AX binding observation failed: " + result.stderr.strip())
        if len(result.stdout.encode()) > 1_048_576:
            raise RuntimeError("native binding observation exceeds budget")
        observed = json.loads(result.stdout)
        after = receiver_state(self.state_path, self.process, require_inactive=False,
                               fresh=True, expected_window_id=self.window_id)
        write_json(prefix.with_suffix(".receiver-after.json"), after)
        if (owned_inventory(after, self.process.pid, self.artifact) != self.windows
                or request["dialog"] != scoped_dialog(after, windows)):
            raise RuntimeError("document/dialog identity changed during AX observation; observe again, no action delivered")
        if (observed.get("pid") != self.process.pid or observed.get("lookup_api") != "_AXUIElementGetWindow"
                or not isinstance(observed.get("windows"), list)):
            raise RuntimeError("native AX observer identity is invalid")
        actual = [{k: w.get(k) for k in self.windows[0]} for w in observed["windows"]]
        if actual != self.windows:
            raise RuntimeError("native AX window inventory differs from retained receiver")
        validate_native_result(observed, request)
        return observed
