"""Bounded, read-only AX exploration shared by operator diagnostics."""

from collections import defaultdict
import re
import time


def _identity(node):
    # Targets are opaque and renewed on inspection. Compare only public observations.
    return {key: value for key, value in node.items() if key != "target"}


def _missing_children(nodes):
    children = defaultdict(set)
    for node in nodes.values():
        if "parent_ref" in node:
            children[node["parent_ref"]].add(node["ref"])
    missing = []
    for ref, node in nodes.items():
        count = node.get("child_count")
        if type(count) is not int or count < 0:
            raise RuntimeError("app inspection has no valid child count")
        if len(children[ref]) > count:
            raise RuntimeError("app children changed during exploration; inspect again")
        if len(children[ref]) < count:
            missing.append(ref)
    return sorted(missing, key=lambda ref: ref.count("."))


class _Exploration:
    def __init__(self, call, pid, max_calls, max_nodes, max_seconds, clock):
        self.call, self.pid, self.clock = call, pid, clock
        self.max_calls, self.max_nodes = max_calls, max_nodes
        self.deadline = clock() + max_seconds
        self.calls, self.nodes, self.app = 0, {}, None
        self.node_limit = False
        self.unexplained_truncation = False

    def available(self, reserve=0):
        return self.calls + reserve < self.max_calls and self.clock() < self.deadline

    def read(self, root=None, depth=10):
        arguments = ["app", "inspect", "--pid", self.pid]
        if root is not None:
            arguments += ["--root", root, "--depth", depth, "--limit", 500]
        self.calls += 1
        snapshot = self.call(*arguments)
        app = snapshot.get("app")
        if not isinstance(app, dict) or app.get("pid") != self.pid:
            raise RuntimeError("app inspection has no matching process instance")
        if self.app is not None and app != self.app:
            raise RuntimeError("app instance changed during exploration; inspect again")
        self.app = app
        if snapshot.get("root_ref") != root:
            raise RuntimeError("app inspection returned a different root")
        return snapshot

    def merge(self, batch, root=None):
        for node in batch:
            ref = node.get("ref", "")
            if re.fullmatch(r"w\d+(?:\.\d+){0,12}", ref) is None:
                raise RuntimeError("app inspection returned an invalid window ref")
            if root is not None and ref != root and not ref.startswith(root + "."):
                raise RuntimeError("app inspection returned a different subtree")
            previous = self.nodes.get(ref)
            if previous is not None and _identity(previous) != _identity(node):
                raise RuntimeError("app node changed during exploration; inspect again")
            if previous is None and len(self.nodes) >= self.max_nodes:
                self.node_limit = True
                return
            self.nodes[ref] = node

    def subtree(self, root, depth):
        snapshot = self.read(root, depth)
        batch = snapshot["nodes"]
        if not batch or batch[0].get("ref") != root:
            raise RuntimeError("app subtree root disappeared; inspect again")
        self.merge(batch, root)
        if snapshot.get("truncated", True) and not _missing_children({n["ref"]: n for n in batch}):
            self.unexplained_truncation = True

    def result(self, initial):
        incomplete = (initial.get("truncated", True) or self.node_limit
                      or self.unexplained_truncation or bool(_missing_children(self.nodes)))
        if not self.available():
            # No final root validation is possible. Keep observations, never usable targets.
            return [{key: value for key, value in node.items() if key != "target"}
                    for node in self.nodes.values()], True
        final = self.read()
        if ([_identity(n) for n in initial["nodes"]]
                != [_identity(n) for n in final["nodes"]]):
            raise RuntimeError("app roots changed during exploration; inspect again")
        return list(self.nodes.values()), incomplete or final.get("truncated", True)


def explore_windows(call, pid, *, max_calls=32, max_nodes=4096, max_seconds=8, clock=time.monotonic):
    """Recover omitted branches using observed refs; return nodes and coverage limits.

    The time budget is checked between calls, not inside the transport's own timeout.
    Public app/node observations detect visible changes, not identical replacements or
    an atomic snapshot. Native action-target validation remains required at dispatch.
    """
    if max_calls < 3 or max_nodes < 1 or max_seconds <= 0:
        raise ValueError("invalid AX exploration budget")
    walk = _Exploration(call, pid, max_calls, max_nodes, max_seconds, clock)
    roots = walk.read()
    windows = [node for node in roots["nodes"] if re.fullmatch(r"w\d+", node.get("ref", ""))]
    walk.merge(windows)
    for node in windows:
        if not walk.available(reserve=1) or walk.node_limit:
            break
        walk.subtree(node["ref"], 10)
    expanded = set()
    while walk.available(reserve=1) and not walk.node_limit:
        frontier = [ref for ref in _missing_children(walk.nodes)
                    if ref not in expanded and ref.count(".") < 12]
        if not frontier:
            break
        ref = frontier[0]
        expanded.add(ref)
        # A shallow read exposes siblings hidden by a large earlier descendant.
        # Child paths are never guessed from child_count or replayed after a change.
        walk.subtree(ref, 1)
    return walk.result(roots)
