"""collect_metrics.run(): the whole output is still captured for parsing, but progress lines are echoed live with
the elapsed time, and the phase length is recorded (the 60-minute hosted cancel of 2026-11-10 left an empty log)."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import collect_metrics as cm  # noqa: E402

SCRIPT = (
    "import sys\n"
    "print('   Compiling rustkit-engine v0.1.0')\n"
    "print('noise that is not progress')\n"
    "sys.stderr.write('     Running unittests src/lib.rs (target/debug/deps/rustkit_layout-0123abcd)\\n')\n"
    "print('test tests::a ... FAILED')\n"
    "print('test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.1s')\n"
    "sys.exit(101)\n"
)


def test_progress_is_echoed_and_everything_is_still_captured(capsys):
    code, out = cm.run([sys.executable, "-c", SCRIPT], "test")
    err = capsys.readouterr().err
    assert code == 101
    assert "noise that is not progress" in out, "the full output is still captured for parsing"
    assert "[test +" in err and "Compiling rustkit-engine" in err and "Running unittests" in err
    assert "... FAILED" in err and "test result: FAILED" in err
    assert "noise that is not progress" not in err, "only progress lines are echoed"


def test_the_captured_output_still_parses_into_per_binary_failures(capsys):
    _, out = cm.run([sys.executable, "-c", SCRIPT], "test")
    _, totals, failed = cm.parse_tests(out)
    assert failed == ["rustkit_layout::tests::a"] and totals["failed"] == 1


def test_the_phase_length_is_recorded():
    cm.run([sys.executable, "-c", "pass"], "x")
    assert cm.run.last_seconds >= 0
