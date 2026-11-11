"""Exercise both diagnostic entry points at the mocked native CLI boundary."""

import copy
import importlib.util
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from observed_cli import ObservedCLI, panel_controls
from ax_exploration import explore_windows

SPEC = importlib.util.spec_from_file_location("first_save_exploration", Path(__file__).with_name("first-save-probe.py"))
BASELINE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BASELINE)


class NativeTree:
    """Finite synthetic AX trees; no application, process or desktop is created."""

    def __init__(self, *, enabled=True, save_first=False, change_app_at=None, change_root_at=None):
        self.calls = []
        self.change_app_at = change_app_at
        self.change_root_at = change_root_at
        save_index, group_index = (0, 1) if save_first else (1, 0)
        self.nodes = {}
        self.add("w0", "AXWindow", 3 if save_first else 2)
        self.add(f"w0.{group_index}", "AXGroup", 498)
        for index in range(498):
            self.add(f"w0.{group_index}.{index}", "AXStaticText", 0)
        self.add(f"w0.{save_index}", "AXButton", 0, name="Save", enabled=enabled)
        if save_first:
            self.add("w0.2", "AXStaticText", 0)

    def add(self, ref, role, child_count, **fields):
        node = dict(ref=ref, role=role, child_count=child_count, receiver_pid=123,
                    actions=["AXPress"] if role == "AXButton" else [], **fields)
        if "." in ref:
            node["parent_ref"] = ref.rsplit(".", 1)[0]
        self.nodes[ref] = node

    def run(self, command, **kwargs):
        self.calls.append(command)
        if command[1:3] != ["app", "inspect"]:
            raise AssertionError("exploration may only read the native CLI")
        def option(flag, default=None):
            return command[command.index(flag) + 1] if flag in command else default
        root = option("--root")
        depth, limit = int(option("--depth", 1)), int(option("--limit", 500))
        result, truncated = [], False
        def visit(ref, level):
            nonlocal truncated
            if len(result) >= limit:
                truncated = True
                return
            node = copy.deepcopy(self.nodes[ref])
            if node.get("role") == "AXButton" and node.get("enabled"):
                node["target"] = f"target:{ref}:read-{len(self.calls)}"
            result.append(node)
            if level >= depth:
                truncated |= node["child_count"] > 0
                return
            for index in range(node["child_count"]):
                visit(f"{ref}.{index}", level + 1)
        if root is None:
            result = [copy.deepcopy(self.nodes["w0"])]
        else:
            visit(root, 0)
        if len(self.calls) == self.change_root_at:
            result[0]["name"] = "different window"
        app = dict(pid=123, start_seconds=2 if len(self.calls) == self.change_app_at else 1,
                   start_micros=0, executable="/mock/owned-receiver")
        payload = json.dumps(dict(nodes=result, truncated=truncated, app=app, root_ref=root))
        return SimpleNamespace(returncode=0, stdout=payload if kwargs.get("text") else payload.encode(), stderr="")

    def call(self, *arguments):
        return json.loads(self.run(["/mock/cueward", *map(str, arguments)], text=True).stdout)


class ExplorationTests(unittest.TestCase):
    def explore(self, route, tree):
        with patch("subprocess.run", side_effect=tree.run):
            if route == "baseline":
                return BASELINE.explore(Path("/mock/cueward"), 123)
            return ObservedCLI("/mock/cueward", Mock(mark=Mock(return_value="mark"))).explore(123)

    def test_both_probes_find_save_beyond_the_first_500_nodes(self):
        for route in ("baseline", "observed"):
            with self.subTest(route=route):
                nodes, incomplete = self.explore(route, NativeTree())
                self.assertEqual(len(panel_controls(nodes)), 1)
                self.assertFalse(incomplete)

    def test_changed_app_instance_stops_instead_of_combining_observations(self):
        for route in ("baseline", "observed"):
            with self.subTest(route=route):
                with self.assertRaisesRegex(RuntimeError, "changed"):
                    self.explore(route, NativeTree(change_app_at=2))

    def test_repeated_subtree_reads_do_not_duplicate_save_controls(self):
        for route in ("baseline", "observed"):
            with self.subTest(route=route):
                nodes, _ = self.explore(route, NativeTree(save_first=True))
                self.assertEqual(len(panel_controls(nodes)), 1)
                self.assertEqual(len({node["ref"] for node in nodes}), len(nodes))

    def test_recovered_disabled_save_does_not_gain_an_action_target(self):
        for route in ("baseline", "observed"):
            with self.subTest(route=route):
                nodes, _ = self.explore(route, NativeTree(enabled=False))
                controls = panel_controls(nodes)
                self.assertEqual(len(controls), 1)
                self.assertFalse(controls[0]["enabled"])
                self.assertNotIn("target", controls[0])

    def test_changed_visible_root_stops_during_continuation_or_final_validation(self):
        for route in ("baseline", "observed"):
            for change_at in (3, 4):
                with self.subTest(route=route, change_at=change_at):
                    with self.assertRaisesRegex(RuntimeError, "changed"):
                        self.explore(route, NativeTree(change_root_at=change_at))

    def test_equal_save_labels_remain_two_distinct_controls(self):
        tree = NativeTree(save_first=True)
        tree.add("w0.2", "AXButton", 0, name="Save", enabled=True)
        nodes, incomplete = self.explore("observed", tree)
        self.assertEqual(len(panel_controls(nodes)), 2)
        self.assertFalse(incomplete)

    def test_continuation_reaches_but_never_exceeds_the_native_path_limit(self):
        for save_depth in (12, 13):
            with self.subTest(save_depth=save_depth):
                tree = NativeTree()
                tree.nodes = {}
                for depth in range(save_depth):
                    tree.add("w0" + ".0" * depth, "AXGroup", 1)
                tree.add("w0" + ".0" * save_depth, "AXButton", 0, name="Save", enabled=True)
                nodes, incomplete = self.explore("observed", tree)
                self.assertEqual(len(panel_controls(nodes)), 1 if save_depth == 12 else 0)
                self.assertEqual(incomplete, save_depth > 12)
                self.assertTrue(all(node["ref"].count(".") <= 12 for node in nodes))


class ExplorationBudgetTests(unittest.TestCase):
    def test_final_root_truncation_cannot_report_complete_coverage(self):
        tree = NativeTree()
        def call(*args):
            result = tree.call(*args)
            if "--root" not in args and len(tree.calls) > 1:
                result["truncated"] = True
            return result
        nodes, incomplete = explore_windows(call, 123)
        self.assertTrue(incomplete)
        self.assertEqual(len(panel_controls(nodes)), 1)

    def test_call_limit_keeps_partial_coverage_without_guessing_hidden_refs(self):
        tree = NativeTree()
        nodes, incomplete = explore_windows(tree.call, 123, max_calls=3)
        self.assertTrue(incomplete)
        self.assertEqual(len(tree.calls), 3)
        self.assertEqual(panel_controls(nodes), [])

    def test_node_limit_bounds_retained_observations(self):
        tree = NativeTree()
        nodes, incomplete = explore_windows(tree.call, 123, max_nodes=8)
        self.assertTrue(incomplete)
        self.assertEqual(len(nodes), 8)
        self.assertLessEqual(len(tree.calls), 3)

    def test_expired_budget_removes_targets_when_final_root_check_cannot_run(self):
        tree, clock = NativeTree(save_first=True), [0]
        def call(*args):
            result = tree.call(*args)
            clock[0] += 1
            return result
        nodes, incomplete = explore_windows(call, 123, max_seconds=1.5, clock=lambda: clock[0])
        self.assertTrue(incomplete)
        self.assertEqual(len(tree.calls), 2)
        self.assertEqual(len(panel_controls(nodes)), 1)
        self.assertFalse(any(node.get("target") for node in nodes))

    def test_transport_failure_is_not_replayed(self):
        tree = NativeTree()
        def call(*args):
            if len(tree.calls) == 2:
                raise RuntimeError("native read failed")
            return tree.call(*args)
        with self.assertRaisesRegex(RuntimeError, "native read failed"):
            explore_windows(call, 123)
        self.assertEqual(len(tree.calls), 2)


if __name__ == "__main__":
    unittest.main()
