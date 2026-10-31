#!/usr/bin/env python3
"""Fresh, bounded verification. No wrapper scheduler or pass cache; Blaze may retain its shared daemon."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time
import tomllib

ROOT = Path(__file__).resolve().parents[1]
DEFAULTS = {
    "CARGO_BUILD_JOBS": "2",
    "NEXTEST_TEST_THREADS": "2",
    "RUST_TEST_THREADS": "2",
    "RAYON_NUM_THREADS": "2",
    "MBX_SCHEDULER_TESTS": "1",
    "MBX_SCHEDULER_PRIORITY": "low",
    "PYTHONDONTWRITEBYTECODE": "1",
}


def environment():
    env = os.environ.copy()
    for name, value in DEFAULTS.items():
        env.setdefault(name, value)
    for name in ("CARGO_BUILD_JOBS", "NEXTEST_TEST_THREADS", "RUST_TEST_THREADS", "RAYON_NUM_THREADS"):
        if not env[name].isdigit() or int(env[name]) < 1:
            raise ValueError(f"{name} must be a positive integer for verification")
    return env


def plan(tier, scope, root=ROOT):
    if tier in ("fast", "full"):
        if scope:
            raise ValueError(f"{tier} does not accept a scope")
        tasks = (["verify-tooling", "format_check", "toml_check", "clippy_workspace", "test"] if tier == "fast"
                 else ["verify-tooling", "lint", "check", "test", "ci_validate", "build_release"])
        return [["@cargo", "clippy", "--locked", "--workspace", "--all-targets", "--", "-D", "warnings"]
                if task == "clippy_workspace" else ["makers", task] for task in tasks]
    if tier == "package" or tier.startswith("iterate-"):
        manifests = [root / "Cargo.toml", root / "xtask/Cargo.toml", *sorted((root / "crates").glob("*/Cargo.toml"))]
        packages = {tomllib.loads(p.read_text()).get("package", {}).get("name") for p in manifests}
        if scope not in packages or not scope:
            raise ValueError("package requires an existing workspace package name")
        if tier.startswith("iterate-"):
            return [["blaze", tier.removeprefix("iterate-"), "--locked", "-p", scope]]
        return [
            ["@cargo", "clippy", "--locked", "-p", scope, "--all-targets", "--", "-D", "warnings"],
            ["@cargo", "nextest", "run", "--locked", "-p", scope, "--all-targets"],
            ["@cargo", "test", "--locked", "-p", scope, "--doc"],
        ]
    if not scope or not re.fullmatch(r"[a-z][a-z0-9]*(?:-[a-z0-9]+)*", scope):
        raise ValueError("rule requires a canonical rule id")
    if not (root / "crates/falcon_rules/tests/corpus" / scope).is_dir():
        raise ValueError(f"no corpus directory for {scope}")
    return [["@cargo", "xtask", "validate-rules", "--rule", scope]]


def iteration_environment(env, root=ROOT):
    env = env.copy()
    # Keep Blaze output out of Cargo/mbx's final-verification artifacts.
    env["CARGO_TARGET_DIR"] = str(root / "target-blaze")
    env.setdefault("BLAZE_TESTS", "affected")
    return env


def require_blaze_version(env, root=ROOT):
    version = tomllib.loads((root / "mise.toml").read_text())["tools"]["blaze"]
    actual = subprocess.check_output(["blaze", "--version"], cwd=root, env=env, text=True, timeout=10)
    if actual.split()[:2] != ["blaze", version]:
        raise ValueError(f"iteration requires Blaze {version}; install the pinned tool with mise install blaze")


def fingerprint(root):
    """Include tracked and nonignored new files, even staged/deleted content."""
    output = subprocess.check_output(
        ["git", "-c", "core.fsmonitor=false", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=root)
    digest = hashlib.sha256()
    for name in sorted(set(output.split(b"\0")) - {b""}):
        path = root / os.fsdecode(name)
        if path.is_symlink():
            # Do not certify external source trees whose contents we cannot enumerate.
            if not path.is_file():
                raise ValueError(f"unsupported source symlink: {path}")
            content = b"link\0" + os.fsencode(os.readlink(path)) + b"\0" + path.read_bytes()
        elif path.is_file():
            content = str(path.stat().st_mode).encode() + b"\0" + path.read_bytes()
        elif not path.exists():
            content = b"deleted"
        else:
            raise ValueError(f"unsupported source entry: {path}")
        for item in (name, content):
            digest.update(len(item).to_bytes(8, "big"))
            digest.update(item)
    return digest.hexdigest()


def stop(child):
    if os.name != "posix":
        if child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
        return
    try:
        os.killpg(child.pid, signal.SIGTERM)
    except ProcessLookupError:
        child.wait()
        return
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline:
        child.poll()
        try:
            os.killpg(child.pid, 0)
        except ProcessLookupError:
            child.wait()
            return
        time.sleep(0.02)
    try:
        os.killpg(child.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    child.wait()


def execute(command, root, env, timeout):
    child = subprocess.Popen(command, cwd=root, env=env, start_new_session=os.name == "posix")
    try:
        return child.wait(timeout=timeout)
    finally:
        # Even a completed parent can leave descendants. Only our group is owned.
        stop(child)


def run(commands, root, env, timeout):
    before = fingerprint(root)
    for command in commands:
        if fingerprint(root) != before:
            raise ValueError("source changed before a check; rerun on settled source")
        print("verify:", json.dumps(command), flush=True)
        code = execute(command, root, env, timeout)
        if code:
            return code if code > 0 else 128 - code
        if fingerprint(root) != before:
            raise ValueError("source changed during verification; this run is not a pass")
    print(f"verify: passed this scope on source sha256:{before}; wrapper caches no result", flush=True)
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tier", choices=("rule", "package", "fast", "full", "iterate-check", "iterate-build", "iterate-test"))
    parser.add_argument("scope", nargs="?")
    parser.add_argument("--dry-run", action="store_true", help="show commands and budgets without running checks")
    parser.add_argument("--timeout", type=int, default=1800, help="maximum seconds per command (default 1800)")
    args = parser.parse_args()
    try:
        if args.timeout < 1:
            raise ValueError("timeout must be positive")
        env = environment()
        commands = plan(args.tier, args.scope)
        iteration = args.tier.startswith("iterate-")
        if iteration:
            env = iteration_environment(env)
            print("Blaze iteration only: cached eligible tests may be reused; this is not final Rust verification.", flush=True)
            print("Blaze target:", env["CARGO_TARGET_DIR"], flush=True)
        for command in commands:
            if command[0] == "@cargo":
                command[0] = env.get("CARGO_CMD", "mbx")
        print("verify budgets:", json.dumps({name: env[name] for name in DEFAULTS}), flush=True)
        if args.dry_run:
            print(json.dumps(commands, indent=2))
            return 0
        if iteration:
            require_blaze_version(env)
        return run(commands, ROOT, env, args.timeout)
    except (ValueError, OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        print(f"verify: failed: {error}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("verify: cancelled; no passing evidence", file=sys.stderr)
        return 130


def interrupted(*_):
    # Repeated terminal signals must not interrupt bounded descendant cleanup.
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    signal.signal(signal.SIGINT, signal.SIG_IGN)
    if hasattr(signal, "SIGHUP"):
        signal.signal(signal.SIGHUP, signal.SIG_IGN)
    raise KeyboardInterrupt


if __name__ == "__main__":
    # SIGTERM must clean up our child group just like Ctrl-C.
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    if hasattr(signal, "SIGHUP"):
        signal.signal(signal.SIGHUP, interrupted)
    sys.exit(main())
