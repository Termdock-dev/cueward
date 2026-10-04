"""Real bounded transport and synthetic receiver oracle tests; no desktop APIs run."""
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from lock_resources import ResourceControl, cleanup_resources, owned_state

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('lock_resource_probe', HERE / 'lock-resource-probe.py')
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


class OracleTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.process = subprocess.Popen(['sleep', '10'])
        self.state = {'run_id': 'run', 'pid': self.process.pid, 'window_id': 10, 'sequence': 4,
                      'uptime': 100., 'presses': 0, 'activations': 0, 'active': False}
        self.path = self.root / 'state.json'

    def write(self, **changes):
        self.path.write_text(json.dumps(self.state | changes))

    def test_owned_state_and_changed_identity(self):
        self.write()
        self.assertEqual(owned_state(self.process, self.path, 'run')['sequence'], 4)
        for changes in ({'pid': self.process.pid + 1}, {'run_id': 'other'}, {'window_id': 11}):
            self.write(**changes)
            with self.assertRaises(RuntimeError):
                owned_state(self.process, self.path, 'run', window_id=10)

    def test_malformed_or_active_state_never_acknowledges(self):
        for changes in ({'sequence': True}, {'presses': -1}, {'uptime': True}, {'uptime': float('nan')},
                        {'active': True}, {'activations': 1}, {'window_id': False}):
            self.write(**changes)
            with self.assertRaises(RuntimeError):
                owned_state(self.process, self.path, 'run')

    def test_frozen_oracle_has_bounded_timeout(self):
        self.write()
        with self.assertRaises(RuntimeError):
            owned_state(self.process, self.path, 'run', after_sequence=4, timeout=.05)

    def tearDown(self):
        if self.process.poll() is None:
            self.process.terminate()
        self.process.wait(timeout=3)


class ControlTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)

    def executable(self, source):
        path = self.root / 'helper'
        path.write_text('#!/usr/bin/env python3\n' + source)
        path.chmod(0o700)
        return path

    def test_real_transport_shutdown_ack_and_cleanup(self):
        binary = self.executable('import sys,json\nprint(json.dumps({"kind":"ready"}),flush=True)\n'
                                 'for line in sys.stdin:\n c=json.loads(line)\n'
                                 ' if c["command"]=="stop":\n  print(json.dumps({"kind":"finished"}),flush=True)\n  break\n'
                                 ' print(json.dumps({"kind":"sample"}),flush=True)\n')
        control = ResourceControl(binary, {}, self.root)
        self.assertEqual(control.receive()['kind'], 'ready')
        self.assertEqual(control.command({'command': 'sample'})['kind'], 'sample')
        result = cleanup_resources(None, control, None)
        self.assertEqual(result['status'], 'passed')
        self.assertEqual(control.process.poll(), 0)
        self.assertTrue(control.stderr.closed)
        self.assertIn(b'"sample"', (self.root / 'probe.stdout.log').read_bytes())
        self.assertIn(b'"stop"', (self.root / 'probe.requests.jsonl').read_bytes())

    def test_partial_line_timeout_is_bounded_and_owned_process_stopped(self):
        binary = self.executable('import sys,time\nsys.stdout.write(\'{"kind":"ready"}\\n{\');sys.stdout.flush();time.sleep(10)\n')
        control = ResourceControl(binary, {}, self.root)
        self.assertEqual(control.receive(timeout=5)['kind'], 'ready')
        with self.assertRaises(TimeoutError):
            control.receive(timeout=.05)
        control.process.terminate()
        control.process.wait(timeout=3)
        result = cleanup_resources(None, control, None)
        self.assertEqual(result['status'], 'failed')
        self.assertIsNotNone(control.process.poll())
        self.assertTrue(control.stderr.closed)
        self.assertEqual((self.root / 'probe.stdout.log').read_bytes(), b'{"kind":"ready"}\n{')


class ActionEffectTests(unittest.TestCase):
    def test_no_api_success_or_after_unlock_effect_shortcut(self):
        before = {'presses': 0}
        after = {'presses': 1, 'uptime': 3., 'press_events': [{'locked': True, 'uptime': 2.}]}
        result = {'phase': 'locked', 'locked': True, 'locked_end': True, 'begin_uptime': 1.,
                  'observation': {'status': 'sent_unverified'}}
        self.assertEqual(probe.action_effect(result, before, after, 'locked'), 'verified')
        for bad in (after | {'presses': 0}, after | {'presses': 2},
                    after | {'press_events': [{'locked': False, 'uptime': 2.}]},
                    after | {'press_events': [{'locked': True, 'uptime': .5}]},
                    after | {'press_events': [{'locked': True, 'uptime': float('nan')}]},
                    after | {'press_events': []}):
            self.assertEqual(probe.action_effect(result, before, bad, 'locked'), 'unverified')
        self.assertEqual(probe.action_effect(result | {'locked_end': False}, before, after, 'locked'), 'unverified')
        self.assertEqual(probe.action_effect(result | {'observation': {'status': 'error'}}, before, after, 'locked'), 'unverified')


class CheckpointTests(unittest.TestCase):
    def test_action_response_and_request_survive_oracle_timeout(self):
        from unittest.mock import Mock, patch
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            control = Mock()
            control.command.return_value = {'kind': 'action', 'phase': 'locked', 'route': 'retained_ax',
                                            'observation': {'attempted': True, 'uptime': 2., 'status': 'sent_unverified'}}
            with patch.object(probe, 'owned_state', side_effect=[{'presses': 0, 'sequence': 2}, RuntimeError('frozen oracle')]):
                with self.assertRaises(RuntimeError):
                    probe.action(control, None, output, 'run', 10, 'locked', True)
            records = json.loads((output / 'actions.json').read_text())
            self.assertEqual(len(records), 1)
            self.assertEqual(records[0]['dispatch_status'], 'attempted')
            self.assertEqual(records[0]['effect_status'], 'unverified')
            self.assertEqual(probe.action_report(records)['attempts'], 1)
            with patch.object(probe, 'owned_state', return_value={'presses': 0}):
                control.command.side_effect = TimeoutError('partial response')
                with self.assertRaises(TimeoutError):
                    probe.action(control, None, output, 'run', 10, 'locked', False)
            records = json.loads((output / 'actions.json').read_text())
            self.assertEqual(probe.action_report(records)['unconfirmed_requests'], 1)

    def test_sample_response_survives_oracle_timeout(self):
        from unittest.mock import Mock, patch
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            control = Mock()
            control.command.return_value = {'kind': 'sample', 'phase': 'locked', 'end_uptime': 2.,
                                            'observations': {'retained_ax': {'status': 'error', 'api_code': -25204}}}
            with patch.object(probe, 'owned_state', side_effect=[{'sequence': 2}, RuntimeError('frozen oracle')]):
                with self.assertRaises(RuntimeError):
                    probe.observation(control, None, output, 'run', 10)
            records = json.loads((output / 'samples.json').read_text())
            self.assertIsNone(records[0]['receiver_after'])
            self.assertEqual(records[0]['observations']['retained_ax']['api_code'], -25204)


class CompletionTests(unittest.TestCase):
    def test_missing_lock_cycle_cannot_exit_success_and_recorded_errors_are_not_support(self):
        from test_lock_resources_report import CLEAN, ROUTES, sample, trial
        from lock_resources_report import summarize
        before_only = summarize([sample(index=0), sample(index=1)], cleanup=CLEAN)
        self.assertEqual(probe.completion_exit(before_only, 'baseline', [], CLEAN), 0)
        self.assertEqual(probe.completion_exit(before_only, 'lock', [], CLEAN), 1)
        data = trial()
        for item in data:
            if item['phase'] == 'locked':
                for route in ROUTES:
                    value = item['observations'][route]
                    value.update(status='error', sequence=None)
                    if route.endswith('ax'): value['api_code'] = -25204
        report = summarize(data, mode='lock', cleanup=CLEAN)
        self.assertEqual(report['lock_status'], 'failed')
        self.assertEqual(report['product_operation_support'], 'unverified')
        self.assertEqual(probe.completion_exit(report, 'lock', [], CLEAN), 0)
        self.assertEqual(probe.completion_exit(report, 'lock', ['timeout'], CLEAN), 1)
        self.assertEqual(probe.completion_exit(report, 'lock', [], {'status': 'failed'}), 1)


if __name__ == '__main__':
    unittest.main()
