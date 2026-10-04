"""Check concrete owned windows and observed save-dialog hierarchy transitions."""

from pathlib import Path
import re

from .trace import MAX_INTERVAL_MS, number


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


def auxiliary_receivers(observer, pids, windows):
    """Shared services are authorized only for a bound panel, never newly owned."""
    if "auxiliary_receivers" not in observer:
        return []
    records = observer["auxiliary_receivers"]
    if not isinstance(records, list):
        raise ValueError("auxiliary receiver records must be a list")
    seen_pids = set()
    for record in records:
        fields(record, ("pid", "window_id", "owner_window", "source"))
        fields(record["owner_window"], ("pid", "window_id"))
        pid, native, owner = record["pid"], record["window_id"], record["owner_window"]
        if (not positive(pid) or pid in pids or pid in seen_pids or not positive(native)
                or native in {w["window_id"] for w in windows.values()}
                or not positive(owner["pid"]) or owner["pid"] not in pids
                or not positive(owner["window_id"]) or record["source"] != "native_ax_window_binding"):
            raise ValueError("foreign, duplicate or invalid auxiliary panel receiver")
        seen_pids.add(pid)
    return records


def auxiliary_dialog(dialog, identity, records, pids):
    fields(dialog, ("window_id", "native_binding"))
    if (not positive(dialog["window_id"]) or any(record["window_id"] != dialog["window_id"]
            or record["owner_window"] != dialog["owner_window"] for record in records)):
        raise ValueError("auxiliary service does not match the observed panel and owner")
    proof = dialog["native_binding"]
    fields(proof, ("source", "window_id", "owner_window", "owner_window_cf_equal", "receiver_pids"))
    fields(proof["owner_window"], ("pid", "window_id"))
    if (proof["source"] != "native_ax_window_binding" or not positive(proof["window_id"])
            or proof["window_id"] != dialog["window_id"] or proof["owner_window"] != dialog["owner_window"]):
        raise ValueError("native AX panel proof contradicts the observed native owner or window")
    receivers = proof["receiver_pids"]
    if (not isinstance(receivers, list) or not receivers or not all(positive(pid) for pid in receivers)
            or len(set(receivers)) != len(receivers) or identity[0] not in receivers
            or set(receivers) - set(pids) != {record["pid"] for record in records}):
        raise ValueError("native panel receiver identities are unknown, duplicate or unbound")
    if dialog["mode"] == "window":
        raise MissingEvidence("standalone auxiliary save-panel owner binding remains unverified")
    if proof["owner_window_cf_equal"] is False:
        raise ValueError("native AX sheet window does not equal its owned document owner")
    if proof["owner_window_cf_equal"] is not True:
        raise MissingEvidence("native AX sheet owner equality was not observed")


def save_dialog(dialog, target, windows, pids, auxiliary):
    if dialog is None:
        raise ValueError("save dialog was not observed in the dialog phase")
    fields(dialog, ("kind", "mode", "owner_window", "root"))
    fields(dialog["owner_window"], ("pid", "window_id"))
    owner = dialog["owner_window"]
    if (dialog["kind"] != "save" or not positive(owner["pid"]) or not positive(owner["window_id"])
            or (owner["pid"], owner["window_id"]) != (target["pid"], target["window_id"])):
        raise ValueError("save dialog does not belong to the intended document window")
    identity = root_identity(dialog["root"], pids + [record["pid"] for record in auxiliary])
    proof = dialog.get("native_binding")
    supplied_receivers = proof.get("receiver_pids") if isinstance(proof, dict) else None
    if isinstance(proof, dict) and "receiver_pids" in proof and (
            not isinstance(supplied_receivers, list) or not supplied_receivers
            or not all(positive(pid) for pid in supplied_receivers)
            or len(set(supplied_receivers)) != len(supplied_receivers) or identity[0] not in supplied_receivers):
        raise ValueError("native panel receiver identities are invalid or duplicate")
    foreign_proof = isinstance(supplied_receivers, list) and any(pid not in pids for pid in supplied_receivers)
    if auxiliary or identity[0] not in pids or foreign_proof:
        auxiliary_dialog(dialog, identity, auxiliary, pids)
    document_roots = {root_identity(w["root"], pids)[:2] for w in windows.values()}
    if identity[:2] in document_roots:
        raise ValueError("dialog cannot reuse a document AX root")
    if dialog["mode"] == "sheet":
        fields(dialog, ("parent_root",))
        parent = root_identity(dialog["parent_root"], pids)
        if (parent != root_identity(target["root"], pids)
                or identity[0] in pids and identity[0] != parent[0]
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


def phase_identity(observation, owned, pids, target_path, auxiliary):
    fields(observation, ("windows", "active_root", "dialog"))
    windows = document_windows(observation["windows"], pids, set(owned), with_roots=True)
    for file, window in windows.items():
        if (window["pid"], window["window_id"]) != (owned[file]["pid"], owned[file]["window_id"]):
            raise ValueError("document window identity changed across phases")
    target = windows[target_path]
    if observation["phase"] == "dialog":
        expected = save_dialog(observation["dialog"], target, windows, pids, auxiliary)
    else:
        if observation["dialog"] is not None:
            raise ValueError("save dialog must be absent before opening and after resuming")
        expected = root_identity(target["root"], pids)
    if root_identity(observation["active_root"], [expected[0]]) != expected:
        raise ValueError("observed action root is not the current document/dialog hierarchy")
    return expected


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
    auxiliary = auxiliary_receivers(observer, pids, owned)
    if "auxiliary_receivers" in observer and pids != [observer["pid"]]:
        raise ValueError("shared auxiliary services cannot become owned task receivers")
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
        phase_identity(observation, owned, pids, target_path, auxiliary)
    return auxiliary


def check_window_dialog(root, evidence, observer):
    """Return unverified for absent identities and failed for contradictions."""
    try:
        transitions(root, evidence, observer)
    except MissingEvidence as error:
        return "unverified", str(error)
    except ValueError as error:
        return "failed", str(error)
    return "passed", "two owned document windows and ordered save-dialog open/close root transitions observed"


def check_auxiliary_receiver_trace(root, evidence, observer):
    """Require service coverage from task start, without inventing earlier state."""
    try:
        records = transitions(root, evidence, observer)
    except MissingEvidence as error:
        return "unverified", str(error)
    except ValueError as error:
        return "failed", str(error)
    if not records:
        return "passed", "no shared auxiliary panel receivers supplied"
    services = {record["pid"] for record in records}
    execution = evidence.get("execution", {})
    samples, events = execution.get("samples"), execution.get("events")
    if isinstance(samples, list) and any(isinstance(sample, dict)
            and isinstance(sample.get("target_active"), dict)
            and any(sample["target_active"].get(str(pid)) is True for pid in services) for sample in samples):
        return "failed", "shared save-panel service became active in execution samples"
    if isinstance(events, list) and any(isinstance(event, dict) and event.get("type") == "activation"
            and type(event.get("pid")) is int and event["pid"] in services for event in events):
        return "failed", "shared save-panel service activation observed between samples"
    native = execution.get("observer")
    receivers = native.get("receiver_pids") if isinstance(native, dict) else None
    expected = set(evidence["receiver_pids"]) | services
    if (not isinstance(native, dict) or native.get("schema") != 1 or not isinstance(receivers, list)
            or not all(positive(pid) for pid in receivers) or len(set(receivers)) != len(receivers)
            or set(receivers) != expected):
        return "unverified", "native observer did not cover the owned host and bound auxiliary services"
    if (not isinstance(samples, list) or len(samples) < 3
            or not all(isinstance(sample, dict) and number(sample.get("ms"))
                       and isinstance(sample.get("target_active"), dict)
                       and all(type(sample["target_active"].get(str(pid))) is bool for pid in services)
                       for sample in samples)):
        return "unverified", "missing Boolean auxiliary activation samples across the full execution interval"
    if (not isinstance(events, list) or not all(isinstance(event, dict) and number(event.get("ms"))
            and (event.get("type") != "activation" or positive(event.get("pid"))) for event in events)
            or any(event.get("type") in ("observation_error", "control_error") for event in events)
            or any(sample.get("session_locked") is not False for sample in samples)):
        return "unverified", "auxiliary observer event or unlocked-session coverage is incomplete"
    times = [sample["ms"] for sample in samples]
    if (any(b <= a for a, b in zip(times, times[1:])) or times[0] > execution["start_ms"]
            or times[-1] < execution["end_ms"] or max(b-a for a, b in zip(times, times[1:])) > MAX_INTERVAL_MS):
        return "unverified", "auxiliary samples do not cover the execution interval within 100 ms"
    return "passed", "bound shared service inactive throughout supplied sampled execution coverage"
