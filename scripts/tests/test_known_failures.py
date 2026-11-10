"""collect_metrics.py: failing-test names and the Linux known-failure split (the job is red only on an
UNEXPECTED failure; a crash that names no failing test is not a pass)."""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import collect_metrics as cm  # noqa: E402

OUT = """\
     Running unittests src/lib.rs (target/debug/deps/rustkit_layout-0123abcd)
test tests::a_fine_test ... ok
test tests::a_known_one ... FAILED
test tests::a_new_one ... FAILED
test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.1s
   Doc-tests hiwave_core
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.0s
"""


def _known(tmp_path, *names):
    p = tmp_path / "known.json"
    p.write_text(json.dumps({"known": [{"test": n, "reason": "r"} for n in names]}))
    return p


def test_failing_tests_are_named_by_the_binary_that_ran_them():
    per_crate, totals, failed = cm.parse_tests(OUT)
    assert failed == ["rustkit_layout::tests::a_known_one", "rustkit_layout::tests::a_new_one"]
    assert totals["failed"] == 2 and per_crate["rustkit_layout"]["failed"] == 2


def test_only_an_unlisted_failure_is_unexpected(tmp_path):
    _, _, failed = cm.parse_tests(OUT)
    r = cm.classify_failures(failed, _known(tmp_path, "rustkit_layout::tests::a_known_one"))
    assert r["failed_known"] == ["rustkit_layout::tests::a_known_one"]
    assert r["failed_unexpected"] == ["rustkit_layout::tests::a_new_one"]
    assert r["known_now_passing"] == []


def test_a_listed_test_that_passes_is_reported_not_red(tmp_path):
    r = cm.classify_failures([], _known(tmp_path, "rustkit_layout::tests::a_known_one"))
    assert r["failed_unexpected"] == [] and r["known_now_passing"] == ["rustkit_layout::tests::a_known_one"]


def test_no_known_file_means_every_failure_is_unexpected(tmp_path):
    r = cm.classify_failures(["x::y"], tmp_path / "absent.json")
    assert r["failed_unexpected"] == ["x::y"]


def test_the_repo_list_is_well_formed():
    data = json.loads(cm.KNOWN_FAILURES.read_text())
    assert data["known"] and all(e["test"].count("::") >= 1 and e["reason"] for e in data["known"])
