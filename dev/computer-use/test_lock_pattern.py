"""Compile/run the actual pure visual decoder; does not launch apps or capture a screen."""
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

HERE = Path(__file__).resolve().parent


@unittest.skipUnless(sys.platform == 'darwin' and shutil.which('swiftc'), 'macOS Swift required')
class LockPatternTests(unittest.TestCase):
    def test_actual_bgra_decoder(self):
        with tempfile.TemporaryDirectory(prefix='cueward-pattern-test-') as directory:
            binary = Path(directory) / 'pattern-test'
            subprocess.run(['swiftc', '-swift-version', '6', '-strict-concurrency=complete',
                            '-warnings-as-errors', str(HERE / 'lock-pattern.swift'),
                            str(HERE / 'test-lock-pattern.swift'), '-o', str(binary)],
                           capture_output=True, check=True, timeout=90)
            result = subprocess.run([str(binary)], capture_output=True, text=True, check=True, timeout=10)
            self.assertIn('12', result.stdout)


if __name__ == '__main__':
    unittest.main()
