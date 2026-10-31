"""Tooling contracts using tiny subprocesses, never Cargo or shared caches."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from concurrent.futures import ThreadPoolExecutor
from unittest.mock import patch

import verify


class VerificationTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="falcon-verify-test-")
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.root = self.base / "source"
        self.root.mkdir()
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        (self.root / "input").write_text("one")

    def command(self, code):
        return [sys.executable, "-c", code]

    def test_final_plan_retains_full_coverage_and_shared_validation(self):
        tasks = [cmd[1] for cmd in verify.plan("full", None)]
        self.assertEqual(tasks, ["verify-tooling", "lint", "check", "test", "ci_validate", "build_release"])
        self.assertNotIn("build_release", [cmd[1] for cmd in verify.plan("fast", None)])
        self.assertEqual(verify.plan("package", "falcon_config")[1][1:5],
                         ["nextest", "run", "--locked", "-p"])
        for tier, scope in [("package", "--workspace"), ("rule", "../escape"),
                            ("rule", "not-a-real-rule"), ("full", "unexpected")]:
            with self.assertRaises(ValueError):
                verify.plan(tier, scope)

    def test_iteration_is_explicit_pinned_and_isolates_compiler_outputs(self):
        self.assertEqual(verify.plan("iterate-test", "falcon_syntax"),
                         [["blaze", "test", "--locked", "-p", "falcon_syntax"]])
        original = {"CARGO_TARGET_DIR": "/shared/cargo"}
        env = verify.iteration_environment(original, self.root)
        self.assertEqual(original["CARGO_TARGET_DIR"], "/shared/cargo")
        self.assertEqual(env["CARGO_TARGET_DIR"], str(self.root / "target-blaze"))
        self.assertEqual(env["BLAZE_TESTS"], "affected")
        self.assertEqual(verify.iteration_environment({"BLAZE_TESTS": "all"})["BLAZE_TESTS"], "all")
        (self.root / "mise.toml").write_text('[tools]\nblaze = "0.20261029.1"\n')
        with patch("verify.subprocess.check_output", return_value="blaze 0.20261029.1 (revision)"):
            verify.require_blaze_version(env, self.root)
        with patch("verify.subprocess.check_output", return_value="blaze 0.0.0"):
            with self.assertRaises(ValueError):
                verify.require_blaze_version(env, self.root)

    def test_budgets_reach_children_without_changing_parent(self):
        output = self.base / "env.json"
        with patch.dict(os.environ, {}, clear=True):
            env = verify.environment()
            verify.execute(self.command(f"import os,json; open({str(output)!r},'w').write(json.dumps(dict(os.environ)))"),
                           self.root, env, 5)
            self.assertNotIn("RAYON_NUM_THREADS", os.environ)
        actual = json.loads(output.read_text())
        for key, value in verify.DEFAULTS.items():
            self.assertEqual(actual[key], value)
        with patch.dict(os.environ, {"RAYON_NUM_THREADS": "3"}):
            self.assertEqual(verify.environment()["RAYON_NUM_THREADS"], "3")
        with patch.dict(os.environ, {"CARGO_BUILD_JOBS": "0"}):
            with self.assertRaises(ValueError):
                verify.environment()

    def test_fingerprint_tracks_new_deleted_staged_modes_and_symlinks(self):
        first = verify.fingerprint(self.root)
        (self.root / "input").write_text("two")
        self.assertNotEqual(first, verify.fingerprint(self.root))
        (self.root / "input").write_text("one")
        subprocess.run(["git", "add", "input"], cwd=self.root, check=True)
        (self.root / "input").unlink()
        self.assertNotEqual(first, verify.fingerprint(self.root))
        (self.root / "input").write_text("one")
        (self.root / "new").write_text("new")
        second = verify.fingerprint(self.root)
        self.assertNotEqual(first, second)
        (self.root / "new").chmod(0o755)
        self.assertNotEqual(second, verify.fingerprint(self.root))
        if os.name == "posix":
            (self.root / "link").symlink_to("input")
            third = verify.fingerprint(self.root)
            (self.root / "link").unlink()
            (self.root / "link").symlink_to("new")
            self.assertNotEqual(third, verify.fingerprint(self.root))

    def test_failure_stops_plan_and_prior_success_never_skips_execution(self):
        marker = self.base / "runs"
        cmd = self.command(f"open({str(marker)!r},'a').write('run\\n')")
        for _ in range(2):
            self.assertEqual(verify.run([cmd], self.root, verify.environment(), 5), 0)
        self.assertEqual(marker.read_text().splitlines(), ["run", "run"])
        self.assertEqual(verify.run([self.command("raise SystemExit(7)"), cmd],
                                    self.root, verify.environment(), 5), 7)
        self.assertEqual(len(marker.read_text().splitlines()), 2)

    def test_mid_run_source_change_cannot_pass(self):
        with self.assertRaisesRegex(ValueError, "source changed"):
            verify.run([self.command("open('input','w').write('changed')")],
                       self.root, verify.environment(), 5)

    def test_independent_checks_overlap_without_a_global_lock(self):
        # Each child waits until BOTH started: serialization cannot satisfy this.
        def work(index):
            mine = self.base / str(index)
            other = self.base / str(1 - index)
            code = (f"from pathlib import Path; import time; Path({str(mine)!r}).touch(); "
                    f"deadline=time.monotonic()+4\nwhile not Path({str(other)!r}).exists():\n"
                    " if time.monotonic()>deadline: raise SystemExit(9)\n time.sleep(.02)\n")
            return verify.run([self.command(code)], self.root, verify.environment(), 5)
        with ThreadPoolExecutor(max_workers=2) as pool:
            self.assertEqual(list(pool.map(work, [0, 1])), [0, 0])

    @unittest.skipUnless(sys.platform == "linux", "Linux /proc liveness assertion")
    def test_timeout_terminates_owned_descendant_that_ignores_term(self):
        pidfile = self.base / "pid"
        descendant = "import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); time.sleep(30)"
        code = (f"import subprocess,sys,time; p=subprocess.Popen([sys.executable,'-c',{descendant!r}]); "
                f"open({str(pidfile)!r},'w').write(str(p.pid)); time.sleep(30)")
        with self.assertRaises(subprocess.TimeoutExpired):
            verify.execute(self.command(code), self.root, verify.environment(), 0.5)
        pid = int(pidfile.read_text())
        status = Path(f"/proc/{pid}/stat")
        deadline = time.monotonic() + 2
        while status.exists() and status.read_text().split()[2] != "Z":
            if time.monotonic() > deadline:
                self.fail("descendant still running after cleanup")
            time.sleep(.02)

    @unittest.skipUnless(sys.platform == "linux", "Linux /proc liveness assertion")
    def test_cancellation_exits_nonzero_and_cleans_child(self):
        for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            with self.subTest(signal=sig):
                marker = self.base / f"started-{sig}"
                child_code = f"import os,time; open({str(marker)!r},'w').write(str(os.getpid())); time.sleep(30)"
                code = (f"import sys,signal; sys.path.insert(0,{str(Path(verify.__file__).parent)!r}); import verify; "
                        "signal.signal(signal.SIGTERM,verify.interrupted); "
                        "signal.signal(signal.SIGINT,verify.interrupted); "
                        "signal.signal(signal.SIGHUP,verify.interrupted); "
                        f"verify.execute({self.command(child_code)!r},{str(self.root)!r},verify.environment(),30)")
                parent = subprocess.Popen(self.command(code), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                try:
                    deadline = time.monotonic() + 5
                    while not marker.exists() or not marker.read_text():
                        if time.monotonic() > deadline:
                            self.fail("child never started")
                        time.sleep(.02)
                    parent.send_signal(sig)
                    self.assertNotEqual(parent.wait(timeout=5), 0)
                    status = Path(f"/proc/{int(marker.read_text())}/stat")
                    if status.exists():
                        self.assertEqual(status.read_text().split()[2], "Z", "child still running")
                finally:
                    if parent.poll() is None:
                        parent.kill()
                        parent.wait()


if __name__ == "__main__":
    unittest.main()
