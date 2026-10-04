"""Owned process lifecycle and bounded controls for the #34 research probe."""
import json
import math
import os
from pathlib import Path
import plistlib
import select
import subprocess
import time
import uuid

from native_agent_task import stop_receiver

HERE = Path(__file__).resolve().parent


def build_resources(output):
    """Build two local executables; never select or instrument a user's app."""
    output = Path(output)
    bundle = output / 'LockReceiver.app'
    receiver = bundle / 'Contents/MacOS/LockReceiver'
    receiver.parent.mkdir(parents=True)
    (bundle / 'Contents/Info.plist').write_bytes(plistlib.dumps({
        'CFBundleExecutable': receiver.name, 'CFBundleIdentifier': 'dev.cueward.LockReceiver.' + uuid.uuid4().hex,
        'CFBundlePackageType': 'APPL', 'CFBundleName': 'Owned lock-resource diagnostic', 'LSUIElement': True,
        'NSPrincipalClass': 'NSApplication',
    }))
    helper = output / 'LockResourceProbe'
    for binary, names in ((receiver, ['lock-pattern.swift', 'lock-resource-fixture.swift']),
                          (helper, ['lock-pattern.swift', 'lock-capture.swift', 'lock-resource-ax.swift', 'lock-resource-main.swift'])):
        with (output / (binary.name + '.compile.stdout.log')).open('xb') as out, \
                (output / (binary.name + '.compile.stderr.log')).open('xb') as err:
            subprocess.run(['swiftc', '-swift-version', '6', '-warnings-as-errors', '-parse-as-library',
                            *[str(HERE / name) for name in names], '-o', str(binary)],
                           stdout=out, stderr=err, check=True, timeout=90)
    return receiver, helper


def launch_receiver(binary, output, run_id, duration):
    """Return a retained owned child handle; only synthetic state is recorded."""
    output = Path(output)
    with (output / 'receiver.stdout.log').open('xb') as out, (output / 'receiver.stderr.log').open('xb') as err:
        return subprocess.Popen([str(binary)], stdout=out, stderr=err, start_new_session=True,
                                env=os.environ | {'CUEWARD_STATE': str(output / 'receiver.json'),
                                                  'CUEWARD_RUN_ID': run_id, 'CUEWARD_LIFETIME': str(duration)})


def owned_state(process, path, run_id, *, window_id=None, after_uptime=0, after_sequence=0, timeout=3):
    """Wait for an atomic, independently timestamped owned receiver acknowledgement."""
    deadline = time.monotonic() + timeout
    while process.poll() is None and time.monotonic() < deadline:
        try:
            value = json.loads(Path(path).read_text())
        except FileNotFoundError:
            time.sleep(.03)
            continue
        if (value.get('run_id') != run_id or type(value.get('pid')) is not int or value['pid'] != process.pid
                or type(value.get('window_id')) is not int or value['window_id'] <= 0
                or window_id is not None and value['window_id'] != window_id):
            raise RuntimeError('receiver ownership changed')
        for key in ('sequence', 'presses', 'activations'):
            if type(value.get(key)) is not int or value[key] < (1 if key == 'sequence' else 0):
                raise RuntimeError('invalid receiver counter')
        uptime = value.get('uptime')
        if type(uptime) not in (int, float) or not math.isfinite(uptime) or uptime <= 0:
            raise RuntimeError('invalid receiver timestamp')
        if value.get('active') is not False or value['activations'] != 0:
            raise RuntimeError('owned receiver became active; stop diagnostic')
        if uptime > after_uptime and value['sequence'] > after_sequence:
            return value
        time.sleep(.03)
    raise RuntimeError('owned receiver did not provide a fresh bounded acknowledgement')


class ResourceControl:
    """Bounded newline transport. Partial output never turns select into an unbounded read."""
    def __init__(self, binary, config, output):
        self.pending = b''
        self.bytes_read = 0
        self.stderr = (Path(output) / 'probe.stderr.log').open('xb')
        self.raw = (Path(output) / 'probe.stdout.log').open('xb')
        self.requests = (Path(output) / 'probe.requests.jsonl').open('xb')
        try:
            self.process = subprocess.Popen([str(binary), json.dumps(config)], stdin=subprocess.PIPE,
                                            stdout=subprocess.PIPE, stderr=self.stderr, bufsize=0)
        except BaseException:
            self.stderr.close(); self.raw.close(); self.requests.close()
            raise

    def receive(self, timeout=10):
        deadline = time.monotonic() + timeout
        while b'\n' not in self.pending:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not select.select([self.process.stdout], [], [], max(0, remaining))[0]:
                raise TimeoutError('resource probe response deadline')
            data = os.read(self.process.stdout.fileno(), 4096)
            if not data:
                raise RuntimeError('resource probe exited before response')
            self.raw.write(data); self.raw.flush()
            self.pending += data
            self.bytes_read += len(data)
            if len(self.pending) > 65536 or self.bytes_read > 64 * 1024 * 1024:
                raise RuntimeError('resource probe output bound exceeded')
        line, self.pending = self.pending.split(b'\n', 1)
        value = json.loads(line)
        if not isinstance(value, dict) or value.get('kind') == 'error':
            raise RuntimeError('resource probe error: ' + str(value.get('error') if isinstance(value, dict) else value))
        return value

    def command(self, value, timeout=10):
        message = json.dumps(value).encode() + b'\n'
        self.requests.write(message); self.requests.flush()
        self.process.stdin.write(message)
        self.process.stdin.flush()
        return self.receive(timeout)

    def close(self):
        error = None
        try:
            if self.process.poll() is None:
                response = self.command({'command': 'stop'}, timeout=5)
                if response.get('kind') != 'finished':
                    raise RuntimeError('missing resource shutdown acknowledgement')
                if response.get('capture_stop', {}).get('error'):
                    raise RuntimeError('capture shutdown not confirmed: ' + str(response['capture_stop']['error']))
                self.process.wait(timeout=5)
            if self.process.returncode != 0:
                raise RuntimeError('resource probe shutdown exit ' + str(self.process.returncode))
        except BaseException as caught:
            error = caught
        finally:
            stop_receiver(self.process)
            for pipe in (self.process.stdin, self.process.stdout):
                if pipe is not None: pipe.close()
            self.stderr.close(); self.raw.close(); self.requests.close()
        if error:
            raise error


def cleanup_resources(receiver, control, binary):
    """Always clean both retained children; unregister only this newly owned bundle."""
    errors = []
    for name, operation in (('probe', lambda: control.close() if control else None),
                            ('receiver', lambda: stop_receiver(receiver))):
        try:
            operation()
        except BaseException as error:
            errors.append(name + ': ' + str(error))
    if receiver is not None and binary is not None:
        tool = '/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister'
        try:
            subprocess.run([tool, '-u', str(Path(binary).parents[2])], capture_output=True, check=True, timeout=10)
        except BaseException as error:
            errors.append('bundle_unregister: ' + str(error))
    return {'status': 'failed' if errors else 'passed', 'errors': errors}
