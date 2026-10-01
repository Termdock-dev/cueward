"""Validate supplied execution samples; do not infer coverage from endpoints."""

import math

MAX_INTERVAL_MS = 100


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


def interference(samples, pids, inactive_space, space):
    baseline = samples[0]
    for sample in samples:
        if any(sample[key] != baseline[key] for key in ("frontmost_pid", "pointer", "visible_spaces")):
            return "failed", "foreground, pointer or visible Space changed"
        if sample["frontmost_pid"] in pids or any(sample["target_active"][str(pid)] for pid in pids):
            return "failed", "receiver became foreground or active"
    if inactive_space:
        if not isinstance(space, str) or not space:
            return "unverified", "missing inactive target Space identity"
        if any(space in s["visible_spaces"].values() for s in samples):
            return "failed", "target window was on a visible Space"
    return "passed", "sampled execution coverage within 100 ms; shorter transients remain outside resolution"


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
    status, reason = interference(samples, pids, inactive_space, evidence.get("target_space"))
    if status == "failed":
        return status, reason, metrics
    times = [s["ms"] for s in samples]
    if any(b <= a for a, b in zip(times, times[1:])):
        return "unverified", "nonmonotonic observation times", metrics
    if times[0] > start or times[-1] < end:
        return "unverified", "execution interval not covered", metrics
    gaps = [b - a for a, b in zip(times, times[1:])]
    metrics["observer_max_interval_ms"] = max(gaps)
    if max(gaps) > MAX_INTERVAL_MS:
        return "unverified", "observation gap exceeds 100 ms", metrics
    return status, reason, metrics
