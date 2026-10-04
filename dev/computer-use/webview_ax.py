"""Effect checks for the ordinary WebView's existing AXPress route."""

import math
import time


BUTTON_NAME = "Count single clicks"


def select_button(tree, state):
    """Require one enabled page button, not an unnamed window/chrome control."""
    window = tree["window"]
    if window["owner_pid"] != state["pid"] or window["window_id"] != state["window_id"]:
        raise RuntimeError("owned WebView inspection identity mismatch")
    snapshot = tree["accessibility"]
    if snapshot.get("truncated") is not False:
        raise RuntimeError("partial AX tree cannot prove button uniqueness")
    nodes = snapshot["nodes"]
    buttons = [n for n in nodes if n.get("role") == "AXButton" and n.get("name") == BUTTON_NAME]
    if len(buttons) != 1:
        raise RuntimeError("expected exactly one observed page button")
    button = buttons[0]
    if (button.get("enabled") is not True or "AXPress" not in button.get("actions", [])
            or not isinstance(button.get("target"), str) or not button["target"]):
        raise RuntimeError("observed page button is not enabled and pressable")
    if not any(n.get("role") == "AXWebArea" and button["ref"].startswith(n["ref"] + ".") for n in nodes):
        raise RuntimeError("named button is not within the observed web area")
    rectangle = state["dom"]["regions"]["button"]
    origin = window["bounds"]
    expected = {"x": origin["x"] + rectangle["x"],
                "y": origin["y"] + state["content_top"] + rectangle["y"],
                "width": rectangle["width"], "height": rectangle["height"]}
    actual = button.get("bounds", {})
    if any(type(actual.get(k)) not in (int, float) or not math.isfinite(actual[k])
           or abs(actual[k] - v) > 2 for k, v in expected.items()):
        raise RuntimeError("AX button bounds do not match the owned synthetic page")
    return button


def click_effect(before, after):
    """Check a new, exactly-once page effect independently of the AX receipt."""
    a, b = before["dom"], after["dom"]
    prefix_intact = b["events"][:len(a["events"])] == a["events"]
    events = b["events"][len(a["events"]):] if prefix_intact else []
    clicks = [e for e in events if e["type"] == "click"]
    increment = b["clicks"] - a["clicks"]
    native_increment = len(after["native_events"]) - len(before["native_events"])
    identity = after["pid"] == before["pid"] and after["window_id"] == before["window_id"]
    passed = (identity and prefix_intact and increment == 1 and len(clicks) == 1
              and clicks[0].get("trusted") is True and native_increment == 0
              and before["active"] is False and after["active"] is False
              and after["native_events"] == before["native_events"]
              and after["activations"] == before["activations"])
    return {"status": "passed" if passed else "failed", "click_increment": increment,
            "dom_click_count": len(clicks), "dom_event_count": len(events),
            "trusted_click": len(clicks) == 1 and clicks[0].get("trusted") is True,
            "native_mouse_event_increment": native_increment,
            "identity_unchanged": identity, "event_prefix_intact": prefix_intact}


def wait_click(read_state, before, timeout=3):
    """Bound observation of a new AX click; AX need not generate DOM mouse-up."""
    started = time.monotonic()
    while True:
        after = read_state()
        events = after["dom"]["events"][len(before["dom"]["events"]):]
        fresh = after["dom"]["clicks"] != before["dom"]["clicks"] or any(e["type"] == "click" for e in events)
        elapsed = time.monotonic() - started
        if fresh or elapsed >= timeout:
            return after, {"status": "new_click_observed" if fresh else "no_click_within_budget",
                           "wait_ms": elapsed * 1000, "budget_ms": timeout * 1000}
        time.sleep(0.05)
