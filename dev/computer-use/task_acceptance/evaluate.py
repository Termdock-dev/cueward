"""Independent artifact checks and honest denominators for prepared tasks."""

from collections import Counter
from decimal import Decimal
import hashlib
import json
from pathlib import Path
import stat

from .setup import CONTENT, EDITED, TASKS
from .trace import check_trace, number
from .window_dialog import check_auxiliary_receiver_trace, check_window_dialog

MAX_BYTES = 1024 * 1024
MAX_JSON_BYTES = 64 * 1024 * 1024
PREREQUISITES = {"session_locked", "permission_denied", "missing_app", "operator_unavailable", "fixture_unavailable"}
FAILURE_CATEGORIES = {"prerequisite_unavailable", "tool_observation_failure", "tool_dispatch_failure",
                      "application_rejection", "partial_observation", "stale_target", "missing_artifact",
                      "interference_observed", "agent_decision_error", "unknown"}


def read_scoped(root, relative):
    """Read a bounded regular file without following links out of the run."""
    if Path(relative).is_absolute() or ".." in Path(relative).parts:
        raise ValueError("artifact path must remain within the run")
    path = root / relative
    for part in path.relative_to(root).parts:
        root = root / part
        if root.is_symlink():
            raise ValueError(f"symlink is not a disposable regular file: {relative}")
    info = path.stat()
    if not stat.S_ISREG(info.st_mode) or info.st_size > MAX_BYTES or info.st_nlink != 1:
        raise ValueError(f"nonregular, hardlinked or oversized artifact: {relative}")
    with path.open("rb") as source:
        content = source.read(MAX_BYTES + 1)
    if len(content) > MAX_BYTES:
        raise ValueError(f"oversized artifact: {relative}")
    return content


def load_json(path):
    """Bound evidence input and reject nonstandard JSON numeric constants."""
    path = Path(path)
    if path.stat().st_size > MAX_JSON_BYTES:
        raise ValueError("JSON input exceeds 64 MiB")
    def invalid(value):
        raise ValueError(f"invalid JSON constant: {value}")
    with path.open("rb") as source:
        data = source.read(MAX_JSON_BYTES + 1)
    if len(data) > MAX_JSON_BYTES:
        raise ValueError("JSON input exceeds 64 MiB")
    return json.loads(data.decode("utf-8"), parse_constant=invalid)


def check(name, status, reason):
    return {"name": name, "status": status, "reason": reason}


def guarded_checks(name, function, *args):
    """Keep other checks and metrics when one artifact/receiver check fails."""
    try:
        return function(*args)
    except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
        return [check(name, "failed", str(error))]


def file_checks(root, manifest, task_id):
    checks = []
    expected = {"new_document": ("result.txt", CONTENT),
                "existing_document": ("existing.txt", EDITED),
                "window_dialog": ("target.txt", EDITED),
                "cross_app": ("../new_document/result.txt", CONTENT),
                "calculation": ("result.txt", f"{Decimal('19.95') * 3 + Decimal('7.50') * 2 - Decimal('5.00'):.2f}\n")}
    protected = [p for p in manifest["baseline"] if p.startswith("seeds/")
                 or p.startswith(f"work/{task_id}/") and p.endswith("bystander.txt")]
    for relative in protected:
        data = read_scoped(root, relative)
        matches = hashlib.sha256(data).hexdigest() == manifest["baseline"][relative]
        checks.append(check("preserved:" + relative, "passed" if matches else "failed", "baseline hash comparison"))
    if task_id in expected:
        name, text = expected[task_id]
        relative = "work/new_document/result.txt" if task_id == "cross_app" else f"work/{task_id}/{name}"
        matches = read_scoped(root, relative) == text.encode("utf-8")
        checks.append(check("artifact", "passed" if matches else "failed", "exact UTF-8 byte comparison"))
    if task_id in ("existing_document", "window_dialog"):
        allowed = {"bystander.txt", expected[task_id][0]}
        actual = {p.name for p in (root / "work" / task_id).iterdir()}
        checks.append(check("workspace_scope", "passed" if actual == allowed else "failed", "only the prepared files may be present"))
    return checks


def effect_checks(root, task_id, evidence):
    observer = evidence.get("observer", {})
    if not isinstance(observer, dict) or observer.get("source") != "receiver_observer":
        return [check("receiver", "unverified", "missing independent receiver observation")]
    if observer.get("run_id") != evidence["run_id"] or observer.get("task_id") != task_id:
        return [check("receiver", "failed", "stale or mismatched receiver evidence")]
    if type(observer.get("pid")) is not int or observer["pid"] <= 0 or observer["pid"] not in evidence.get("receiver_pids", []):
        return [check("receiver", "failed", "observer PID outside owned receivers")]
    if task_id == "new_document":
        if "save_requests" not in observer:
            return [check("save_once", "unverified", "missing save submission observation")]
        good = type(observer.get("save_requests")) is int and observer["save_requests"] == 1
        return [check("save_once", "passed" if good else "failed", "receiver observed one save submission")]
    if task_id == "cross_app":
        if "producer_pid" not in evidence or not all(k in observer for k in ("file", "content")):
            return [check("reader", "unverified", "missing producer identity or recipient content")]
        producer = evidence.get("producer_pid")
        if type(producer) is not int or producer <= 0 or producer == observer["pid"]:
            return [check("reader", "failed", "reader must be distinct from the producing process")]
        good = (observer.get("file") == str(root / "work/new_document/result.txt")
                and observer.get("content") == CONTENT)
        return [check("reader", "passed" if good else "failed", "recipient file identity and exact content")]
    if task_id == "window_dialog":
        status, reason = check_window_dialog(root, evidence, observer)
        checks = [check("fresh_window_observations", status, reason), *final_receiver_checks(observer)]
        if "auxiliary_receivers" in observer:
            status, reason = check_auxiliary_receiver_trace(root, evidence, observer)
            checks.append(check("auxiliary_receiver_trace", status, reason))
        if "native_io_complete" in observer:
            checks.append(check("native_document_io", "passed" if observer["native_io_complete"] is True else "failed",
                                "independently observed normal target save/write and untouched bystander"))
        return checks
    if task_id in ("existing_document", "calculation"):
        return [check("receiver", "passed", "independent receiver identity supplied; result checked from disk"),
                *final_receiver_checks(observer)]
    return pointer_checks(task_id, evidence, observer)


def final_receiver_checks(observer):
    """Reject supplemental late activation; do not upgrade older evidence with invented state."""
    fields = ("active", "activations", "observation_sequence", "fresh_after_sequence")
    if not any(k in observer for k in fields):
        return []  # Older supplied identity/artifact evidence has no final-snapshot contract.
    active, count = observer.get("active"), observer.get("activations")
    if active is True or type(count) is int and count > 0:
        return [check("final_receiver_inactive", "failed", "receiver active or lifetime activation recorded in final observation")]
    if type(active) is not bool or type(count) is not int or count < 0:
        return [check("final_receiver_inactive", "unverified", "missing or malformed final receiver activation state")]
    sequence, baseline, window = (observer.get(k) for k in ("observation_sequence", "fresh_after_sequence", "window_id"))
    if (type(sequence) is not int or type(baseline) is not int or not 0 < baseline < sequence
            or type(window) is not int or window <= 0):
        return [check("final_receiver_inactive", "unverified", "missing or invalid fresh final receiver observation")]
    return [check("final_receiver_inactive", "passed", "fresh owned receiver inactive with zero lifetime activations")]


def pointer_checks(task_id, evidence, observer):
    if not all(k in observer for k in ("initial", "final", "dispatch_count")):
        return [check("effect", "unverified", "missing initial/final states or dispatch count")]
    initial, final = observer.get("initial", {}), observer.get("final", {})
    if not isinstance(initial, dict) or not isinstance(final, dict):
        return [check("effect", "unverified", "missing initial/final receiver states")]
    if type(observer.get("dispatch_count")) is not int or observer["dispatch_count"] != 1:
        return [check("no_replay", "failed", "one dispatch required; replay cannot repair a failed effect")]
    if task_id == "webview_first_click":
        if "clicks" not in initial or "clicks" not in final:
            return [check("one_increment", "unverified", "missing counter observation")]
        good = (type(initial.get("clicks")) is int and initial["clicks"] == 0
                and type(final.get("clicks")) is int and final["clicks"] == 1)
        return [check("one_increment", "passed" if good else "failed", "fresh counter increased exactly once")]
    return canvas_checks(task_id, evidence, observer, initial, final)


def canvas_checks(task_id, evidence, observer, initial, final):
    if not all(k in state for state in (initial, final) for k in ("object", "events", "dragging", "releases")):
        return [check("canvas", "unverified", "missing object/event/release observation")]
    events = final.get("events", [])
    types = [e.get("type") for e in events if isinstance(e, dict)]
    released = (types.count("mousedown") == 1 and types.count("mouseup") == 1
                and types.index("mousedown") < types.index("mouseup")
                and final.get("dragging") is False and type(final.get("releases")) is int
                and final["releases"] == 1
                and all(isinstance(e, dict) for e in events)
                and all(e.get("trusted") is True for e in events if e.get("type") in ("mousedown", "mouseup"))
                and next((e.get("buttons") for e in events if e.get("type") == "mouseup"), None) == 0)
    checks = [check("release", "passed" if released else "failed", "one observed down/up pair and no retained drag")]
    fresh = (initial.get("object") == {"x": 100, "y": 80} and initial.get("dragging") is False
             and initial.get("releases") == 0 and initial.get("events") == [])
    checks.append(check("fresh_canvas", "passed" if fresh else "failed", "separate fresh receiver with no prior drag events"))
    if task_id == "canvas_interrupted":
        interrupted = (observer.get("controlled_interruption") is True
                       and any(r.get("status") == "dispatch_interrupted" for r in evidence.get("receipts", [])))
        checks.append(check("interruption", "passed" if interrupted else "unverified", "operator interruption must be recorded independently"))
    else:
        a, b = initial.get("object", {}), final.get("object", {})
        good = (all(number(v.get(k)) for v in (a, b) for k in ("x", "y"))
                and a["x"] == 100 and a["y"] == 80
                and b["x"] - a["x"] == 80 and b["y"] - a["y"] == 40)
        checks.append(check("displacement", "passed" if good else "failed", "fresh object moved by the requested 80 × 40 pixels"))
    return checks


def evaluate_task(root, manifest, task_id, evidence, kind):
    result = {"task_id": task_id, "family": TASKS[task_id][0], "status": "unverified", "checks": [], "metrics": {}}
    if evidence is None:
        result["reason"] = "not run"
        return result
    if not isinstance(evidence, dict) or evidence.get("run_id") != manifest["run_id"] or evidence.get("task_id") != task_id:
        result.update(status="failed", reason="invalid or stale task evidence")
        return result
    category = evidence.get("failure_category", "unknown")
    result["reported_failure_category"] = category if isinstance(category, str) and category in FAILURE_CATEGORIES else "unknown"
    result["category_source"] = "operator supplied, not inferred from a missing effect"
    if evidence.get("attempted") is not True:
        blocked = evidence.get("blocked_reason")
        prerequisite = evidence.get("prerequisite")
        is_blocked = isinstance(prerequisite, str) and prerequisite in PREREQUISITES and isinstance(blocked, str) and blocked.strip()
        result.update(status="blocked" if is_blocked else "unverified",
                      reason=blocked if isinstance(blocked, str) and blocked.strip() else "not run")
        return result
    try:
        checks = guarded_checks("artifact", file_checks, root, manifest, task_id)
        checks.extend(guarded_checks("receiver", effect_checks, root, task_id, evidence))
        trace_evidence = evidence
        if task_id == "window_dialog" and any(c["name"] == "auxiliary_receiver_trace"
                and c["status"] == "passed" for c in checks) and evidence["observer"].get("auxiliary_receivers"):
            # This temporary observation scope never changes owned receiver PIDs
            # or drops any foreground/pointer/Space/activation records.
            trace_evidence = evidence | {"receiver_pids": evidence["execution"]["observer"]["receiver_pids"]}
        status, reason, metrics = check_trace(trace_evidence, task_id == "new_document")
        checks.append(check("interference_trace", status, reason))
        result["metrics"] = metrics
        if kind != "agent":
            checks.append(check("fresh_agent", "unverified", "diagnostic or simulated runs do not count as agent acceptance"))
        result["checks"] = checks
        statuses = {c["status"] for c in checks}
        result["status"] = "failed" if "failed" in statuses else "unverified" if "unverified" in statuses else "passed"
        result["reason"] = "; ".join(c["reason"] for c in checks if c["status"] != "passed") or "artifact/effect checks and supplied sampled trace passed"
    except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
        result.update(status="failed", reason=f"artifact or evidence check failed: {error}")
    return result


def evaluate(root, evidence):
    """Keep every prepared task in the denominator, including unrun tasks."""
    root = Path(root).resolve()
    manifest = load_json(root / "manifest.json")
    if manifest.get("schema") != 1 or evidence.get("schema") != 1 or evidence.get("run_id") != manifest.get("run_id"):
        raise ValueError("unsupported schema or mismatched run ID")
    if not isinstance(evidence.get("tasks"), dict) or set(evidence["tasks"]) - set(TASKS):
        raise ValueError("unknown tasks or missing tasks mapping")
    kind = evidence.get("kind")
    if kind not in ("agent", "diagnostic", "simulation"):
        raise ValueError("kind must be agent, diagnostic or simulation")
    results = [evaluate_task(root, manifest, task, evidence["tasks"].get(task), kind) for task in TASKS]
    check_receivers(results, evidence)
    counts = Counter(r["status"] for r in results)
    matrix = {family: {s: sum(r["family"] == family and r["status"] == s for r in results)
                       for s in ("passed", "failed", "blocked", "unverified")}
              for family in sorted({v[0] for v in TASKS.values()})}
    return {"schema": 1, "run_id": manifest["run_id"], "kind": kind, "tasks": results,
            "summary": {s: counts[s] for s in ("passed", "failed", "blocked", "unverified")},
            "denominator": len(TASKS), "support_matrix": matrix,
            "limitations": "Validates supplied observer evidence, not its provenance. Sampling at <=100 ms cannot exclude shorter transients. Real-app coverage and operator-assisted tasks remain separate runs."}


def check_receivers(results, evidence):
    """Reject reuse and bind a cross-app artifact to its observed producer."""
    owners = {}
    for result in results:
        record = evidence["tasks"].get(result["task_id"], {})
        pids = record.get("receiver_pids", []) if isinstance(record, dict) else []
        if record and isinstance(record, dict) and record.get("attempted") is True and isinstance(pids, list):
            for pid in pids:
                if type(pid) is int:
                    owners.setdefault(pid, set()).add(result["task_id"])
    reused = {task for tasks in owners.values() if len(tasks) > 1 for task in tasks}
    for result in results:
        if result["task_id"] in reused:
            result["checks"].append(check("fresh_receivers", "failed", "receiver PID reused across separate tasks"))
            result.update(status="failed", reason="receiver PID reused across separate tasks")
    cross = next(r for r in results if r["task_id"] == "cross_app")
    record = evidence["tasks"].get("cross_app")
    if cross["status"] != "passed":
        return
    producer = evidence["tasks"].get("new_document", {})
    producer_result = next(r for r in results if r["task_id"] == "new_document")
    if producer_result["status"] != "passed":
        cross["checks"].append(check("producer", "unverified", "producing document task has not passed"))
        cross.update(status="unverified", reason="producing document task has not passed")
    elif record["producer_pid"] != producer["observer"]["pid"]:
        cross["checks"].append(check("producer", "failed", "producer PID does not match the saved-document receiver"))
        cross.update(status="failed", reason="producer PID does not match the saved-document receiver")
