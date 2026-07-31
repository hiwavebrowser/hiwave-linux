#!/usr/bin/env python3
"""Classify unreachable CSS properties by whether wiring an arm would DO anything.

The reachability metric (collect_metrics.py) answers "can CSS set this field".
It cannot answer "would setting it change anything", because a field read by a
DEAD function still counts as read. This tool answers the second question.

Three shapes the fleet has now hit, in increasing sneakiness:

  1. producer with no consumer      - the classic dead wire
  2. consumer with no producer      - flex before hiwave-linux #30
  3. consumer implemented and ORPHANED
     3a. helper with zero production callers        <- a one-level check finds this
     3b. helper whose callers are THEMSELVES orphaned  <- one level says "healthy"

3b is why this file exists. On hiwave-linux, `overflow_x` is read by
`establishes_bfc`, which has three non-test callers - so a one-level check
reports overflow as WIREABLE. All three of those callers have zero production
callers of their own. Wiring the arm would drop two names off the reachability
list and change nothing on screen: the exact metric-gaming the pin forbids.

So: reachability to a LIVE ENTRY POINT, transitively. Not "has a caller".

Wireability asymmetry (Prometheus, companion rule): a field is WIREABLE if ANY
reader is transitively live; it is ORPHANED only if ALL readers are dead.
"""
from __future__ import annotations

import glob
import re
import sys
from pathlib import Path

LAYOUT = "crates/rustkit-layout/src"
ENGINE = "crates/rustkit-engine/src/lib.rs"


def strip_tests(src: str) -> str:
    """Drop everything from the first `#[cfg(test)]` marker onward.

    Crude but deliberately conservative in the RIGHT direction: it can only
    discard code, so it may under-report liveness (a false ORPHANED, which
    gets caught the moment someone writes the Group B test) and can never
    invent liveness (a false WIREABLE, which is the expensive mistake).
    """
    i = src.find("#[cfg(test)]")
    return src[:i] if i > 0 else src


def load() -> tuple[dict[str, str], str]:
    layout = {p: strip_tests(Path(p).read_text()) for p in sorted(glob.glob(f"{LAYOUT}/*.rs"))}
    engine = strip_tests(Path(ENGINE).read_text())
    return layout, engine


def functions(src: str) -> list[tuple[str, int]]:
    return [(m.group(1), m.start()) for m in re.finditer(r"\bfn\s+([a-z_0-9]+)\s*[(<]", src)]


def enclosing_fn(src: str, pos: int) -> str | None:
    best = None
    for name, start in functions(src):
        if start <= pos:
            best = name
        else:
            break
    return best


def call_graph(layout: dict[str, str], engine: str) -> tuple[dict[str, set[str]], set[str]]:
    """caller -> callees, plus the seed set of functions the ENGINE calls."""
    all_fns = set()
    for src in list(layout.values()) + [engine]:
        all_fns |= {n for n, _ in functions(src)}

    graph: dict[str, set[str]] = {f: set() for f in all_fns}
    for src in list(layout.values()) + [engine]:
        for m in re.finditer(r"\b([a-z_0-9]+)\s*\(", src):
            callee = m.group(1)
            if callee not in all_fns:
                continue
            # skip the definition site itself
            if src[max(0, m.start() - 3):m.start()].endswith("fn "):
                continue
            caller = enclosing_fn(src, m.start())
            if caller and caller != callee:
                graph.setdefault(caller, set()).add(callee)

    # SEEDS: anything the engine crate calls is live by definition - the engine
    # is the production entry into layout. Plus `layout`, the recursive spine
    # every box goes through.
    seeds = set()
    for m in re.finditer(r"\b([a-z_0-9]+)\s*\(", engine):
        if m.group(1) in all_fns and not engine[max(0, m.start() - 3):m.start()].endswith("fn "):
            seeds.add(m.group(1))
    seeds |= {"layout", "build"}
    return graph, seeds & all_fns


def live_set(graph: dict[str, set[str]], seeds: set[str]) -> set[str]:
    live, stack = set(seeds), list(seeds)
    while stack:
        for callee in graph.get(stack.pop(), ()):
            if callee not in live:
                live.add(callee)
                stack.append(callee)
    return live


def classify(fields: list[str]) -> dict[str, dict]:
    layout, engine = load()
    graph, seeds = call_graph(layout, engine)
    live = live_set(graph, seeds)

    out = {}
    for f in fields:
        readers = set()
        for src in layout.values():
            for m in re.finditer(r"\." + re.escape(f) + r"\b", src):
                fn = enclosing_fn(src, m.start())
                if fn:
                    readers.add(fn)
        live_readers = sorted(r for r in readers if r in live)
        out[f] = {
            "readers": sorted(readers),
            "live_readers": live_readers,
            # ANY live reader is enough. Requiring all would report `position`
            # as orphaned because set_z_index and with_position are dead.
            "verdict": "WIREABLE" if live_readers else "ORPHANED",
        }
    return out


def classify_one_level(fields: list[str]) -> dict[str, str]:
    """The NAIVE check, kept deliberately, as this file's own falsifier.

    "Does the reading function have a caller?" - the obvious reading of the
    original pin. It is wrong, and the only convincing way to say so is to run
    it beside the correct one and show it disagreeing. A tool whose extra
    complexity is never demonstrated to matter is complexity nobody can audit.
    """
    layout, engine = load()
    out = {}
    for f in fields:
        readers = set()
        for src in layout.values():
            for m in re.finditer(r"\." + re.escape(f) + r"\b", src):
                fn = enclosing_fn(src, m.start())
                if fn:
                    readers.add(fn)
        has_caller = False
        for r in readers:
            for src in list(layout.values()) + [engine]:
                for m in re.finditer(r"\b" + re.escape(r) + r"\s*\(", src):
                    if src[max(0, m.start() - 3):m.start()].endswith("fn "):
                        continue
                    has_caller = True
        out[f] = "WIREABLE" if has_caller else "ORPHANED"
    return out


def main() -> int:
    sys.path.insert(0, str(Path(__file__).parent))
    import collect_metrics  # noqa: E402

    fields = collect_metrics.reachability()["unreachable_fields"]
    result = classify(fields)
    wireable = [f for f, v in result.items() if v["verdict"] == "WIREABLE"]
    orphaned = [f for f, v in result.items() if v["verdict"] == "ORPHANED"]

    print(f"{'field':26} verdict     live reader")
    print("-" * 72)
    for f in sorted(result):
        v = result[f]
        print(f"{f:26} {v['verdict']:11} {(v['live_readers'] or ['(none - all readers dead)'])[0]}")
    print()
    print(f"WIREABLE ({len(wireable)}): an arm here would actually change the page.")
    print(f"ORPHANED ({len(orphaned)}): an arm here drops a name off the metric "
          f"and changes NOTHING. Wire the caller first.")

    # FALSIFIER: show the naive check disagreeing, or admit it does not.
    naive = classify_one_level(fields)
    disagree = sorted(f for f in fields if naive[f] != result[f]["verdict"])
    print()
    if disagree:
        print("ONE-LEVEL CHECK DISAGREES ON: " + ", ".join(disagree))
        for f in disagree:
            print(f"  {f}: one-level says {naive[f]}, transitive says "
                  f"{result[f]['verdict']} - shape 3b (readers exist, their "
                  f"callers are orphaned too)")
        print("This is why the transitive walk exists. A one-level script "
              "would green-light these arms.")
    else:
        print("One-level and transitive agree on every field TODAY. That does "
              "NOT retire the transitive check - it means no shape-3b case is "
              "currently on the list. Re-read this line before simplifying.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
