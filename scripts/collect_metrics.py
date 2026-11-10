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
import time
from datetime import datetime, timezone
from pathlib import Path

# "test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
RESULT_RE = re.compile(
    r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored"
)
# "     Running unittests src\lib.rs (target\debug\deps\rustkit_text-abc123.exe)"
RUNNING_RE = re.compile(r"Running .*?\(.*?[\\/]deps[\\/]([A-Za-z0-9_]+?)-[0-9a-f]+")
DOCTEST_RE = re.compile(r"^\s*Doc-tests (\S+)")


# Lines worth showing live. The step used to print NOTHING between 'Run python ...' and its end, so a
# timeout (the 60-minute cancel of 2026-11-10) was undiagnosable: no way to tell the build from the tests,
# or which binary was running. The whole output is still captured for parsing; only these are echoed.
PROGRESS_RE = re.compile(
    r"^\s*(Running |Doc-tests |Finished |Compiling (?:rustkit|hiwave)|test result:|error(?:\[|:)|warning: unused)"
    r"|\.\.\. FAILED|panicked at|has been running for over"
)


def run(cmd: list[str], label: str = "") -> tuple[int, str]:
    """Run `cmd`, capture everything, and echo progress lines to stderr as they arrive.

    stderr is merged into stdout by the PIPE, not concatenated afterwards.
    cargo prints "Running <binary>" on stderr and "test result:" on stdout,
    so appending one to the other loses the interleaving that attributes a
    result block to the binary that produced it -- every crate then lands
    in a single bucket.
    """
    started = time.monotonic()
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                            text=True, encoding="utf-8", errors="replace", bufsize=1)
    lines: list[str] = []
    assert proc.stdout is not None
    for line in proc.stdout:
        lines.append(line)
        if PROGRESS_RE.search(line):
            elapsed = time.monotonic() - started
            print(f"[{label or cmd[1]} +{elapsed:7.0f}s] {line.rstrip()}", file=sys.stderr, flush=True)
    code = proc.wait()
    run.last_seconds = time.monotonic() - started  # type: ignore[attr-defined]
    return code, "".join(lines)


APPLIER_FN = "fn apply_style_property_impl"

# ComputedStyle fields the layout reads that are ENGINE-INTERNAL STATE, not CSS properties: nothing in
# the applier can set them because no author declaration maps to them. Each is set by the cascade or the
# animation system. Listing one here is not an exemption from evidence: `reachability()` still requires
# an assignment to it in the engine's non-test source, so a removed setter re-fires the ratchet.
ENGINE_STATE = {
    "abspos_was_inline_level": "cascade flag: the element's display before out-of-flow blockification",
    "animation_settled_opacity": "animation system: the opacity a settled animation leaves for paint",
    "webkit_scrollbar": "cascade: built from ::-webkit-scrollbar pseudo-element rules, not a property",
}


def _applier_body(engine: str) -> tuple[str, bool]:
    """The body of the CSS applier, brace-matched.

    Returns (body, fell_back). If the applier cannot be located - renamed,
    moved, split - this returns the WHOLE FILE and flags it, loudly, in three
    places: stderr, a caveat key in the JSON, and the CI markdown summary.

    The loudness is the point and it is not decoration. A silent whole-file
    fallback reintroduces exactly the false-reachable this scoping removes:
    internal plumbing would count as writability again and capabilities would
    silently leave the work-list while still being dead. Argos caught that
    this function CLAIMED to say so and did not - the claim was in the
    docstring and nowhere in the code, which is the same "correct rule,
    incomplete implementation" shape this PR was written to fix.
    """
    # TOKEN-BOUNDARY match, not substring. `find()` would still match after a
    # rename to `apply_inline_style_decls_RENAMED` and silently scope to that
    # function's body without firing the fallback - Argos's soft note on #37.
    # A rename that ADDS a suffix is exactly the shape a refactor produces, so
    # the substring form is loud only for the renames least likely to happen.
    # EVERY definition, not the first. Linux carries a thin `Engine::apply_style_property_impl`
    # forwarder (the wave-4 seam: a 3-line body that calls LayoutCtx's) BEFORE the real applier; the
    # first match scoped the body to that forwarder, which writes no field, so every field the layout
    # reads counted as unreachable (a whole-tree REGRESSION on every PR, 2026-11-09). The applier is
    # the definition with the largest body; a forwarder is never where a CSS arm lives.
    starts = [m.start() for m in re.finditer(re.escape(APPLIER_FN) + r"\s*[(<]", engine)]
    if not starts:
        print(
            f"WARNING: could not find `{APPLIER_FN}` - reachability is falling "
            f"back to a WHOLE-FILE scan. Internal plumbing will count as "
            f"writability and the unreachable count will be UNDER-reported. "
            f"Fix the function name in {__file__} before trusting this number.",
            file=sys.stderr,
        )
        return engine, True
    return max((_brace_body(engine, i) for i in starts), key=len), False


def _brace_body(text: str, i: int) -> str:
    """The `{ ... }` block that opens at or after `i`, brace-matched."""
    j = text.find("{", i)
    depth, k = 0, j
    while k < len(text):
        if text[k] == "{":
            depth += 1
        elif text[k] == "}":
            depth -= 1
            if depth == 0:
                break
        k += 1
    return text[j:k]


def _helper_bodies(applier: str, sources: dict) -> str:
    """Bodies of the functions the applier calls WITH `style` as an argument.

    A CSS arm often delegates (`animations::set_animation_names(style, value)`): the field is
    assigned in the helper, not in the applier's own text, and is exactly as settable from CSS.
    Only DIRECT callees, only those handed the style: a helper the applier never gives the style to
    cannot write it, and recursing further would drift toward the whole-file scan this scoping
    exists to avoid (a false REACHABLE is the expensive direction).
    """
    names = set(re.findall(r"\b([a-z_][a-z_0-9]*)\(\s*(?:&mut\s+)?(?:style|s)\b", applier))
    names.discard("apply_style_property_impl")
    out = []
    for text in sources.values():
        for name in names:
            for m in re.finditer(r"\bfn " + re.escape(name) + r"\b\s*[(<]", text):
                out.append(_brace_body(text, m.start()))
    return "\n".join(out)


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

    # WRITABLE means "an applier arm can set it from CSS" - so only the
    # APPLIER's body counts, not the whole engine file.
    #
    # A whole-file scan counts internal plumbing as writability and produces a
    # FALSE REACHABLE. Worked example, caught by this metric's own ratchet
    # contradicting a fact I knew: text-decoration-thickness has NO arm (the
    # macOS reference has none, so wiring one would be a divergence), but the
    # text-run propagation block assigns the field. Whole-file scanning called
    # it settable and dropped it off the list. Nothing can set it.
    #
    # This is the same applier-scoping correction I published for the
    # reference-subset check and then failed to apply to the metric itself.
    applier, applier_fallback = _applier_body(engine)

    # A field is WRITABLE if the applier assigns it OR MUTATES IT IN PLACE.
    # Collection-valued properties are set by method, never by assignment:
    # the `box-shadow` arm does `style.box_shadows.clear()` and
    # `.push(shadow)`. An assignment-only regex called that field unwritable,
    # and it stayed invisible until #41 added the first LAYOUT READ of
    # box_shadows - at which point the ratchet fired a REGRESSION for a
    # property that had worked since A2.
    #
    # An explicit mutator list rather than "any method call": `.to_px()` and
    # `.is_visible()` are reads, and treating every receiver as a write would
    # mark fields writable that no arm can set - a FALSE REACHABLE, the
    # expensive direction.
    # `apply`/`reset` are the mask-image mutators (`style.mask.apply(..)`, `style.mask.reset()`).
    MUTATORS = "push|clear|insert|extend|remove|pop|truncate|retain|append|push_str|set|apply|reset"
    # What the applier delegates to is part of the applier: see `_helper_bodies`.
    engine_dir = Path("crates/rustkit-engine/src")
    sources = {p.name: p.read_text() for p in sorted(engine_dir.glob("*.rs"))
               if not p.name.endswith("_tests.rs")}
    scope = applier + "\n" + _helper_bodies(applier, sources)

    def _writes(f: str) -> bool:
        for recv in ("style", "s"):
            r = r"\b" + recv + r"\." + f
            if (re.search(r + r"\s*=(?!=)", scope)                          # assignment
                    or re.search(r + r"\s*(?:[-+*/&|^]|<<|>>)=", scope)     # compound: &=, |=, +=, ...
                    or re.search(r + r"\.(?:" + MUTATORS + r")\s*\(", scope)  # in-place mutator
                    # tuple target: `(style.a, style.b) = ..` (either position)
                    or re.search(r"\([^()=;]*" + r + r"\b[^()=;]*\)\s*=(?!=)", scope)):
                return True
        return False

    engine_src = "\n".join(sources.values())
    written = [f for f in fields
               if _writes(f)
               or (f in ENGINE_STATE and re.search(r"\.(?:" + f + r")\s*=(?!=)", engine_src))]
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
        # Loud in the artefact, not only on stderr: whoever reads this JSON
        # six weeks from now will not have the CI log.
        "applier_body_fallback": applier_fallback,
        "this_is_a_floor_not_a_proof": (
            "WRITABLE IS NOT THE SAME AS VISIBLE. A name leaving this list "
            "proves only that an applier arm exists - NOT that the value "
            "reaches paint. text-decoration is the worked example: it is not "
            "inherited, and rustkit-layout emits the decoration commands from "
            "the TEXT box's style, so wiring the arms alone drops the name "
            "from this list while the page still renders undecorated. "
            "Athena hit exactly this on hiwave-windows #64 and only her "
            "layout-reaching test group caught it. "
            "So: a falling count is necessary evidence, never sufficient. "
            "Any wire unit still owes a Group B test that asserts the value "
            "REACHED the box or the display list. Do not cite this metric as "
            "end-to-end proof - that would be the same overclaim the metric "
            "was built to expose."
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


KNOWN_FAILURES = Path(__file__).with_name("linux_known_failures.json")
FAILED_RE = re.compile(r"^test (\S+) \.\.\. FAILED\s*$")


def classify_failures(failed_names: list[str], known_path: Path = KNOWN_FAILURES) -> dict:
    """Split failing tests into the ones this tree is KNOWN to fail and the ones it is not.

    Linux carries tests whose expectations are Chrome-on-macOS font metrics (one FreeType run face, no
    fallback-face line union, runner fonts that are not the reference machine's). Counting them as
    failures turned the job red on every PR, and a check that is always red gates nothing; deleting or
    `#[ignore]`-ing them would hide a real signal. They are listed, with a reason each, in
    scripts/linux_known_failures.json and reported on every run; ONLY a failure outside that list
    fails the job. A known failure that now passes is reported (remove its entry) but is not red: the
    hosted runner's fonts differ from a developer machine's, so the same test can flip between them.
    """
    known: dict[str, str] = {}
    if known_path.exists():
        known = {e["test"]: e.get("reason", "") for e in json.loads(known_path.read_text(encoding="utf-8"))["known"]}
    seen = set(failed_names)
    return {
        "failed_unexpected": sorted(n for n in seen if n not in known),
        "failed_known": sorted(n for n in seen if n in known),
        "known_now_passing": sorted(k for k in known if k not in seen),
    }


def parse_tests(test_out: str) -> tuple[dict, dict, list[str]]:
    """Per-binary results, totals, and the names of the failing tests (`binary::test_path`)."""
    # Attribute each "test result:" block to the binary that produced it.
    current = None
    per_crate: dict[str, dict[str, int]] = {}
    totals = {"passed": 0, "failed": 0, "ignored": 0}
    failed_names: list[str] = []
    for line in test_out.splitlines():
        fm = FAILED_RE.match(line)
        if fm:
            failed_names.append(f"{current or 'unattributed'}::{fm.group(1)}")
            continue
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
    return per_crate, totals, failed_names


def collect(commit: str, branch: str) -> dict:
    build_code, build_out = run(["cargo", "build", "--workspace"], "build")
    build_seconds = round(run.last_seconds)  # type: ignore[attr-defined]
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
    # --test-threads=1: engine tests share one software GPU behind a guard that
    # panics a waiter after 120s (test_gpu::MAX_WAIT). In parallel the queue of
    # GPU tests is longer than that on a CI runner, so the unlucky tail fails
    # on the wait, not on its assertions (5 tests, run 36804275308); serial
    # runs the whole engine suite in ~50s locally with no contention.
    test_code, test_out = run(["cargo", "test", "--workspace", "--no-fail-fast",
                               "--", "--test-threads=1"], "test")
    test_seconds = round(run.last_seconds)  # type: ignore[attr-defined]
    print(f"PHASES: build {build_seconds}s, test {test_seconds}s", file=sys.stderr, flush=True)

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

    per_crate, totals, failed_names = parse_tests(test_out)

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
            "seconds": build_seconds,
        },
        "tests": {
            "ok": test_code == 0,
            "exit_code": test_code,
            "passed": totals["passed"],
            "failed": totals["failed"],
            "ignored": totals["ignored"],
            "total": totals["passed"] + totals["failed"],
            "seconds": test_seconds,
            **classify_failures(failed_names),
            # A non-zero exit that names no failing test is a crash or a timeout, not a pass.
            "unexplained_exit": test_code != 0 and not failed_names,
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
        f"| Tests failed | {t['failed']} ({len(t.get('failed_known', []))} known on Linux, "
        f"{len(t.get('failed_unexpected', []))} unexpected) |",
        f"| Tests ignored | {t['ignored']} |",
        f"| Commit | `{m['commit'][:8]}` |",
        f"| Branch | {m['branch']} |",
    ]
    r = m.get("reachability") or {}
    if r.get("applier_body_fallback"):
        # Loudest of the three signals, because this is the one a human reads.
        # Without it the count below looks like an ordinary improvement.
        lines += [
            "",
            "> **WARNING - reachability fell back to a whole-file scan.** The CSS "
            "applier function could not be located, so internal plumbing counts "
            "as writability and the number below is UNDER-reported. Fix the "
            "function name in `scripts/collect_metrics.py` before trusting it.",
        ]
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
            "**This count is a floor, not a proof.** A name leaving this list "
            "means an applier arm exists - not that the value reaches paint. "
            "A wire unit still owes a test that asserts the value reached the "
            "layout box or the display list.",
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
