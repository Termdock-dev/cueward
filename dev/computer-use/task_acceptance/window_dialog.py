"""Check concrete owned windows and observed save-dialog hierarchy transitions."""

from pathlib import Path
import re

from .trace import number


class MissingEvidence(ValueError):
    """A required observation was not collected, rather than contradicting a goal."""


def fields(value, names):
    if not isinstance(value, dict):
        raise ValueError("window/dialog identity must be an object")
    missing = [name for name in names if name not in value]
    if missing:
        raise MissingEvidence("missing window/dialog identity fields: " + ", ".join(missing))


def positive(value):
    return type(value) is int and value > 0


def root_identity(value, pids):
    fields(value, ("receiver_pid", "ref", "role"))
    reference = value["ref"]
    if not (positive(value["receiver_pid"]) and value["receiver_pid"] in pids
            and isinstance(reference, str) and re.fullmatch(r"w[0-9]+(?:\.[0-9]+){0,12}", reference)
            and isinstance(value["role"], str)):
        raise ValueError("invalid or unowned AX root")
    return value["receiver_pid"], reference, value["role"]


def document_windows(windows, pids, paths, with_roots=False):
    if not isinstance(windows, list) or len(windows) != 2:
        raise ValueError("two document windows required")
    result, ids, roots = {}, set(), set()
    for window in windows:
        fields(window, ("pid", "window_id", "file"))
        pid, native_id, file = window["pid"], window["window_id"], window["file"]
        if not (positive(pid) and pid in pids and positive(native_id)
                and isinstance(file, str) and file in paths and file not in result and native_id not in ids):
            raise ValueError("document windows must have distinct owned IDs and prepared file identities")
        if with_roots:
            fields(window, ("root",))
            identity = root_identity(window["root"], pids)
            if identity[0] != pid or identity[2] != "AXWindow" or "." in identity[1] or identity[:2] in roots:
                raise ValueError("ambiguous document root or wrong document receiver")
            roots.add(identity[:2])
        result[file] = window
        ids.add(native_id)
    return result


def save_dialog(dialog, target, windows, pids):
    if dialog is None:
        raise ValueError("save dialog was not observed in the dialog phase")
    fields(dialog, ("kind", "mode", "owner_window", "root"))
    fields(dialog["owner_window"], ("pid", "window_id"))
    owner = dialog["owner_window"]
    if (dialog["kind"] != "save" or not positive(owner["pid"]) or not positive(owner["window_id"])
            or (owner["pid"], owner["window_id"]) != (target["pid"], target["window_id"])):
        raise ValueError("save dialog does not belong to the intended document window")
    identity = root_identity(dialog["root"], pids)
    document_roots = {root_identity(w["root"], pids)[:2] for w in windows.values()}
    if identity[:2] in document_roots:
        raise ValueError("dialog cannot reuse a document AX root")
    if dialog["mode"] == "sheet":
        fields(dialog, ("parent_root",))
        parent = root_identity(dialog["parent_root"], pids)
        if (parent != root_identity(target["root"], pids) or identity[0] != parent[0]
                or not identity[1].startswith(parent[1] + ".") or identity[2] != "AXSheet"):
            raise ValueError("sheet is not a descendant of the current target document root")
    elif dialog["mode"] == "window":
        fields(dialog, ("window_id",))
        if (not positive(dialog["window_id"]) or dialog["window_id"] in {w["window_id"] for w in windows.values()}
                or "." in identity[1] or identity[2] not in ("AXWindow", "AXDialog")):
            raise ValueError("standalone save panel requires its own native window and AX root")
    else:
        raise ValueError("unknown save dialog mode")
    return identity


def phase_identity(observation, owned, pids, target_path):
    fields(observation, ("windows", "active_root", "dialog"))
    windows = document_windows(observation["windows"], pids, set(owned), with_roots=True)
    for file, window in windows.items():
        if (window["pid"], window["window_id"]) != (owned[file]["pid"], owned[file]["window_id"]):
            raise ValueError("document window identity changed across phases")
    target = windows[target_path]
    if observation["phase"] == "dialog":
        expected = save_dialog(observation["dialog"], target, windows, pids)
    else:
        if observation["dialog"] is not None:
            raise ValueError("save dialog must be absent before opening and after resuming")
        expected = root_identity(target["root"], pids)
    if root_identity(observation["active_root"], pids) != expected:
        raise ValueError("observed action root is not the current document/dialog hierarchy")


def transitions(root, evidence, observer):
    fields(observer, ("owned_windows", "window_observations"))
    pids = evidence.get("receiver_pids", [])
    if not isinstance(pids, list) or not pids or not all(positive(pid) for pid in pids):
        raise MissingEvidence("missing owned receiver PIDs")
    folder = Path(root) / "work/window_dialog"
    target_path = str(folder / "target.txt")
    owned = document_windows(observer["owned_windows"], pids, {target_path, str(folder / "bystander.txt")})
    if owned[target_path]["pid"] != observer["pid"]:
        raise ValueError("target window must belong to the observing document receiver")
    observations = observer["window_observations"]
    if not isinstance(observations, list) or len(observations) != 3:
        raise ValueError("initial, dialog and resumed observations required")
    execution = evidence.get("execution", {})
    fields(execution, ("start_ms", "end_ms"))
    start, end = execution["start_ms"], execution["end_ms"]
    if not (number(start) and number(end) and start < end):
        raise ValueError("invalid execution interval for dialog observations")
    snapshots, last = set(), None
    for phase, observation in zip(("initial", "dialog", "resumed"), observations):
        fields(observation, ("phase", "snapshot_id", "observed_ms", "target_file"))
        snapshot, time = observation["snapshot_id"], observation["observed_ms"]
        if not (observation["phase"] == phase and observation["target_file"] == target_path
                and isinstance(snapshot, str) and snapshot.strip() and snapshot not in snapshots
                and number(time) and start <= time <= end and (last is None or last < time)):
            raise ValueError("stale, unordered or mismatched window/dialog observation")
        snapshots.add(snapshot)
        last = time
        phase_identity(observation, owned, pids, target_path)


def check_window_dialog(root, evidence, observer):
    """Return unverified for absent identities and failed for contradictions."""
    try:
        transitions(root, evidence, observer)
    except MissingEvidence as error:
        return "unverified", str(error)
    except ValueError as error:
        return "failed", str(error)
    return "passed", "two owned document windows and ordered save-dialog open/close root transitions observed"
