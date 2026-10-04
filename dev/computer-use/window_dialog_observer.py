"""Independent native/AX window binding; never chooses or delivers task actions."""
import json
from pathlib import Path
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
                             str(HERE / "window-dialog-ax.swift"), "-o", str(binary)],
                            capture_output=True, timeout=90)
    (output / "binding-compiler.stdout.log").write_bytes(result.stdout)
    (output / "binding-compiler.stderr.log").write_bytes(result.stderr)
    result.check_returncode()
    return binary


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
        return observed
