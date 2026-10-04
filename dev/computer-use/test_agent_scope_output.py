"""No arbitrary snapshot output writes through either spelling of the scoped flag."""
import unittest
from unittest.mock import Mock

from agent_session import AgentSession, ScopeError, validate_command
from test_agent_session import token


class AgentScopeOutputTests(unittest.TestCase):
    def test_attached_output_value_is_rejected_before_any_cli_dispatch(self):
        for action in ("snapshot", "inspect"):
            with self.subTest(action=action):
                session = AgentSession("/mock/cueward", None, 100, 20, "unused", {})
                session.commands.call = Mock(return_value={"status": "observed"})
                result = session.request(["window", action, "--id", "20",
                                          "--output=/operator/should-not-write.png"])
                self.assertTrue(result.get("not_dispatched"), result)
                session.commands.call.assert_not_called()

    def test_separate_output_value_remains_rejected(self):
        with self.assertRaises(ScopeError):
            validate_command(["window", "snapshot", "--id", "20", "--output", "/user/file"], 100, 20)

    def test_literal_output_text_is_not_an_app_scope_flag(self):
        ax = token({"kind": "app_ax", "app": {"pid": 100}})
        args = ["app", "set-value", "--target", ax, "--value", "--output=literal text"]
        self.assertEqual(validate_command(args, 100, 20), args)


if __name__ == "__main__":
    unittest.main()
