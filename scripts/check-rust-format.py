#!/usr/bin/env python3
"""Check changed Rust files without rewriting unrelated baseline formatting."""
import argparse
import pathlib
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--base', required=True, help='Commit/ref to compare against')
args = parser.parse_args()
base = subprocess.check_output(
    ['git', 'rev-parse', '--verify', '--end-of-options', args.base + '^{commit}'], text=True
).strip()
changed = subprocess.check_output(
    ['git', 'diff', '--name-only', '-z', '--diff-filter=ACMR', base, '--', '*.rs']
)
untracked = subprocess.check_output(['git', 'ls-files', '--others', '--exclude-standard', '-z', '--', '*.rs'])
files = sorted({p.decode() for p in (changed + untracked).split(b'\0') if p and pathlib.Path(p.decode()).is_file()})
if files:
    raise SystemExit(subprocess.call([
        'rustfmt', '--edition', '2024', '--check', '--config', 'skip_children=true', '--', *files
    ]))
print('No changed Rust files.')
