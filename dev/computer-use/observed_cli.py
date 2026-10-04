"""Record actual CLI receipts alongside a separately running desktop observer."""

import json
import re
import shlex
import subprocess
import time


def parse_cli_output(text):
    """Parse operator data only after validating an external-content envelope."""
    if text.startswith("<external"):
        opening, separator, rest = text.partition("\n")
        payload, closing_separator, closing = rest.rpartition("\n")
        if (not separator or not closing_separator
                or re.fullmatch(r'<external source="[^"<>\r\n]+">', opening) is None
                or closing != "</external>" or "</external>" in payload):
            raise ValueError("malformed external CLI output")
        text = payload.replace("&lt;/external&gt;", "</external>")
    return json.loads(text)


def external_for_agent(value):
    """Keep all CLI-derived data inside the product's external-content boundary."""
    payload = json.dumps(value, ensure_ascii=False).replace("</external>", "&lt;/external&gt;")
    return '<external source="cueward/agent-session">\n' + payload + '\n</external>'


class ObservedCLI:
    """A diagnostic transport, not a recorded task sequence or an agent adapter."""

    def __init__(self, cli, observer):
        self.cli, self.observer = str(cli), observer
        self.receipts = []

    def call(self, *arguments):
        command = [self.cli, *map(str, arguments)]
        before = self.observer.mark()
        start = time.monotonic()
        receipt = {"command": shlex.join(command), "status": "unknown", "start_marker": before}
        try:
            output = subprocess.run(command, capture_output=True, text=True, timeout=45)
            receipt["exit_code"] = output.returncode
            if output.returncode:
                receipt["status"] = "tool_error"
                raise RuntimeError(output.stderr.strip() or "CLI exited without a result")
            value = parse_cli_output(output.stdout.strip())
            receipt["status"] = value.get("status", "observed") if isinstance(value, dict) else "observed"
            return value
        except (OSError, ValueError, subprocess.TimeoutExpired) as error:
            receipt["status"] = "unknown_outcome"
            raise RuntimeError(f"CLI outcome unknown; inspect before retrying: {error}") from error
        finally:
            receipt["elapsed_ms"] = (time.monotonic() - start) * 1000
            self.receipts.append(receipt)
            try:
                receipt["end_marker"] = self.observer.mark()
            except (OSError, RuntimeError) as error:
                receipt["observation_error"] = str(error)
                # A recording failure must not erase an uncertain dispatch outcome.
                # Stop the caller even if the CLI itself succeeded; never retry here.
                if receipt["status"] not in ("tool_error", "unknown_outcome"):
                    raise RuntimeError("CLI returned but trailing observation failed; do not replay") from error

    def explore(self, pid):
        """Discover fresh app roots and read each current window subtree."""
        roots = self.call("app", "inspect", "--pid", pid)
        nodes, incomplete = [], roots.get("truncated", True)
        for root in roots["nodes"]:
            if root["ref"].startswith("w"):
                tree = self.call("app", "inspect", "--pid", pid, "--root", root["ref"], "--depth", 10, "--limit", 500)
                nodes.extend(tree["nodes"])
                incomplete |= tree.get("truncated", True)
        return nodes, incomplete


def panel_controls(nodes):
    """Return currently observed Save controls, excluding the fixture's document command."""
    sheets = [node["ref"] for node in nodes if node.get("role") == "AXSheet"
              and isinstance(node.get("ref"), str) and "." not in node["ref"]]
    # AppKit may expose a sheet both under its parent and as an app root.
    # Select the independently observed sheet roots, without guessing that equal
    # labels/bounds mean equal elements. Multiple sheets remain ambiguous.
    return [node for node in nodes if node.get("identifier") != "save-document"
            and node.get("role") == "AXButton" and node.get("name") in ("Save", "儲存")
            and (not sheets or any(node.get("ref", "").startswith(ref + ".") for ref in sheets))]


def sole_target(nodes, identifier):
    """Require one current actionable control; AX text may omit AXEnabled."""
    matches = [n for n in nodes if n.get("identifier") == identifier and n.get("target") and n.get("enabled") is not False]
    if len(matches) != 1:
        raise RuntimeError(f"control {identifier} is not uniquely observed and enabled")
    return matches[0]["target"]
