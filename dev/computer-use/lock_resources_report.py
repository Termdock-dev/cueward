"""Aggregate a dev-only lock probe; API success is not evidence of fresh data.

All times are Swift systemUptime seconds. Only aggregate evidence is returned:
receiver identities, captured text, paths, and raw samples remain caller-owned.
"""

from collections import Counter
import math


PHASES = ("before_lock", "locked", "after_unlock")
ROUTES = ("retained_ax", "new_ax", "retained_stream", "new_screenshot")
AX_ROUTES = frozenset(ROUTES[:2])
CAPTURE_STATUSES = frozenset(("decoded", "blank", "undecodable", "stopped",
                             "error", "timeout", "no_frame", "permission_denied"))


def _integer(value, *, positive=False):
    return type(value) is int and (value > 0 if positive else value >= 0)


def _uptime(value):
    if type(value) not in (int, float) or value < 0:
        return False
    try:
        return math.isfinite(value)
    except OverflowError:
        return False


def _receiver(value):
    return (isinstance(value, dict)
            and all(_integer(value.get(k), positive=True)
                    for k in ("sequence", "pid", "window_id"))
            and _uptime(value.get("uptime")))


def _context(sample, phase):
    before, after = sample.get("receiver_before"), sample.get("receiver_after")
    begin, end = sample.get("begin_uptime"), sample.get("end_uptime")
    locked = phase == "locked"
    valid = (type(sample.get("locked")) is bool
             and type(sample.get("locked_end")) is bool
             and sample["locked"] == sample["locked_end"] == locked
             and _uptime(begin) and _uptime(end) and begin <= end
             and _receiver(before) and _receiver(after))
    if valid:
        valid = (before["pid"] == after["pid"]
                 and before["window_id"] == after["window_id"]
                 and before["sequence"] <= after["sequence"]
                 and begin <= before["uptime"] <= after["uptime"] <= end
                 and (before["sequence"] == after["sequence"]
                      or before["uptime"] < after["uptime"]))
    return {"sample": sample, "valid": valid, "before": before, "after": after}


def _timeline(contexts):
    if not contexts or not all(c["valid"] for c in contexts):
        return False
    for left, right in zip(contexts, contexts[1:]):
        a, b = left["after"], right["before"]
        if (a["pid"] != b["pid"] or a["window_id"] != b["window_id"]
                or a["uptime"] > b["uptime"] or a["sequence"] > b["sequence"]
                or (a["sequence"] < b["sequence"] and a["uptime"] >= b["uptime"])):
            return False
    return True


def _advances(contexts):
    return (_timeline(contexts)
            and contexts[0]["before"]["sequence"] < contexts[-1]["after"]["sequence"]
            and contexts[0]["before"]["uptime"] < contexts[-1]["after"]["uptime"])


def _observation_shape(value, route):
    if not isinstance(value, dict) or not _uptime(value.get("uptime")):
        return False
    sequence, status = value.get("sequence"), value.get("status")
    if "sequence" not in value or not isinstance(status, str):
        return False
    if sequence is not None and not _integer(sequence, positive=True):
        return False
    if route in AX_ROUTES:
        valid = (status in ("decoded", "error", "permission_denied")
                 and type(value.get("api_code")) is int)
    else:
        valid = status in CAPTURE_STATUSES and _integer(value.get("frameCount"))
        if status == "decoded":
            valid = valid and value["frameCount"] > 0
    return valid and (status != "decoded" or _integer(sequence, positive=True))


def _outcome(value, route, context, oracle_advances):
    if value is None:
        return "unverified"
    if not _observation_shape(value, route):
        return "error"
    if (value["status"] == "permission_denied"
            or value.get("error_kind") == "permission_denied"
            or (route in AX_ROUTES and value["api_code"] == -25211)):
        return "permission_denied"
    if value["status"] != "decoded":
        return value["status"]
    if route in AX_ROUTES and value["api_code"] != 0:
        return "error"
    if not context["valid"]:
        return "unverified"
    before, after = context["before"], context["after"]
    sequence, uptime = value["sequence"], value["uptime"]
    if uptime > after["uptime"]:
        return "unverified"
    if sequence < before["sequence"]:
        return "stale" if oracle_advances else "unverified"
    matching = (before["sequence"] <= sequence <= after["sequence"]
                and before["uptime"] <= uptime <= after["uptime"])
    return "matching" if matching else "unverified"


def _route_status(outcomes, matches, phase_valid):
    kinds = set(outcomes)
    if not kinds:
        return "unverified"
    mapped = {"matching": "unverified", "no_frame": "unavailable",
              "stopped": "unavailable", "undecodable": "unavailable"}
    if kinds == {"matching"}:
        increasing = any(a[0] < b[0] and a[1] < b[1]
                         for a, b in zip(matches, matches[1:]))
        return "fresh" if phase_valid and increasing else "unverified"
    statuses = {mapped.get(kind, kind) for kind in kinds}
    return statuses.pop() if len(statuses) == 1 else "mixed"


def _route_report(contexts, route, phase_valid):
    outcomes, matches = Counter(), []
    sample_count = observed_count = 0
    oracle_advances = _advances(contexts)
    for context in contexts:
        observations = context["sample"].get("observations")
        value = observations.get(route) if isinstance(observations, dict) else None
        sample_count += value is not None
        observed_count += context["valid"] and _observation_shape(value, route)
        outcome = _outcome(value, route, context, oracle_advances)
        outcomes[outcome] += 1
        if outcome == "matching":
            matches.append((value["sequence"], value["uptime"]))
    return {"status": _route_status(outcomes, matches, phase_valid),
            "sample_count": sample_count, "observed_count": observed_count,
            "matching_count": len(matches),
            "distinct_sequence_count": len({sequence for sequence, _ in matches}),
            "outcome_counts": dict(sorted(outcomes.items()))}


def _phase_report(samples, phase):
    contexts = [_context(s, phase) for s in samples]
    phase_valid = len(contexts) >= 2 and _timeline(contexts)
    receiver_fresh = phase_valid and _advances(contexts)
    routes = {r: _route_report(contexts, r, phase_valid) for r in ROUTES}
    complete = receiver_fresh and all(r["observed_count"] >= 2 for r in routes.values())
    fresh = complete and all(r["status"] == "fresh" for r in routes.values())
    status = "fresh" if fresh else ("mixed" if complete else "unverified")
    return {"status": status, "sample_count": len(samples),
            "receiver_status": "fresh" if receiver_fresh else "unverified",
            "recording_status": "complete" if complete else "unverified",
            "routes": routes}


def _count_errors(errors):
    if errors is None:
        return 0
    return len(errors) if isinstance(errors, (list, tuple)) else int(bool(errors))


def _cleanup_report(cleanup):
    if not isinstance(cleanup, dict):
        return "unverified", _count_errors(cleanup)
    count = _count_errors(cleanup.get("errors"))
    status = cleanup.get("status")
    if count or status == "failed":
        return "failed", max(1, count)
    return ("passed" if status == "passed" else "unverified"), 0


def _trial_status(phases, required, clean):
    if any(phases[p]["recording_status"] != "complete" for p in required):
        return "unverified"
    return "passed" if clean and all(phases[p]["status"] == "fresh"
                                     for p in required) else "failed"


def _lock_timeline(samples):
    indices = [PHASES.index(s["phase"]) for s in samples]
    if set(indices) != set(range(len(PHASES))) or indices != sorted(indices):
        return False
    return _timeline([_context(s, s["phase"]) for s in samples])


def summarize(samples, *, mode="baseline", errors=None, cleanup=None):
    """Return an aggregate research matrix without claiming product support.

    A route is fresh only after two different, increasing receiver-correlated
    sequences in one consistently locked/unlocked phase. A complete recording
    can include API failures; it is distinct from fresh observation evidence.
    """
    if mode not in ("baseline", "lock"):
        raise ValueError("mode must be baseline or lock")
    groups = {p: [] for p in PHASES}
    valid_samples = []
    invalid_count = 0
    for sample in samples:
        if (isinstance(sample, dict) and isinstance(sample.get("phase"), str)
                and sample["phase"] in groups):
            groups[sample["phase"]].append(sample)
            valid_samples.append(sample)
        else:
            invalid_count += 1
    phases = {p: _phase_report(groups[p], p) for p in PHASES}
    cleanup_status, cleanup_count = _cleanup_report(cleanup)
    error_count = _count_errors(errors) + invalid_count
    clean = not error_count and cleanup_status == "passed" and not cleanup_count
    required = PHASES if mode == "lock" else PHASES[:1]
    recorded = all(phases[p]["recording_status"] == "complete" for p in required)
    lock_order = mode == "lock" and not invalid_count and _lock_timeline(valid_samples)
    if mode == "lock":
        recorded = recorded and lock_order
    return {"schema_version": 1, "mode": mode, "phases": phases,
            "baseline_status": _trial_status(phases, PHASES[:1], clean),
            "lock_status": (_trial_status(phases, PHASES, clean)
                            if lock_order else "unverified"),
            "phase_order_status": "verified" if lock_order else "unverified",
            "recording_status": "complete" if recorded else "unverified",
            "error_count": error_count, "cleanup_status": cleanup_status,
            "cleanup_error_count": cleanup_count,
            "product_operation_support": "unverified"}
