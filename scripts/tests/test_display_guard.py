"""
test_display_guard.py: guards for scripts/display_guard.py and the runs that call it.

1. A desktop session's display (0 or 1, any host or screen form) and any WAYLAND_DISPLAY are
   refused; no DISPLAY, an empty one or a private Xvfb's (:99) is allowed; the override lets a
   person through.
2. The refusal reaches the runs: collect_metrics.py exits 1 before collecting, and chrome_board.py
   returns 2 before capturing, both naming the reason; with a private display chrome_board.py goes
   past the guard (it refuses later, on the missing binary).
"""

import importlib.util
import io
import os
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from unittest import mock

REPO = Path(__file__).resolve().parent.parent.parent
SPEC = importlib.util.spec_from_file_location("display_guard", REPO / "scripts" / "display_guard.py")
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)


class Reason(unittest.TestCase):
    def test_a_desktop_display_is_refused(self):
        for display in (":0", ":1", ":0.0", ":1.1", "unix:0", "localhost:1", "localhost:0.0"):
            self.assertIn("desktop session", guard.live_display_reason({"DISPLAY": display}) or "", display)

    def test_wayland_is_refused_whatever_the_display(self):
        self.assertIn("WAYLAND_DISPLAY", guard.live_display_reason({"WAYLAND_DISPLAY": "wayland-0"}) or "")
        self.assertIn("WAYLAND_DISPLAY",
                      guard.live_display_reason({"WAYLAND_DISPLAY": "wayland-0", "DISPLAY": ":99"}) or "")

    def test_no_display_or_a_private_one_is_allowed(self):
        for env in ({}, {"DISPLAY": ""}, {"DISPLAY": ":99"}, {"DISPLAY": ":99.0"}, {"DISPLAY": ":10"},
                    {"WAYLAND_DISPLAY": ""}):
            self.assertIsNone(guard.live_display_reason(env), env)

    def test_an_unreadable_display_is_refused(self):
        self.assertIsNotNone(guard.live_display_reason({"DISPLAY": "garbage"}))

    def test_the_override_lets_a_person_through(self):
        env = {"DISPLAY": ":0", "WAYLAND_DISPLAY": "wayland-0", "HIWAVE_ALLOW_LIVE_DISPLAY": "1"}
        self.assertIsNone(guard.live_display_reason(env))
        self.assertIsNotNone(guard.live_display_reason({**env, "HIWAVE_ALLOW_LIVE_DISPLAY": "yes"}))


def scrubbed(**extra):
    env = {k: v for k, v in os.environ.items()
           if k not in ("DISPLAY", "WAYLAND_DISPLAY", "HIWAVE_ALLOW_LIVE_DISPLAY")}
    env.update(extra)
    return env


class TheRunsCallIt(unittest.TestCase):
    def test_the_cli_exits_1_with_the_reason(self):
        r = subprocess.run([sys.executable, str(REPO / "scripts" / "display_guard.py")],
                           env=scrubbed(DISPLAY=":0"), capture_output=True, text=True)
        self.assertEqual(r.returncode, 1)
        self.assertIn("DISPLAY=:0", r.stderr)
        ok = subprocess.run([sys.executable, str(REPO / "scripts" / "display_guard.py")],
                            env=scrubbed(DISPLAY=":99"), capture_output=True, text=True)
        self.assertEqual(ok.returncode, 0, ok.stderr)

    def test_collect_metrics_refuses_before_collecting(self):
        # `cargo` on PATH is a stand-in that leaves a mark and fails at once: were the guard gone,
        # collection would run it (and the real one would build the workspace), not hang here.
        with tempfile.TemporaryDirectory() as tmp:
            mark = Path(tmp) / "cargo-ran"
            stub = Path(tmp) / "cargo"
            stub.write_text(f"#!/bin/sh\ntouch '{mark}'\nexit 97\n")
            stub.chmod(0o755)
            env = scrubbed(DISPLAY=":0", PATH=f"{tmp}{os.pathsep}{os.environ.get('PATH', '')}")
            r = subprocess.run([sys.executable, str(REPO / "scripts" / "collect_metrics.py"), "--output",
                                os.devnull], env=env, capture_output=True, text=True, timeout=120, cwd=REPO)
            self.assertFalse(mark.exists(), "cargo ran: collection started on a desktop display")
        self.assertEqual(r.returncode, 1, r.stderr)
        self.assertIn("collect_metrics: refused: DISPLAY=:0", r.stderr)

    def _board(self, env):
        spec = importlib.util.spec_from_file_location("chrome_board_dg", REPO / "scripts" / "chrome_board.py")
        board = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(board)
        err = io.StringIO()
        with mock.patch.dict(os.environ, env, clear=True), redirect_stderr(err):
            code = board.main(["--parity-capture", "/nonexistent/parity-capture", "--only", "no-such-page"])
        return code, err.getvalue()

    def test_chrome_board_refuses_before_capturing(self):
        code, err = self._board(scrubbed(WAYLAND_DISPLAY="wayland-0"))
        self.assertEqual(code, 2)
        self.assertIn("refused: WAYLAND_DISPLAY=wayland-0", err)

    def test_chrome_board_goes_past_the_guard_on_a_private_display(self):
        code, err = self._board(scrubbed(DISPLAY=":99"))
        self.assertNotIn("WAYLAND_DISPLAY", err)
        self.assertNotIn("desktop session", err)


if __name__ == "__main__":
    unittest.main()
