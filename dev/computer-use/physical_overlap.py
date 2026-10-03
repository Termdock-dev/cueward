"""Assess one actual human/background round without treating human movement as interference."""
import math
import re

from task_acceptance.trace import MAX_INTERVAL_MS, number, observer_checks, valid_sample

TAG = 0x43554549
MODIFIERS = 0x1E0000
SHIFT, COMMAND = 1 << 17, 1 << 20


def result(status, reason, **metrics):
    return {"status": status, "reason": reason, **metrics}


def trace_check(execution, pids, human_pid, target_space):
    samples = execution.get("samples", [])
    start, end = execution.get("start_ms"), execution.get("end_ms")
    if not (pids and all(type(pid) is int and pid > 0 for pid in pids)
            and type(human_pid) is int and human_pid > 0 and human_pid not in pids
            and isinstance(target_space, str) and target_space and isinstance(execution.get("observer"), dict)
            and number(start) and number(end) and end > start and len(samples) >= 3
            and all(valid_sample(s, pids) for s in samples)):
        return result("unverified", "missing complete desktop coverage")
    native = observer_checks(execution, pids)
    if native:
        return result(*native)
    if any(e.get("type") == "activation" and e.get("pid") != human_pid for e in execution["events"]):
        return result("failed", "another foreground App activated between samples")
    times = [s["ms"] for s in samples]
    if times[0] > start or times[-1] < end or any(b <= a for a, b in zip(times, times[1:])):
        return result("unverified", "nonmonotonic or uncovered task interval")
    gap = max(b - a for a, b in zip(times, times[1:]))
    if gap > MAX_INTERVAL_MS:
        return result("unverified", "desktop gap exceeds 100 ms", maximum_gap_ms=gap)
    baseline = samples[0]["visible_spaces"]
    if any(s["frontmost_pid"] != human_pid or s["visible_spaces"] != baseline
           or any(s["target_active"][str(pid)] for pid in pids)
           or target_space in s["visible_spaces"].values() for s in samples):
        return result("failed", "foreground/visible Space/receiver activity changed", maximum_gap_ms=gap)
    return result("passed", "human foreground and background isolation at sampled resolution",
                  maximum_gap_ms=gap, pointer_positions=len({tuple(s["pointer"]) for s in samples}))


def paired_keys(events):
    pending = set()
    for e in events:
        if e["type"] == 10:
            code = e.get("key_code")
            if code is None or (code in pending and not e.get("is_repeat")):
                return False
            if e.get("is_repeat") and code not in pending:
                return False
            pending.add(code)
        elif e["type"] == 11:
            code = e.get("key_code")
            if code not in pending:
                return False
            pending.remove(code)
    return not pending


def background_check(initial, final, first_text, last_text):
    events = final.get("events", [])[len(initial.get("events", [])):]
    downs = [e for e in events if e["type"] == 10]
    expected = first_text + "a" + last_text
    text_ok = final.get("text") == last_text and "".join(e.get("characters", "") for e in downs) == expected
    keys = [e for e in events if e["type"] in (10, 11)]
    modifiers_ok = (len(keys) == 2 * len(expected) and [e["type"] for e in keys] == [10, 11] * len(expected)
                    and all(e["flags"] & MODIFIERS == (COMMAND if i // 2 == len(first_text) else 0)
                            for i, e in enumerate(keys)))
    annotated = bool(events) and all(e.get("tag") == TAG for e in events if e["type"] in (1, 2, 6, 10, 11, 12, 22))
    click_ok = final.get("clicks") == initial.get("clicks", 0) + 1
    a, b = initial.get("object", {}), final.get("object", {})
    drag_ok = (all(number(v.get(k)) for v in (a, b) for k in ("x", "y"))
               and abs(b["x"] - a["x"] - 80) < 0.1 and abs(b["y"] - a["y"] - 40) < 0.1
               and final.get("releases") == initial.get("releases", 0) + 1 and final.get("dragging") is False)
    scroll_ok = number(final.get("scroll_y")) and final["scroll_y"] > initial.get("scroll_y", 0)
    checks = {"text_and_shortcut": text_ok and final.get("select_alls") == initial.get("select_alls", 0) + 1,
              "key_release": paired_keys(events) and bool(downs), "modifier_isolation": modifiers_ok,
              "event_annotations": annotated, "click": click_ok, "drag_and_release": drag_ok, "scroll": scroll_ok}
    return result("passed" if all(checks.values()) else "failed", "receiver content AND event/effect checks", checks=checks)


def human_check(state, first_text, last_text, lower_uptime, upper_uptime):
    events = [e for e in state.get("events", []) if lower_uptime <= e.get("uptime", -1) <= upper_uptime]
    text = state.get("text", "")
    downs = [e for e in events if e["type"] == 10]
    movement = [e for e in events if e["type"] == 5]
    good_text = bool(text) and re.fullmatch(r"[aA\s]+", text) is not None and first_text not in text and last_text not in text
    annotated = all(e.get("tag") != TAG for e in events)
    physical = bool(downs) and bool(movement) and any(e["flags"] & SHIFT for e in downs)
    checks = {"text_matches_challenge": good_text, "no_background_annotations": annotated,
              "shift_typing_and_mouse_movement": physical, "event_log_complete": not state.get("event_log_full")}
    status = "passed" if all(checks.values()) else "failed"
    # A command interval can begin after a physical down. Pair across the whole
    # user-started trial instead; old trials without an explicit finish remain unverified.
    acknowledged = (state.get("phase") == "acknowledged" and number(state.get("started_at"))
                    and number(state.get("uptime")) and type(state.get("physical_modifiers")) is int)
    if acknowledged:
        trial = [e for e in state.get("events", [])
                 if state["started_at"] <= e.get("uptime", -1) <= state["uptime"]]
        trial_downs = [e for e in trial if e["type"] == 10]
        checks["content_matches_key_events"] = "".join(e.get("characters", "") for e in trial_downs) == text
        checks["key_release"] = paired_keys(trial)
        checks["modifier_release"] = state["physical_modifiers"] & MODIFIERS == 0
        if not all(checks.values()):
            status = "failed"
    elif status == "passed":
        status = "unverified"
    return result(status, "owned human text and unannotated event observations; annotations are not hardware authentication",
                  checks=checks, finish_acknowledged=acknowledged,
                  key_downs=len(downs), mouse_moves=len(movement), shift_key_downs=sum(bool(e["flags"] & SHIFT) for e in downs))



def pointer_correlation(execution, human_events):
    origin = execution.get("observer", {}).get("system_uptime_origin")
    if not number(origin):
        return result("unverified", "missing shared uptime origin")
    moves = [e for e in human_events if e.get("type") == 5 and e.get("tag") != TAG
             and number(e.get("uptime")) and isinstance(e.get("pointer"), list)]
    changes, unattributed = 0, 0
    for a, b in zip(execution["samples"], execution["samples"][1:]):
        if a["pointer"] == b["pointer"]:
            continue
        changes += 1
        matching = any(abs((e["uptime"] - origin) * 1000 - b["ms"]) <= 100
                       and math.dist(e["pointer"], b["pointer"]) <= 15 for e in moves)
        if not matching:
            unattributed += 1
    return result("passed" if changes and not unattributed else "unverified",
                  "sampled pointer changes correlated to owned-window/global-mouse observations; not causal authentication or a sub-sample warp guarantee",
                  changes=changes, unattributed_changes=unattributed)


def _evaluate_complete(report):
    execution = report["execution"]
    origin = execution["observer"].get("system_uptime_origin")
    if not number(origin):
        return {"status": "unverified", "reason": "missing shared clock origin"}
    lo, hi = origin + execution["start_ms"] / 1000, origin + execution["end_ms"] / 1000
    checks = {
        "desktop": trace_check(execution, report["receiver_pids"], report["human_pid"], report["target_space"]),
        "background": background_check(report["background_initial"], report["background_final"], report["first_text"], report["last_text"]),
        "human": human_check(report["human_final"], report["first_text"], report["last_text"], lo, hi),
        "pointer_correlation": pointer_correlation(execution, report["human_final"]["events"]),
    }
    marker_times = report["marker_times"]
    overlap = {}
    for action in report["actions"]:
        if action["name"] not in ("text", "shortcut", "replacement", "click", "drag", "scroll"):
            continue
        lower = origin + marker_times[action["receipt"]["start_marker"]] / 1000
        upper = origin + marker_times[action["receipt"]["end_marker"]] / 1000
        physical = [e for e in report["human_final"]["events"] if lower <= e["uptime"] <= upper]
        overlap[action["name"]] = {"key_downs": sum(e["type"] == 10 for e in physical), "mouse_moves": sum(e["type"] == 5 for e in physical)}
    complete = set(overlap) == {"text", "shortcut", "replacement", "click", "drag", "scroll"}
    checks["command_overlap"] = result("passed" if complete and all(v["key_downs"] and v["mouse_moves"] for v in overlap.values()) else "unverified",
                                       "physical event streams overlap each CLI command interval; does not assert simultaneous individual event delivery", actions=overlap)
    checks["controlled_interruption"] = interruption_check(report["actions"])
    checks["foreground_refusal"] = foreground_check(report["actions"])
    checks["midflight_foreground"] = result("unverified", "target becoming foreground during dispatch was not exercised")
    statuses = {v["status"] for v in checks.values()}
    return {"status": "failed" if "failed" in statuses else "unverified" if "unverified" in statuses else "passed", "checks": checks}


def interruption_check(actions):
    matches = [a for a in actions if a.get("name") == "interrupted_drag"]
    if len(matches) != 1:
        return result("unverified", "controlled interruption was not uniquely recorded")
    action = matches[0]
    before, after, outcome = action["before"], action["after"], action.get("result", {})
    events = after.get("events", [])[len(before.get("events", [])):]
    kinds = [e["type"] for e in events]
    stopped = (outcome.get("status") == "partially_sent" and
               "window changed" in outcome.get("interruption", "") and
               type(outcome.get("events_sent")) is int and 2 <= outcome["events_sent"] < 52)
    app_released = (kinds.count(1) == kinds.count(2) == 1 and kinds[-1:] == [2]
                    and all(e.get("tag") == TAG for e in events))
    view_released = (after.get("releases") == before.get("releases", 0) + 1
                     and after.get("dragging") is False)
    callbacks = after.get("canvas_callbacks", {})
    before_downs = before.get("canvas_callbacks", {}).get("down")
    not_started = (number(before_downs) and callbacks.get("down") == before_downs
                   and after.get("dragging") is False)
    triggered = after.get("interruption_triggered") is True
    status = "passed" if stopped and app_released and view_released and triggered else "failed"
    if stopped and app_released and triggered and not_started and not view_released:
        status = "unverified"
    return result(status, "identity change must stop dispatch, deliver original receiver up AND release an accepted gesture",
                  dispatch_stopped=stopped, original_receiver_up=app_released,
                  receiver_gesture_released=view_released, receiver_gesture_not_started=not_started,
                  interruption_triggered=triggered)



def foreground_check(actions):
    matches = [a for a in actions if a.get("name") == "foreground_refusal"]
    if len(matches) != 1:
        return result("unverified", "foreground refusal was not uniquely recorded")
    action = matches[0]
    events = action["after"].get("events", [])[len(action["before"].get("events", [])):]
    receipt = action.get("receipt", {})
    refused = ("target app is in" in action.get("error", "") and "foreground" in action["error"]
               and receipt.get("status") == "tool_error" and receipt.get("exit_code", 0) != 0
               and "result" not in action and action["before"].get("active") is True)
    no_input = not any(e.get("tag") == TAG for e in events)
    return result("passed" if refused and no_input else "failed",
                  "pre-dispatch foreground refusal only; not a mid-flight activation case",
                  refused=refused, no_background_annotations=no_input)


def evaluate(report):
    """Incomplete evidence is unverified, never inferred from a dispatch receipt."""
    try:
        return _evaluate_complete(report)
    except (KeyError, TypeError, ValueError, IndexError) as error:
        return result("unverified", "incomplete physical round evidence: " + str(error))
