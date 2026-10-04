"""Actual capture-error producer and report regressions; no capture or desktop app runs."""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

from lock_resources_report import summarize

HERE = Path(__file__).resolve().parent


@unittest.skipUnless(sys.platform == 'darwin' and shutil.which('swiftc'), 'macOS Swift required')
class LockCaptureErrorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        directory = tempfile.TemporaryDirectory(prefix='cueward-capture-errors-test-')
        cls.addClassCleanup(directory.cleanup)
        binary = Path(directory.name) / 'capture-errors-test'
        subprocess.run(['swiftc', '-swift-version', '6', '-strict-concurrency=complete',
                        '-warnings-as-errors', str(HERE / 'lock-pattern.swift'),
                        str(HERE / 'lock-capture.swift'), str(HERE / 'test-lock-capture-errors.swift'),
                        '-o', str(binary)], capture_output=True, check=True, timeout=90)
        result = subprocess.run([str(binary)], capture_output=True, text=True, check=True, timeout=10)
        cls.records = json.loads(result.stdout)

    def test_sdk_permission_errors_are_structured_and_sanitized(self):
        for phase, record in self.records['permission'].items():
            with self.subTest(phase=phase):
                self.assertEqual(record['status'], 'permission_denied')
                self.assertEqual(record['error'], phase + ' failed (code -3801)')
                self.assertIsNone(record['sequence'])
                self.assertEqual(record['frameCount'], 0)
                self.assertGreater(record['uptime'], 0)

    def test_same_code_other_domain_and_permission_words_are_not_guessed(self):
        for name in ('other_domain', 'other_code', 'local_error'):
            self.assertEqual(self.records[name]['status'], 'error')

    def test_terminal_permission_and_shutdown_outcomes_remain_distinct(self):
        self.assertEqual(self.records['terminal_permission']['status'], 'permission_denied')
        self.assertEqual(self.records['confirmed_stop']['status'], 'stopped')
        self.assertIsNone(self.records['confirmed_stop']['error'])
        self.assertEqual(self.records['stop_permission']['status'], 'permission_denied')
        self.assertTrue(self.records['stop_permission']['error'])

    def test_actual_producer_records_reach_both_capture_routes(self):
        for record in [*self.records['permission'].values(), self.records['other_domain'],
                       self.records['other_code'], self.records['terminal_permission']]:
            data = []
            for index in range(2):
                begin = record['uptime'] + index * 2
                before = {'pid': 401, 'window_id': 901, 'sequence': index * 2 + 1, 'uptime': begin}
                after = before | {'sequence': before['sequence'] + 1, 'uptime': begin + 1}
                data.append({'phase': 'before_lock', 'locked': False, 'locked_end': False,
                             'begin_uptime': begin, 'end_uptime': begin + 1,
                             'receiver_before': before, 'receiver_after': after,
                             'observations': {'retained_stream': record, 'new_screenshot': record}})
            report = summarize(data, cleanup={'status': 'passed', 'errors': []})
            for name in ('retained_stream', 'new_screenshot'):
                result = report['phases']['before_lock']['routes'][name]
                self.assertEqual(result['status'], record['status'])
                self.assertEqual(result['outcome_counts'], {record['status']: 2})
                self.assertEqual(result['observed_count'], 2)


if __name__ == '__main__':
    unittest.main()
