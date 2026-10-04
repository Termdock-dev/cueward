#!/usr/bin/env python3
"""Observe one already-owned fresh-agent task; commands arrive as JSON arrays."""
import argparse
from pathlib import Path
import tempfile

from agent_session import AgentSession
from desktop_observer import DesktopObserver, compile_observer


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--pid", type=int, required=True)
    parser.add_argument("--window-id", type=int, required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--task-id", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--socket", type=Path)
    args = parser.parse_args()
    metadata = {"run_id": args.run_id, "task_id": args.task_id, "attempted": True}
    with tempfile.TemporaryDirectory(prefix="cueward-agent-observer-") as directory:
        binary = compile_observer(directory)
        with DesktopObserver(binary, [args.pid], args.output.with_suffix(".trace.jsonl"), duration_ms=900_000) as observer:
            session = AgentSession(args.cli, observer, args.pid, args.window_id, args.output, metadata)
            if args.socket:
                session.serve_socket(args.socket)
            else:
                session.serve()


if __name__ == "__main__":
    main()
