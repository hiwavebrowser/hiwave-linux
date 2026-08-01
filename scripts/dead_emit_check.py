#!/usr/bin/env python3
"""Fail when a DisplayCommand variant is only ever constructed from dead code.

Prometheus's N3, and #41 is the case study. I added
`DisplayCommand::BoxShadow` and called the emitter from `render_box` - which
is marked `#[allow(dead_code)]` and labelled "legacy method". It compiled,
every existing test stayed green, and the reaching test stayed red with NO
ERROR ANYWHERE, because a dead function accepts calls perfectly well. I lost
a debugging round to it while holding wireability.py, which exists to detect
exactly that shape.

This is the mechanical version of that lesson. A paint command constructed
only inside dead code is a display-list variant that can never appear in a
display list - a dead wire that looks like a feature.

WHAT IT IS NOT. It does not prove a variant reaches the screen: the renderer
still has to handle it, and the emitter still has to be called with data that
passes its gates. It rules out one specific silent failure, which is worth
saying because a check that oversells itself is the thing this fleet keeps
deleting.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

LAYOUT = Path("crates/rustkit-layout/src")
BASELINE = Path(__file__).with_name("dead_emit_baseline.json")


def _fn_spans(src: str) -> list[tuple[str, int, int, bool]]:
    """(name, start, end, is_dead) for each fn, with its attributes inspected."""
    out = []
    for m in re.finditer(r"(?:^|\n)([ \t]*)(?:pub(?:\([^)]*\))?\s+)?fn\s+([a-z_0-9]+)", src):
        name = m.group(2)
        # attributes immediately above the fn
        head = src[max(0, m.start() - 400):m.start()]
        is_dead = "#[allow(dead_code)]" in head.split("fn ")[-1] or head.rstrip().endswith(
            "#[allow(dead_code)]"
        )
        brace = src.find("{", m.end())
        if brace < 0:
            continue
        depth, k = 0, brace
        while k < len(src):
            if src[k] == "{":
                depth += 1
            elif src[k] == "}":
                depth -= 1
                if depth == 0:
                    break
            k += 1
        out.append((name, brace, k, is_dead))
    return out


def analyse() -> dict:
    variants: dict[str, list[tuple[str, bool]]] = {}
    for path in sorted(LAYOUT.glob("*.rs")):
        src = path.read_text()
        # ignore test modules: a variant constructed only in tests is a
        # different (and much louder) problem than one constructed only in
        # dead production code.
        t = src.find("#[cfg(test)]")
        prod = src[:t] if t > 0 else src
        spans = _fn_spans(prod)
        for m in re.finditer(r"DisplayCommand::([A-Z][A-Za-z0-9]*)\s*[{(]", prod):
            v = m.group(1)
            owner = next(
                ((n, dead) for n, s, e, dead in spans if s <= m.start() <= e), ("<top-level>", False)
            )
            variants.setdefault(v, []).append(owner)

    # TRANSITIVE reachability, not "is the constructing function itself dead".
    #
    # My first version asked only whether the fn holding the construction
    # carried #[allow(dead_code)]. It did NOT fire when I reproduced the exact
    # #41 mistake, because BoxShadow is constructed inside render_box_shadows -
    # an ordinary function - which was merely CALLED from the dead render_box.
    # One level too shallow, in a checker written about depth, after learning
    # that same lesson twice (one-level callers, per-family reference check).
    #
    # So: a variant is dead-only if NONE of its constructing functions is
    # reachable from a live entry point.
    graph, live = _reachability()
    dead_only = {}
    for v, sites in variants.items():
        owners = {n for n, _ in sites}
        if owners and not (owners & live):
            dead_only[v] = sorted(owners)
    return {"variants_constructed": len(variants), "dead_only": dead_only, "live_fns": len(live)}


ENTRY_POINTS = {"build", "render_stacking_context", "layout"}


def _reachability() -> tuple[dict[str, set[str]], set[str]]:
    """Call graph over production layout code, and the set reachable from entries.

    Entry points are the display-list and layout roots the engine actually
    calls. A function only reachable via #[allow(dead_code)] paths never
    appears in `live`.
    """
    srcs = []
    for path in sorted(LAYOUT.glob("*.rs")):
        src = path.read_text()
        t = src.find("#[cfg(test)]")
        srcs.append(src[:t] if t > 0 else src)

    all_fns: set[str] = set()
    spans_by_src = []
    for src in srcs:
        spans = _fn_spans(src)
        spans_by_src.append((src, spans))
        all_fns |= {n for n, _, _, _ in spans}

    graph: dict[str, set[str]] = {f: set() for f in all_fns}
    for src, spans in spans_by_src:
        for m in re.finditer(r"\b([a-z_0-9]+)\s*\(", src):
            callee = m.group(1)
            if callee not in all_fns:
                continue
            if src[max(0, m.start() - 3):m.start()].endswith("fn "):
                continue
            owner = next((n for n, s, e, dead in spans if s <= m.start() <= e and not dead), None)
            # calls made FROM a dead function do not propagate liveness
            if owner and owner != callee:
                graph.setdefault(owner, set()).add(callee)

    live = {f for f in ENTRY_POINTS if f in all_fns}
    stack = list(live)
    while stack:
        for callee in graph.get(stack.pop(), ()):
            if callee not in live:
                live.add(callee)
                stack.append(callee)
    return graph, live


def main() -> int:
    import json

    r = analyse()
    # Baseline, same asymmetry as the reachability ratchet: a variant may only
    # LEAVE this file. The three entries here are pre-existing and genuine -
    # forms.rs and images.rs are re-exported but never called, so Border,
    # Image and BackgroundImage cannot appear in a display list on this tree.
    # Failing CI on them today would just teach everyone to ignore the check.
    known = set()
    if BASELINE.exists():
        known = set(json.loads(BASELINE.read_text())["dead_only_variants"])
    fresh = {v: fns for v, fns in r["dead_only"].items() if v not in known}
    fixed = sorted(known - set(r["dead_only"]))
    print(f"DisplayCommand variants constructed in production code: {r['variants_constructed']}")
    if fixed:
        print()
        print(f"NOW REACHABLE ({len(fixed)}): {', '.join(fixed)} - remove from "
              f"scripts/dead_emit_baseline.json.")
    if fresh:
        print()
        print(f"NEWLY CONSTRUCTED ONLY FROM DEAD CODE ({len(fresh)}):")
        for v, fns in sorted(fresh.items()):
            print(f"   DisplayCommand::{v}  <- constructed only in {', '.join(fns)}, "
                  f"none reachable from a live entry point")
        print()
        print("A paint command emitted only from dead code can never appear in a "
              "display list. Either call the emitter from a live path, or delete "
              "the variant.")
        return 1
    print(f"SUMMARY: {len(r['dead_only'])} variant(s) constructed only from dead "
          f"code, all baselined; 0 new.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
