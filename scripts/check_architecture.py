#!/usr/bin/env python3
"""Check declared local production/build boundaries; this does not prove DDD ownership."""
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def violations(metadata, allowed):
    members = set(metadata['workspace_members'])
    packages = {p['name']: p for p in metadata['packages'] if p['id'] in members}
    errors = []
    if set(packages) != set(allowed):
        errors.append(f"workspace policy mismatch: unclassified={sorted(set(packages) - set(allowed))}; "
                      f"missing={sorted(set(allowed) - set(packages))}")
    for owner, dependencies in allowed.items():
        for dependency in dependencies:
            if dependency not in allowed:
                errors.append(f"unknown policy dependency: {owner} -> {dependency}")
    paths = {Path(p['manifest_path']).resolve().parent: name for name, p in packages.items()}
    edges = {name: set() for name in packages}
    for owner, package in packages.items():
        for dependency in package['dependencies']:
            # Test-only dependencies are deliberate harness relationships, not runtime ownership.
            if dependency['kind'] == 'dev':
                continue
            path = dependency.get('path')
            target = paths.get(Path(path).resolve()) if path else dependency['name']
            if path and target is None:
                errors.append(f"unclassified local dependency: {owner} -> {path}")
                continue
            if target not in packages:
                continue  # Third-party API choice is reviewed, not governed by this graph.
            edges[owner].add(target)
            if target not in allowed.get(owner, []):
                errors.append(f"forbidden {dependency['kind'] or 'normal'} dependency: "
                              f"{owner} -> {target} (target={dependency.get('target')})")
    # Include conditional edges across all targets and features, even if inactive on this host.
    def visit(node, stack, done):
        if node in stack:
            errors.append('production cycle: ' + ' -> '.join([*stack, node]))
            return
        if node in done:
            return
        for target in sorted(edges[node]):
            visit(target, [*stack, node], done)
        done.add(node)
    done = set()
    for node in sorted(edges):
        visit(node, [], done)
    return errors


def main():
    allowed = json.loads((ROOT / 'scripts/architecture.json').read_text())['production_dependencies']
    result = subprocess.run(
        [os.environ.get('CARGO_CMD', 'cargo'), 'metadata', '--locked', '--no-deps', '--format-version', '1'],
        cwd=ROOT, text=True, capture_output=True, timeout=120, check=True)
    errors = violations(json.loads(result.stdout), allowed)
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        return 1
    print('Architecture: local production/build dependency boundaries passed (all declared targets).')
    return 0


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f'Architecture check failed: {error}', file=sys.stderr)
        if isinstance(error, subprocess.CalledProcessError):
            print(error.stderr, file=sys.stderr)
        raise SystemExit(1)
