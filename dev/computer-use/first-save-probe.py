#!/usr/bin/env python3
"""Use observed generic AX controls to test a native document's first save."""
import argparse
import json
import os
from pathlib import Path
import platform
import plistlib
import signal
import subprocess
import tempfile
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
CONTENT = "Cueward first-save artifact αβγ\n拋棄式文件驗收\n"


def run(command):
    result = subprocess.run(command, capture_output=True, timeout=45)
    if result.returncode:
        raise RuntimeError(result.stderr.decode().strip())
    text = result.stdout.decode().strip()
    if text.startswith("<external "):
        text = text.split("\n", 1)[1].rsplit("\n</external>", 1)[0]
    return json.loads(text) if text else None


def explore(cli, pid):
    roots = run([str(cli), "app", "inspect", "--pid", str(pid)])
    nodes, truncated = [], False
    for root in roots["nodes"]:
        if root["ref"].startswith("w"):
            tree = run([str(cli), "app", "inspect", "--pid", str(pid), "--root", root["ref"], "--depth", "10", "--limit", "500"])
            nodes.extend(tree["nodes"])
            truncated |= tree["truncated"]
    return nodes, truncated


def wait_file(path, timeout=5):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.exists():
            return
        time.sleep(0.05)
    raise RuntimeError("expected disposable file was not created")


def make_reader(directory):
    bundle = directory / "Reader.app"
    executable = bundle / "Contents/MacOS/Reader"
    executable.parent.mkdir(parents=True)
    subprocess.run(["swiftc", str(ROOT / "crates/adapter-macos/src/apps/open_live_fixture.swift"), "-o", str(executable)], check=True, timeout=45)
    (bundle / "Contents/Info.plist").write_bytes(plistlib.dumps({
        "CFBundleExecutable": "Reader", "CFBundleIdentifier": "dev.cueward.FirstSaveReaderFixture",
        "CFBundleName": "Cueward First Save Reader", "CFBundlePackageType": "APPL",
    }))
    return bundle


def probe(cli, inactive_space):
    with tempfile.TemporaryDirectory(prefix="cueward-first-save-") as temporary:
        directory = Path(temporary)
        binary = directory / "DocumentFixture"
        subprocess.run(["swiftc", str(HERE / "document-fixture.swift"), "-o", str(binary)], check=True, timeout=45)
        state_path = directory / "state.json"
        receiver = subprocess.Popen([str(binary), str(state_path), str(directory)], start_new_session=True)
        reader_pid = None
        try:
            wait_file(state_path)
            state = json.loads(state_path.read_text())
            if state["active"]:
                raise RuntimeError("document fixture became foreground")
            window_id = state["window_id"]
            if inactive_space:
                spaces = run([str(cli), "space", "list"])
                choices = [s["id"] for d in spaces["displays"] for s in d["spaces"] if s["type"] == 0 and not s["is_visible"]]
                if not choices:
                    raise RuntimeError("no existing inactive Space; no desktop created")
                membership = run([str(cli), "space", "window", "--id", str(window_id)])
                moved = run([str(cli), "space", "move-window", "--target", membership["move_target"], "--space", str(choices[0])])
                if moved["status"] != "confirmed" or moved["foreground_changed"] or moved["visible_spaces_changed"]:
                    raise RuntimeError("Space move did not preserve endpoints")
            nodes, _ = explore(cli, receiver.pid)
            editors = [node for node in nodes if node.get("identifier") == "draft-editor" and node.get("target")]
            if len(editors) != 1:
                raise RuntimeError("editor target is not uniquely observed")
            edited = run([str(cli), "app", "set-value", "--target", editors[0]["target"], "--value", CONTENT])
            if edited["status"] != "confirmed" or edited["foreground_changed"]:
                raise RuntimeError("document edit was not confirmed in background")
            nodes, _ = explore(cli, receiver.pid)
            buttons = [node for node in nodes if node.get("identifier") == "save-document" and node.get("target")]
            if len(buttons) != 1:
                raise RuntimeError("document Save command is not uniquely observed")
            requested = run([str(cli), "app", "press", "--target", buttons[0]["target"]])
            if requested["foreground_changed"]:
                raise RuntimeError("Save request changed foreground endpoints")
            time.sleep(0.3)
            nodes, truncated = explore(cli, receiver.pid)
            panel_saves = [node for node in nodes if node.get("identifier") != "save-document" and node["role"] == "AXButton" and node["name"] in ("Save", "儲存")]
            report = {"macos": platform.mac_ver()[0], "architecture": platform.machine(), "inactive_space": inactive_space,
                "observer_interval_ms": 10, "new_document_edit_confirmed": True,
                "panel_traversal_truncated": truncated,
                "save_controls": [{"enabled": n.get("enabled"), "has_target": bool(n.get("target"))} for n in panel_saves],
                "file_created": False, "file_content_matches": False, "cross_app_content_matches": False,
                "status": "save_control_unavailable"}
            if len(panel_saves) == 1 and panel_saves[0].get("target"):
                saved = run([str(cli), "app", "press", "--target", panel_saves[0]["target"]])
                if saved["foreground_changed"]:
                    raise RuntimeError("panel Save action changed foreground endpoints")
                artifact = directory / "artifact.txt"
                wait_file(artifact)
                report["file_created"] = True
                report["file_content_matches"] = artifact.read_bytes() == CONTENT.encode()
                reader = make_reader(directory)
                opened = run([str(cli), "app", "open", "--path", str(reader), "--file", str(artifact), "--new-instance"])
                reader_pid = opened["app"]["pid"]
                reader_state = directory / f"state-{reader_pid}.json"
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    if reader_state.exists():
                        loaded = json.loads(reader_state.read_text())
                        if loaded["documents"]:
                            report["cross_app_content_matches"] = loaded["documents"][0].get("content") == CONTENT
                            report["cross_app_activations"] = loaded["activations"]
                            break
                    time.sleep(0.05)
                report["status"] = "completed" if report["file_content_matches"] and report["cross_app_content_matches"] else "artifact_verification_failed"
            state = json.loads(state_path.read_text())
            report.update({key: state[key] for key in ("active", "activations", "foreground_changes", "save_requests")})
            return report
        finally:
            if receiver.poll() is None:
                os.killpg(receiver.pid, signal.SIGTERM)
            receiver.wait(timeout=5)
            if reader_pid:
                try:
                    os.kill(reader_pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--inactive-space", action="store_true")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    session = subprocess.run(["swift", "-e", 'import CoreGraphics; guard let s = CGSessionCopyCurrentDictionary() as? [String:Any] else { exit(1) }; print(s["CGSSessionScreenIsLocked"] as? Bool == true ? "locked" : "unlocked")'], capture_output=True, check=True, timeout=10)
    if session.stdout.decode().strip() != "unlocked":
        raise RuntimeError("desktop is locked; no document or AX action was started")
    report = probe(args.cli.resolve(), args.inactive_space)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
