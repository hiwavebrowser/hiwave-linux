"""Scoping of the source-only CI checks (scripts/dead_emit_check.py, scripts/canonical_constructor_check.py).
Each case is a way they were wrong on the Linux tree and turned `collect-metrics` red on every PR (2026-11-09)."""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import canonical_constructor_check as cc  # noqa: E402
import dead_emit_check as de  # noqa: E402


def _is_pattern(src: str) -> bool:
    m = re.search(r"DisplayCommand::([A-Z][A-Za-z0-9]*)\s*[{(]", src)
    return de._is_pattern(src, m)


def test_a_match_arm_is_not_a_construction():
    assert _is_pattern("match c { DisplayCommand::FillRect { color: k, .. } => tint(k), _ => {} }")


def test_an_or_pattern_is_not_a_construction():
    assert _is_pattern("match c { DisplayCommand::SolidColor(k, _)\n | DisplayCommand::Line { color: k, .. } => 1 }")


def test_if_let_and_matches_are_not_constructions():
    assert _is_pattern("if let DisplayCommand::Line { color, .. } = cmd { f(color) }")
    assert _is_pattern("let n = cmds.iter().filter(|c| matches!(c, DisplayCommand::Line { .. })).count();")


def test_a_real_construction_still_counts():
    assert not _is_pattern("out.push(DisplayCommand::FillRect { rect, color });")
    assert not _is_pattern("let c = DisplayCommand::SolidColor(color, rect);")
    assert not _is_pattern("vec![DisplayCommand::Line { a, b, color }, other]")


def test_the_five_vector_commands_are_not_dead_emits_on_this_tree():
    analysis = de.analyse()
    assert not set(analysis["dead_only"]) & {"FillRect", "Line", "Polyline", "StrokePolygon", "StrokeRect"}


def test_baseline_matches_by_text_not_line_and_counts():
    found = [("a.rs", 10, "let s = X::default();"), ("a.rs", 99, "let s = X::default();")]
    assert cc.unbaselined(found, [("a.rs", 3, "let s = X::default();")]) == [("a.rs", 99, "let s = X::default();")], (
        "one baselined use covers one occurrence wherever it moved to, not a second one")
    assert cc.unbaselined(found[:1], [("a.rs", 3, "let s = X::default();")]) == []


def test_external_test_modules_are_test_code():
    tests = {p.relative_to(cc.CRATES.parent).as_posix() for p in cc.test_files()}
    assert "crates/rustkit-engine/src/border_medium_apply_tests.rs" in tests
    assert "crates/rustkit-engine/src/lib.rs" not in tests, "a production file must never be test code"
