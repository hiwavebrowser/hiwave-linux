#!/usr/bin/env python3
"""Collect build/test metrics for hiwave-linux.

hiwave-macos collects parity metrics by running the pixel-capture harness
(scripts/parity_test.py) and recording an average pixel diff. That metric is
NOT collectable on a GitHub-hosted Linux runner: parity capture needs a real
GPU adapter, and when capture yields nothing the macOS pipeline defaults the
per-case diff to 100.0 rather than erroring -- so a GPU-less runner would
publish a confident-looking "100% diff" that is an artefact of the harness,
not a measurement of the renderer.

So this collects what IS true headless: does the workspace build, and do the
tests pass. Those are real numbers a Linux runner can stand behind.

Usage:
    python scripts/collect_metrics.py --output metrics.json
    python scripts/collect_metrics.py --input metrics.json --format markdown
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

# "test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
RESULT_RE = re.compile(
    r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored"
)
# "     Running unittests src\lib.rs (target\debug\deps\rustkit_text-abc123.exe)"
RUNNING_RE = re.compile(r"Running .*?\(.*?[\\/]deps[\\/]([A-Za-z0-9_]+?)-[0-9a-f]+")
DOCTEST_RE = re.compile(r"^\s*Doc-tests (\S+)")


def run(cmd: list[str]) -> tuple[int, str]:
    # stderr is merged into stdout by the PIPE, not concatenated afterwards.
    # cargo prints "Running <binary>" on stderr and "test result:" on stdout,
    # so appending one to the other loses the interleaving that attributes a
    # result block to the binary that produced it -- every crate then lands
    # in a single bucket.
    proc = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                          text=True, encoding="utf-8", errors="replace")
    return proc.returncode, proc.stdout or ""


def reachability() -> dict:
    """Count ComputedStyle fields that rustkit-layout READS but no CSS can SET.

    A field in this set means the layout code consuming it is unreachable: the
    property is implemented and cannot be triggered. Every page renders as if
    the author never wrote it, and nothing logs anything.

    This is not visible to any other metric we collect. Test coverage cannot
    see it - the positioned-layout tests PASS, because they build LayoutBoxes
    directly and set the fields by hand. Coverage measures whether code runs,
    not whether a USER can cause it to run. LOC ratio cannot see it either: the
    missing producer is a one-line match arm while the implemented consumer is
    hundreds of lines, so a tree can close LOC and still be unable to set a
    property.

    Method is Athena's (hiwave-windows, 2026-07-31); the reference check that
    made it actionable is that macOS CAN set these, so they are port defects
    rather than a shared limit.

    Deliberately reported as a LIST, not just a count. A count that ticks down
    tells you progress; the list tells you which capability is dead, and that
    is the part someone can act on.
    """
    css = Path("crates/rustkit-css/src/lib.rs").read_text()
    i = css.find("pub struct ComputedStyle")
    if i < 0:
        return {"error": "ComputedStyle not found"}
    fields = sorted(set(re.findall(r"pub ([a-z_0-9]+):", css[i:css.find("}", css.find("{", i))])))

    layout = "".join(p.read_text() for p in sorted(Path("crates/rustkit-layout/src").glob("*.rs")))
    engine = Path("crates/rustkit-engine/src/lib.rs").read_text()

    read = [f for f in fields if re.search(r"\." + f + r"\b", layout)]
    written = [f for f in fields
               if re.search(r"\bstyle\." + f + r"\s*=", engine)
               or re.search(r"\bs\." + f + r"\s*=", engine)]
    unreachable = [f for f in read if f not in written]

    return {
        "computed_style_fields": len(fields),
        "read_by_layout": len(read),
        "writable_by_applier": len(written),
        "unreachable_count": len(unreachable),
        "unreachable_fields": unreachable,
        "caveat": (
            "Regex over source, not a type-checked analysis. A field written "
            "only through an alias this pattern does not match would be a "
            "FALSE POSITIVE - confirm any new entry by grepping the field "
            "individually before reporting it as a gap."
        ),
    }


BASELINE = Path(__file__).with_name("reachability_baseline.json")


def check_reachability_regression(current: dict) -> tuple[bool, str]:
    """Fail if a capability CSS could reach has become unreachable.

    #32 made this number VISIBLE. Visible is not guarded: a refactor that
    drops an applier arm would silently kill a property, and the metric would
    print a larger number that nobody diffed. This is the ratchet.

    Compares the SET, not the count. A count cannot see a SWAP - close one
    capability while breaking another and the total is unchanged while a
    property died.

    Asymmetric on purpose: a field may only LEAVE the baseline. Closing one
    requires deleting it from the baseline file in the same PR, and that edit
    is the receipt - it makes the win explicit in the diff instead of letting
    the number drift down unremarked.
    """
    if not BASELINE.exists():
        return True, "no baseline file; skipping (first run)"
    allowed = set(json.loads(BASELINE.read_text(encoding="utf-8"))["unreachable_fields"])
    now = set(current.get("unreachable_fields", []))
    regressed = sorted(now - allowed)
    fixed = sorted(allowed - now)

    msg = []
    if fixed:
        msg.append(
            "REACHABILITY IMPROVED - now settable from CSS: "
            + ", ".join(f.replace("_", "-") for f in fixed)
            + ". Remove them from scripts/reachability_baseline.json."
        )
    if regressed:
        msg.append(
            "REACHABILITY REGRESSION - layout still reads these but nothing "
            "can set them any more: "
            + ", ".join(f.replace("_", "-") for f in regressed)
            + ". A property that used to work now silently does nothing."
        )
    return (not regressed), " | ".join(msg) or "reachability unchanged"


def collect(commit: str, branch: str) -> dict:
    build_code, build_out = run(["cargo", "build", "--workspace"])
    build_warnings = len(re.findall(r"^warning:", build_out, re.M))

    # On a failed build, echo the tail to stderr so CI shows WHY. The metrics
    # JSON only records exit_code; without this, a red build (often a missing
    # system -sys lib on a bare runner) is invisible in the step log.
    if build_code != 0:
        tail = "\n".join(build_out.splitlines()[-60:])
        print(f"\n=== cargo build --workspace FAILED (exit {build_code}) ===\n{tail}",
              file=sys.stderr)

    # --no-fail-fast so a failing crate does not stop the run: a metrics
    # collector must see EVERY crate's result, not a truncated prefix ending at
    # the first failure (that is the same "green incomplete detector" blind
    # spot the zero-test detector exists to avoid).
    test_code, test_out = run(["cargo", "test", "--workspace", "--no-fail-fast"])

    # Echo failing-test context to stderr so CI names WHICH tests failed and
    # why, instead of only a count. Mirrors the build-failure surfacing above.
    if test_code != 0:
        fail_lines = [ln for ln in test_out.splitlines()
                      if ("FAILED" in ln or "panicked at" in ln
                          or ln.strip().startswith("assertion")
                          or ln.strip().startswith("left:")
                          or ln.strip().startswith("right:"))]
        if fail_lines:
            print("\n=== FAILING TESTS ===\n" + "\n".join(fail_lines[:80]),
                  file=sys.stderr)

    # Attribute each "test result:" block to the binary that produced it.
    current = None
    per_crate: dict[str, dict[str, int]] = {}
    totals = {"passed": 0, "failed": 0, "ignored": 0}
    for line in test_out.splitlines():
        m = RUNNING_RE.search(line)
        if m:
            current = m.group(1)
            continue
        m = DOCTEST_RE.match(line)
        if m:
            current = m.group(1) + " (doc)"
            continue
        m = RESULT_RE.search(line)
        if m:
            _ok, p, f, i = m.group(1), int(m.group(2)), int(m.group(3)), int(m.group(4))
            key = current or "unattributed"
            bucket = per_crate.setdefault(key, {"passed": 0, "failed": 0, "ignored": 0})
            bucket["passed"] += p
            bucket["failed"] += f
            bucket["ignored"] += i
            totals["passed"] += p
            totals["failed"] += f
            totals["ignored"] += i

    # A crate with zero tests is worth surfacing: it is the shape of the
    # rustkit-text gap (compiled green, ran nothing, looked fine).
    empty = sorted(k for k, v in per_crate.items()
                   if v["passed"] == 0 and v["failed"] == 0 and not k.endswith("(doc)"))

    return {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "commit": commit,
        "branch": branch,
        "platform": "linux",
        "build": {
            "ok": build_code == 0,
            "exit_code": build_code,
            "warnings": build_warnings,
        },
        "tests": {
            "ok": test_code == 0,
            "exit_code": test_code,
            "passed": totals["passed"],
            "failed": totals["failed"],
            "ignored": totals["ignored"],
            "total": totals["passed"] + totals["failed"],
        },
        "per_crate": dict(sorted(per_crate.items())),
        "crates_with_no_tests": empty,
        "reachability": reachability(),
        "not_collected": {
            "parity_pixel_diff": (
                "requires a GPU adapter; not collectable on a hosted Linux "
                "runner. Deliberately omitted rather than emitting the "
                "harness's 100.0 default as if it were a measurement."
            )
        },
    }


def to_markdown(m: dict) -> str:
    b, t = m["build"], m["tests"]
    lines = [
        "## Linux Metrics",
        "",
        "| Metric | Value |",
        "|--------|-------|",
        f"| Build | {'PASS' if b['ok'] else 'FAIL'} ({b['warnings']} warnings) |",
        f"| Tests passed | **{t['passed']}** |",
        f"| Tests failed | {t['failed']} |",
        f"| Tests ignored | {t['ignored']} |",
        f"| Commit | `{m['commit'][:8]}` |",
        f"| Branch | {m['branch']} |",
    ]
    r = m.get("reachability") or {}
    if "unreachable_count" in r:
        lines.append(
            f"| CSS properties layout reads but nothing can set | "
            f"**{r['unreachable_count']}** of {r['read_by_layout']} |"
        )
    lines.append("")
    if r.get("unreachable_fields"):
        # Named, not just counted. A count says how far there is to go; the
        # list says WHICH capability is dead, which is the part someone can
        # act on. Each of these is layout code that exists and cannot be
        # reached from any stylesheet.
        lines += [
            "<details><summary>Unreachable from CSS "
            f"({r['unreachable_count']}) - implemented in layout, no applier arm</summary>",
            "",
            *(f"- `{f.replace('_', '-')}`" for f in r["unreachable_fields"]),
            "",
            "</details>",
            "",
        ]
    if m["crates_with_no_tests"]:
        lines += [
            "<details><summary>Crates running zero tests "
            f"({len(m['crates_with_no_tests'])})</summary>",
            "",
            # ASCII only: this string is written to a redirected stdout pipe,
            # which under a CI C/POSIX locale is ASCII, not UTF-8.
            "These compile but execute no tests - the same shape as the "
            "`rustkit-text` parity gap.",
            "",
        ]
        lines += [f"- `{c}`" for c in m["crates_with_no_tests"]]
        lines += ["", "</details>", ""]
    lines += [
        "<details><summary>Per-crate results</summary>",
        "",
        "| Crate | passed | failed | ignored |",
        "|-------|--------|--------|---------|",
    ]
    for name, v in m["per_crate"].items():
        lines.append(f"| {name} | {v['passed']} | {v['failed']} | {v['ignored']} |")
    lines += [
        "",
        "</details>",
        "",
        f"> Not collected: parity pixel diff - "
        f"{m['not_collected']['parity_pixel_diff']}",
    ]
    return "\n".join(lines)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--output")
    ap.add_argument("--input", help="render an existing metrics.json instead of collecting")
    ap.add_argument("--format", choices=["json", "markdown"], default="json")
    ap.add_argument("--commit", default="")
    ap.add_argument("--branch", default="")
    ap.add_argument(
        "--check-reachability",
        action="store_true",
        help="exit non-zero if a capability CSS could reach has become unreachable",
    )
    a = ap.parse_args()

    # Runs BEFORE collection: this check needs only the source tree, so it
    # gives its answer in a second instead of after a full workspace build.
    if a.check_reachability:
        ok, msg = check_reachability_regression(reachability())
        print(msg)
        if not ok:
            print(
                "Either restore the applier arm, or - if this is intentional - "
                "say so explicitly by editing scripts/reachability_baseline.json "
                "in the same PR.",
                file=sys.stderr,
            )
        return 0 if ok else 1

    if a.input:
        metrics = json.loads(Path(a.input).read_text(encoding="utf-8"))
    else:
        metrics = collect(a.commit, a.branch)

    if a.output:
        Path(a.output).write_text(json.dumps(metrics, indent=2), encoding="utf-8")

    if a.format == "markdown":
        sys.stdout.write(to_markdown(metrics) + "\n")
    elif not a.output:
        sys.stdout.write(json.dumps(metrics, indent=2) + "\n")

    # Collection succeeding is the contract here, not the tests passing --
    # the workflow records red builds as data rather than losing the run.
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
