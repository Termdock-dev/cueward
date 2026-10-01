#!/usr/bin/env python3
"""Prepare/evaluate issue #36 tasks without scripted desktop actions."""

import argparse
import json
from pathlib import Path
import sys

from task_acceptance.evaluate import evaluate, load_json
from task_acceptance.setup import prepare, write_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    setup = commands.add_parser("prepare", help="Create a new disposable run; never reuse or delete an old run")
    setup.add_argument("--parent", type=Path, required=True)
    verify = commands.add_parser("evaluate", help="Check artifacts and independently supplied receiver/trace evidence")
    verify.add_argument("--run", type=Path, required=True)
    verify.add_argument("--evidence", type=Path, required=True)
    verify.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "prepare":
            print(json.dumps({"run": str(prepare(args.parent))}))
            return 0
        output = args.output.resolve()
        root = args.run.resolve()
        reserved = {root / name for name in ("manifest.json", "agent-goals.json", "evidence-template.json")}
        if output in reserved or output == args.evidence.resolve() or any(output.is_relative_to(root / name) for name in ("work", "seeds")):
            raise ValueError("report output must not overwrite prepared data, artifacts or evidence")
        report = evaluate(args.run, load_json(args.evidence))
        write_json(args.output, report)
        print(json.dumps({"report": str(args.output), **report["summary"]}))
        return 0 if report["summary"]["passed"] == report["denominator"] else 1
    except (OSError, ValueError, TypeError, AttributeError, KeyError) as error:
        print(f"task acceptance: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
