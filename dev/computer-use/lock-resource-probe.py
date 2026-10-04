#!/usr/bin/env python3
"""Prepare #34 retained-resource research and a bounded owned-window observation run."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import time
import uuid

from lock_resources import ResourceControl, build_resources, cleanup_resources, launch_receiver, owned_state
from lock_resources_report import summarize
from task_acceptance.setup import write_json


def checkpoint(output, name, record, index=None):
    """Keep a request/response before a later independent acknowledgement can fail."""
    path = output / (name + '.json')
    records = json.loads(path.read_text()) if path.exists() else []
    if index is None:
        index = len(records); records.append(record)
    else:
        records[index] = record
    write_json(path, records)
    return index


def observation(control, receiver, output, run_id, window_id):
    before = owned_state(receiver, output / 'receiver.json', run_id, window_id=window_id)
    sample = control.command({'command': 'sample'})
    if sample.get('kind') != 'sample':
        raise RuntimeError('missing sample response')
    pending = sample | {'receiver_before': before, 'receiver_after': None}
    index = checkpoint(output, 'samples', pending)
    after = owned_state(receiver, output / 'receiver.json', run_id, window_id=window_id,
                        after_uptime=sample['end_uptime'], after_sequence=before['sequence'])
    sample |= {'probe_begin_uptime': sample['begin_uptime'], 'probe_end_uptime': sample['end_uptime'],
               'begin_uptime': before['uptime'], 'end_uptime': after['uptime'],
               'receiver_before': before, 'receiver_after': after}
    checkpoint(output, 'samples', sample, index)
    return sample


def action(control, receiver, output, run_id, window_id, phase, retained):
    before = owned_state(receiver, output / 'receiver.json', run_id, window_id=window_id)
    pending = {'kind': 'action_request', 'phase': phase, 'route': 'retained_ax' if retained else 'new_ax',
               'receiver_before': before, 'dispatch_status': 'unknown', 'effect_status': 'unverified'}
    index = checkpoint(output, 'actions', pending)
    result = control.command({'command': 'action', 'phase': phase, 'retained': retained})
    if result.get('kind') != 'action':
        raise RuntimeError('missing action response')
    result.update(receiver_before=before, effect_status='unverified',
                  dispatch_status='attempted' if result.get('observation', {}).get('attempted') is True else 'guard_rejected')
    checkpoint(output, 'actions', result, index)
    after = owned_state(receiver, output / 'receiver.json', run_id, window_id=window_id,
                        after_uptime=result['observation']['uptime'], after_sequence=before['sequence'])
    result.update(receiver_after=after, effect_status=action_effect(result, before, after, phase))
    checkpoint(output, 'actions', result, index)
    return result


def action_effect(result, before, after, phase):
    """Require a single independently timed effect in the observed lock phase."""
    stable = (type(result.get('locked')) is bool and result['locked'] == result.get('locked_end')
              and result.get('phase') == phase and result['locked'] == (phase == 'locked'))
    event = after.get('press_events', [])[-1:] or [{}]
    event = event[0]
    sent = result.get('observation', {}).get('status') == 'sent_unverified'
    effect = (type(after.get('presses')) is int and type(before.get('presses')) is int
              and after['presses'] == before['presses'] + 1
              and type(event.get('locked')) is bool and event['locked'] == result.get('locked')
              and type(event.get('uptime')) in (int, float)
              and result.get('begin_uptime', float('inf')) <= event['uptime'] <= after['uptime'])
    return 'verified' if stable and sent and effect else 'unverified'


def collect(control, receiver, output, run_id, window_id, mode, duration, diagnostic_input):
    samples, actions, tested = [], [], set()
    deadline = time.monotonic() + duration
    after_count = 0
    ready_announced = False
    with (output / 'records.jsonl').open('x', encoding='utf-8') as log:
        while time.monotonic() < deadline:
            sample = observation(control, receiver, output, run_id, window_id)
            samples.append(sample)
            log.write(json.dumps(sample) + '\n'); log.flush()
            phase = sample['phase']
            if mode == 'baseline' and phase != 'before_lock':
                raise RuntimeError('lock transition occurred during unlocked baseline')
            if phase == 'before_lock' and not ready_announced:
                report = summarize(samples, mode='baseline', cleanup={'status': 'passed', 'errors': []})
                if report['baseline_status'] == 'passed':
                    ready_announced = True
                    print(json.dumps({'status': 'baseline_ready', 'mode': mode, 'run': str(output)}), flush=True)
            if mode == 'lock' and phase == 'locked' and not ready_announced:
                raise RuntimeError('locked before retained-resource baseline established')
            if diagnostic_input and phase in ('before_lock', 'locked', 'after_unlock') and phase not in tested:
                tested.add(phase)
                for retained in (True, False):
                    result = action(control, receiver, output, run_id, window_id, phase, retained)
                    actions.append(result); log.write(json.dumps(result) + '\n'); log.flush()
            after_count = after_count + 1 if phase == 'after_unlock' else 0
            if mode == 'lock' and after_count >= 3:
                break
            time.sleep(.2)
    return samples, actions


def run(parent, mode, duration, diagnostic_input):
    output = Path(tempfile.mkdtemp(prefix='cueward-lock-resources-', dir=parent)).resolve()
    output.chmod(0o700)
    run_id = uuid.uuid4().hex
    sources = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in Path(__file__).parent.glob('lock*')
               if p.is_file() and p.suffix in ('.py', '.swift')}
    write_json(output / 'source-hashes.json', sources)
    write_json(output / 'scope.json', {'run_id': run_id, 'mode': mode, 'duration': duration,
                                     'diagnostic_input': diagnostic_input, 'created_at': time.time()})
    receiver = control = binary = None
    errors = []
    samples, actions = [], []
    try:
        binary, helper = build_resources(output)
        receiver = launch_receiver(binary, output, run_id, duration + 30)
        initial = owned_state(receiver, output / 'receiver.json', run_id)
        control = ResourceControl(helper, {'pid': receiver.pid, 'window_id': initial['window_id'],
                                          'lifetime': duration + 20, 'diagnostic_input': diagnostic_input}, output)
        ready = control.receive(timeout=15)
        if (ready.get('kind') != 'ready' or ready.get('pid') != receiver.pid
                or ready.get('window_id') != initial['window_id'] or ready.get('retained_ax') is not True):
            raise RuntimeError('invalid retained-resource readiness')
        write_json(output / 'ready.json', ready)
        time.sleep(.3)
        samples, actions = collect(control, receiver, output, run_id, initial['window_id'], mode, duration, diagnostic_input)
    except BaseException as error:
        errors.append(str(error))
        for name, default in (('samples', []), ('actions', [])):
            path = output / (name + '.json')
            if path.exists():
                if name == 'samples': samples = json.loads(path.read_text())
                else: actions = json.loads(path.read_text())
    finally:
        cleanup = cleanup_resources(receiver, control, binary)
    report = summarize(samples, mode=mode, errors=errors, cleanup=cleanup)
    report['actions'] = action_report(actions)
    write_json(output / 'report.json', report)
    write_json(output / 'errors.json', {'errors': errors, 'cleanup': cleanup})
    print(json.dumps({'run': str(output), 'report': report}), flush=True)
    return completion_exit(report, mode, errors, cleanup)


def completion_exit(report, mode, errors, cleanup):
    """Lock exit zero means a recorded three-phase study, not supported lock operation."""
    complete = report['baseline_status'] == 'passed' if mode == 'baseline' else (
        report['recording_status'] == 'complete' and report['phase_order_status'] == 'verified')
    return 0 if complete and not errors and cleanup['status'] == 'passed' else 1


def action_report(actions):
    """Distinguish requests, actual dispatch calls and unknown transport outcomes."""
    return {'requests': len(actions), 'attempts': sum(a.get('dispatch_status') == 'attempted' for a in actions),
            'unconfirmed_requests': sum(a.get('dispatch_status') == 'unknown' for a in actions),
            'guard_rejected': sum(a.get('dispatch_status') == 'guard_rejected' for a in actions),
            'verified_effects': sum(a.get('effect_status') == 'verified' for a in actions),
            'scope': 'optional AXPress with fresh-window validation; no CG keyboard/pointer targets'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--parent', type=Path, default=Path(tempfile.gettempdir()))
    parser.add_argument('--mode', choices=('baseline', 'lock'), default='baseline')
    parser.add_argument('--duration', type=int)
    parser.add_argument('--diagnostic-input', action='store_true', help='Permit one owned AXPress per retained/new target per phase; never replay.')
    args = parser.parse_args()
    duration = args.duration if args.duration is not None else (8 if args.mode == 'baseline' else 180)
    if not 4 <= duration <= 480:
        parser.error('duration must be 4..480 seconds')
    # Permissions only: never request or change them or lock the machine automatically.
    subprocess.run(['swift', '-e', 'import ApplicationServices; import CoreGraphics; guard let s=CGSessionCopyCurrentDictionary() as? [String:Any], s["CGSSessionScreenIsLocked"] as? Bool != true, AXIsProcessTrusted(), CGPreflightScreenCaptureAccess() else { exit(1) }'],
                   capture_output=True, check=True, timeout=10)
    raise SystemExit(run(args.parent, args.mode, duration, args.diagnostic_input))


if __name__ == '__main__':
    main()
