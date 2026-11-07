"""Validate supplied execution samples; do not infer coverage from endpoints."""

import math

MAX_INTERVAL_MS = 100
# Input arrives on a different callback from sampling, so a change can lead or trail its count.
POINTER_ATTRIBUTION_MS = 100
# Foreground and Space changes finish after the click or key press that requested them.
DESKTOP_ATTRIBUTION_MS = 1000
INPUT_COUNTS = ("physical_pointer", "automated_pointer", "physical_other")


def number(value):
    return type(value) in (int, float) and math.isfinite(value)


def valid_sample(sample, pids):
    return (isinstance(sample, dict) and number(sample.get("ms"))
            and type(sample.get("frontmost_pid")) is int and sample["frontmost_pid"] > 0
            and isinstance(sample.get("pointer"), list) and len(sample["pointer"]) == 2
            and all(number(v) for v in sample["pointer"])
            and isinstance(sample.get("visible_spaces"), dict) and sample["visible_spaces"]
            and all(isinstance(k, str) and isinstance(v, str) and v
                    for k, v in sample["visible_spaces"].items())
            and isinstance(sample.get("target_active"), dict)
            and all(type(sample["target_active"].get(str(pid))) is bool for pid in pids))


def has_input_counts(sample):
    counts = sample.get("input")
    return isinstance(counts, dict) and all(type(counts.get(key)) is int and counts[key] >= 0 for key in INPUT_COUNTS)


def input_advanced(samples, index, keys, before_ms):
    """Report whether the named counts grew around one sample."""
    moment = samples[index]["ms"]
    first = index
    while first > 0 and moment - samples[first]["ms"] < before_ms:
        first -= 1
    last = index
    while last + 1 < len(samples) and samples[last + 1]["ms"] - moment <= POINTER_ATTRIBUTION_MS:
        last += 1
    return any(samples[last]["input"][key] > samples[first]["input"][key] for key in keys)


def command_spans(receipts, marks):
    """Return each command's marked interval, or None when any boundary is unknown."""
    if not isinstance(marks, dict):
        return None
    spans = [(marks.get(r.get("start_marker")), marks.get(r.get("end_marker"))) for r in receipts]
    return spans if all(number(start) and number(end) for start, end in spans) else None


def cueward_could_act(spans, previous, sample):
    """Helpers exit with their command, so a change outside every span is not Cueward's."""
    return spans is None or any(sample["ms"] >= start and previous["ms"] <= end + DESKTOP_ATTRIBUTION_MS
                                for start, end in spans)


def command_coverage_gap(samples, spans):
    """Return the largest sample gap in which a Cueward command could have acted."""
    return max((b["ms"] - a["ms"] for a, b in zip(samples, samples[1:]) if cueward_could_act(spans, a, b)), default=0)


def unexplained_changes(samples):
    """Yield each change that physical input cannot explain, with the samples around it."""
    for index in range(1, len(samples)):
        previous, sample = samples[index - 1], samples[index]
        if sample["input"]["automated_pointer"] > previous["input"]["automated_pointer"]:
            yield previous, sample, "automated input reached the global pointer stream"
        elif (sample["pointer"] != previous["pointer"]
                and not input_advanced(samples, index, ("physical_pointer",), POINTER_ATTRIBUTION_MS)):
            yield previous, sample, "pointer moved without physical input"
        elif (any(sample[key] != previous[key] for key in ("frontmost_pid", "visible_spaces"))
                and not input_advanced(samples, index, ("physical_pointer", "physical_other"), DESKTOP_ATTRIBUTION_MS)):
            yield previous, sample, "foreground or visible Space changed without physical input"


def attributed_change(samples, spans=None):
    """Fail changes that neither physical input nor another process can explain."""
    return next((reason for previous, sample, reason in unexplained_changes(samples)
                 if cueward_could_act(spans, previous, sample)), None)


def external_changes(samples, spans):
    """Count changes made while no Cueward command could have made them."""
    return sum(not cueward_could_act(spans, previous, sample)
               for previous, sample, _ in unexplained_changes(samples))


def interference(samples, pids, inactive_space, space, spans=None):
    baseline = samples[0]
    for sample in samples:
        if sample["frontmost_pid"] in pids or any(sample["target_active"][str(pid)] for pid in pids):
            return "failed", "receiver became foreground or active"
    if all(has_input_counts(sample) for sample in samples):
        reason = attributed_change(samples, spans)
        if reason:
            return "failed", reason
    elif any(sample[key] != baseline[key] for sample in samples for key in ("frontmost_pid", "pointer", "visible_spaces")):
        return "failed", "foreground, pointer or visible Space changed"
    if inactive_space:
        if not isinstance(space, str) or not space:
            return "unverified", "missing inactive target Space identity"
        if any(space in s["visible_spaces"].values() for s in samples):
            return "failed", "target window was on a visible Space"
    return "passed", "sampled execution coverage within 100 ms; shorter transients remain outside resolution"


def observer_checks(execution, pids):
    """Inspect optional native-observer evidence without authenticating its provenance."""
    observer = execution.get("observer")
    if observer is None:
        return None
    if not isinstance(observer, dict) or observer.get("schema") != 1 or observer.get("receiver_pids") != pids:
        return "unverified", "native observer receiver scope is incomplete or mismatched"
    events = execution.get("events")
    if not isinstance(events, list) or not all(isinstance(e, dict) and number(e.get("ms")) for e in events):
        return "unverified", "native observer event evidence is incomplete"
    if any(e.get("type") in ("observation_error", "control_error") for e in events):
        return "unverified", "native observer failed within the execution interval"
    if any(e.get("type") == "activation" and e.get("pid") in pids for e in events):
        return "failed", "receiver activation observed between samples"
    if any(s.get("session_locked") is not False for s in execution.get("samples", [])):
        return "unverified", "desktop locked or session state unavailable during execution"
    return None


def check_trace(evidence, inactive_space=False):
    """Return status/reason and metrics from an independently collected trace."""
    execution = evidence.get("execution", {})
    start, end = execution.get("start_ms"), execution.get("end_ms")
    samples = execution.get("samples", [])
    pids = evidence.get("receiver_pids", [])
    receipts = evidence.get("receipts", [])
    if not (number(start) and number(end) and end > start):
        return "unverified", "missing execution interval", {}
    if not (isinstance(pids, list) and pids and all(type(p) is int and p > 0 for p in pids)):
        return "unverified", "missing owned receiver PIDs", {}
    if not isinstance(receipts, list) or not receipts:
        return "unverified", "missing command receipts", {}
    if not all(isinstance(r, dict) and isinstance(r.get("command"), str)
               and r.get("command") and number(r.get("elapsed_ms"))
               and r["elapsed_ms"] >= 0 and isinstance(r.get("status"), str) for r in receipts):
        return "unverified", "incomplete command metrics", {}
    metrics = {"command_count": len(receipts), "task_wall_ms": end - start,
               "tool_wall_ms": sum(r["elapsed_ms"] for r in receipts)}
    if not isinstance(samples, list) or len(samples) < 3:
        return "unverified", "endpoint-only or missing observation", metrics
    if not all(valid_sample(s, pids) for s in samples):
        return "unverified", "incomplete foreground/pointer/Space/activation samples", metrics
    observed = observer_checks(execution, pids)
    if observed:
        return *observed, metrics
    metrics["input_attributed"] = all(has_input_counts(sample) for sample in samples)
    spans = command_spans(receipts, execution.get("marks"))
    if spans is not None and metrics["input_attributed"]:
        metrics["external_desktop_changes"] = external_changes(samples, spans)
    status, reason = interference(samples, pids, inactive_space, evidence.get("target_space"), spans)
    if status == "failed":
        return status, reason, metrics
    times = [s["ms"] for s in samples]
    if any(b <= a for a, b in zip(times, times[1:])):
        return "unverified", "nonmonotonic observation times", metrics
    if times[0] > start or times[-1] < end:
        return "unverified", "execution interval not covered", metrics
    gaps = [b - a for a, b in zip(times, times[1:])]
    metrics["observer_max_interval_ms"] = max(gaps)
    metrics["observer_max_command_interval_ms"] = command_coverage_gap(samples, spans)
    if metrics["observer_max_command_interval_ms"] > MAX_INTERVAL_MS:
        return "unverified", "observation gap exceeds 100 ms", metrics
    return status, reason, metrics
