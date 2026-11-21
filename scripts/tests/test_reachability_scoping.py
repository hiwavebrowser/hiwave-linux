"""The reachability ratchet's scoping (scripts/collect_metrics.py): which function is the applier, and what
counts as the applier writing a field. Each case is a way the metric was wrong on Linux (2026-11-09: red on
every PR because the first `fn apply_style_property_impl` is a 3-line forwarder)."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import collect_metrics as cm  # noqa: E402

REAL = ("fn apply_style_property_impl(style: &mut S) { match p { \"width\" => { style.width = 1; } "
        "\"mask\" => { style.mask.apply(2); } \"stacks\" => { style.stacks &= 3; } _ => {} } }")
FORWARDER = "fn apply_style_property_impl(style: &mut S) { Other::apply_style_property_impl(style) }"


def test_the_applier_is_the_largest_definition_not_the_first():
    body, fell_back = cm._applier_body(FORWARDER + "\n" + REAL)
    assert not fell_back
    assert "style.width = 1" in body, "scoped to the forwarder: every field would read unreachable"
    body_rev, _ = cm._applier_body(REAL + "\n" + FORWARDER)
    assert "style.width = 1" in body_rev, "order must not matter"


def test_a_missing_applier_still_falls_back_loudly(capsys):
    body, fell_back = cm._applier_body("fn something_else() {}")
    assert fell_back and "WARNING" in capsys.readouterr().err


def test_helpers_called_with_the_style_are_part_of_the_applier():
    applier = "fn apply_style_property_impl(style: &mut S) { animations::set_names(style, v); log(x); }"
    sources = {
        "animations.rs": "fn set_names(style: &mut S, v: &str) { style.animation_name = v; }",
        "other.rs": "fn log(x: u8) { style.not_a_css_field = x; }",
    }
    helpers = cm._helper_bodies(applier, sources)
    assert "style.animation_name" in helpers
    assert "not_a_css_field" not in helpers, "a function never handed the style cannot write it"


def test_engine_state_is_a_short_named_table_not_an_open_door():
    assert set(cm.ENGINE_STATE) == {"abspos_was_inline_level", "animation_settled_opacity", "webkit_scrollbar"}
    assert all(reason for reason in cm.ENGINE_STATE.values())
