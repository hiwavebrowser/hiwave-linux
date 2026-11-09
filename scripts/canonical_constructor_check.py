#!/usr/bin/env python3
"""Assert no PRODUCTION path builds a ComputedStyle through the wrong door.

WHY THIS EXISTS
---------------
`ComputedStyle` has two ways to produce "a fresh style" and they disagree:

    ComputedStyle::new()      -> the CSS initial values (min-width: auto, ...)
    ComputedStyle::default()  -> #[derive(Default)], so every field falls back
                                 to its own TYPE default, independently of what
                                 CSS says

`Length::default()` is `Zero`, but the Flexbox §4.5 initial for `min-width` is
`Auto`. So `default()` silently disagrees with `new()` on a field that decides
whether a flex item can be shrunk to nothing.

That is not hypothetical. Changing only `new()` when the §4.5 initial landed left
`inherit_from` handing back `Zero`, and the engine reaches every non-root element
through `inherit_from` -- the automatic minimum was correct in every unit test
and did nothing at all in the engine, with a fully green suite.

`min_width` is NOT special. The derived `Default` disagrees with `new()` on every
field `new()` spells out -- colour, opacity, flex-shrink, width, height,
background. `min_width` is merely the one a feature happened to branch on. The
rest are latent by the identical mechanism.

The real fix is to make `new()` fully explicit and have `Default` delegate to it,
removing the second door entirely. That touches ~100 fields and collides with the
exhaustive-destructure guards, so it rides a future refactor. Until then this
converts an invisible trap into a checked rule -- the same move as the
reachability ratchet.

NOTE a custom `impl Default { Self::new() }` is FORBIDDEN while `new()` ends in
`..Default::default()`: it is an infinite cycle, measured as a stack overflow.

TESTS MAY USE `default()`. A test that builds a style by the wrong door is
testing its own fixture, not shipping a defect to a user.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

CRATES = Path(__file__).resolve().parent.parent / "crates"
BASELINE = Path(__file__).with_name("canonical_constructor_baseline.json")

# The canonical constructor, and the door that disagrees with it.
FORBIDDEN = "ComputedStyle::default()"


def strip_test_modules(src: str) -> str:
    """Remove every `#[cfg(test)] mod ... { ... }` span, brace-matched.

    Deliberately NOT `src[:src.find("#[cfg(test)]")]`. Truncating at the first
    test module silently drops every line of PRODUCTION code that happens to sit
    after it, so a real violation below a mid-file test module would go unseen --
    a checker that reports "clean" because it stopped reading. `is_self_checking`
    below demonstrates the difference rather than asserting it in a comment.
    """
    out = []
    i = 0
    while True:
        m = re.compile(r"#\[cfg\(test\)\]").search(src, i)
        if not m:
            out.append(src[i:])
            break
        out.append(src[i : m.start()])
        brace = src.find("{", m.end())
        if brace == -1:
            # An attribute with no block after it (e.g. on a `use`): skip the
            # attribute itself and keep scanning, rather than discarding the
            # remainder of the file.
            i = m.end()
            continue
        depth, j = 0, brace
        while j < len(src):
            if src[j] == "{":
                depth += 1
            elif src[j] == "}":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        i = j + 1
    return "".join(out)


# `#[cfg(test)] mod name;` / `#[cfg(all(test, ..))] mod name;` declares a test module whose BODY lives in
# another file. `strip_test_modules` only removes brace-delimited modules, so those files were scanned as
# production: every macOS-style `foo_tests.rs` using `ComputedStyle::default()` (which the docstring above
# explicitly allows in tests) failed the check on every Linux PR (2026-11-09, 18 hits, all in test files).
_TEST_MOD_DECL = re.compile(
    r"#\[cfg\((?:all\(|any\()?[^\]]*\btest\b[^\]]*\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_0-9]+)\s*;"
)
_INNER_TEST_CFG = re.compile(r"^\s*#!\[cfg\([^\]]*\btest\b[^\]]*\]", re.M)


def test_files() -> set[Path]:
    """Files that are test code: declared `#[cfg(test)] mod x;`, `#![cfg(..test..)]` inside, under a
    `tests/` directory, or nested below a test module's own directory."""
    files: set[Path] = set()
    for path in CRATES.rglob("*.rs"):
        if "tests" in path.relative_to(CRATES).parts[:-1]:
            files.add(path)
            continue
        text = path.read_text()
        if _INNER_TEST_CFG.search(text[:2000]):
            files.add(path)
        # A module declared in `lib.rs`/`main.rs`/`mod.rs` lives beside it; in `foo.rs` under `foo/`.
        base = path.parent if path.name in ("lib.rs", "main.rs", "mod.rs") else path.parent / path.stem
        for m in _TEST_MOD_DECL.finditer(text):
            for cand in (base / f"{m.group(1)}.rs", base / m.group(1) / "mod.rs"):
                if cand.exists():
                    files.add(cand)
    # Everything nested under a test module's own directory (`close_tab_tests/tab_order_tests.rs`).
    grown = True
    while grown:
        grown = False
        for f in list(files):
            sub = f.parent / f.stem
            if sub.is_dir():
                for child in sub.rglob("*.rs"):
                    if child not in files:
                        files.add(child)
                        grown = True
    return files


def violations() -> list[tuple[str, int, str]]:
    found = []
    tests = test_files()
    for path in sorted(CRATES.rglob("*.rs")):
        if path in tests:
            continue
        prod = strip_test_modules(path.read_text())
        # Re-derive line numbers against the ORIGINAL file so the report points
        # at somewhere a human can actually look.
        original = path.read_text().splitlines()
        for lineno, line in enumerate(original, 1):
            if FORBIDDEN in line and line in prod:
                rel = path.relative_to(CRATES.parent).as_posix()
                found.append((rel, lineno, line.strip()))
    return found


def unbaselined(found, baseline) -> list[tuple[str, int, str]]:
    """The findings the baseline does not cover, matched by (path, source text) and COUNT.

    Not by (path, line): a line number in a 46k-line file that a macOS sync rewrites moves on every sync,
    so a baselined use stopped matching the next time anything above it changed and the check went red for
    a use nobody touched. The line in a baseline entry is informational. Counted, so a SECOND occurrence
    of the same text in the same file is still new.
    """
    allowed: dict[tuple[str, str], int] = {}
    for b in baseline:
        key = (b[0], b[2].strip())
        allowed[key] = allowed.get(key, 0) + 1
    new = []
    for rel, lineno, text in found:
        key = (rel, text.strip())
        if allowed.get(key, 0) > 0:
            allowed[key] -= 1
        else:
            new.append((rel, lineno, text))
    return new


def is_self_checking() -> bool:
    """Prove the checker can fail, and that the naive version could not.

    A production violation placed AFTER a test module is invisible to the
    truncate-at-first-cfg-test approach and visible to this one. If this ever
    stops holding, the checker has quietly become decorative.
    """
    sample = (
        "fn prod_before() { let _ = ComputedStyle::new(); }\n"
        "#[cfg(test)]\n"
        "mod t {\n"
        "    fn helper() { let _ = ComputedStyle::default(); }\n"
        "}\n"
        "fn prod_after() { let _ = ComputedStyle::default(); }\n"
    )
    naive = sample[: sample.find("#[cfg(test)]")]
    proper = strip_test_modules(sample)
    return (
        FORBIDDEN not in naive  # naive sees nothing: it stopped reading
        and FORBIDDEN in proper  # proper sees the real violation
        and proper.count(FORBIDDEN) == 1  # and does NOT flag the one in tests
    )


def main() -> int:
    if not is_self_checking():
        print(
            "SELF-CHECK FAILED: this checker can no longer distinguish a "
            "production violation after a test module from no violation at all. "
            "Fix the checker before trusting its output.",
            file=sys.stderr,
        )
        return 2

    baseline = set()
    if BASELINE.exists():
        baseline = {tuple(x) for x in json.loads(BASELINE.read_text())["allowed"]}

    found = violations()
    new = unbaselined(found, baseline)

    print(f"ComputedStyle::default() in production code: {len(found)}")
    if not new:
        print(
            f"SUMMARY: {len(found)} occurrence(s), all baselined; 0 new."
            if found
            else "SUMMARY: none. Production builds styles through ComputedStyle::new()."
        )
        return 0

    print()
    print("CANONICAL CONSTRUCTOR VIOLATION - production code built a ComputedStyle")
    print("through the derived Default, which does NOT hold the CSS initial values.")
    print("min-width is Zero there and Auto in new(); a flex item built this way")
    print("silently loses its Flexbox §4.5 content floor, and the suite stays green.")
    print()
    for rel, lineno, text in new:
        print(f"  {rel}:{lineno}  {text}")
    print()
    print("Use ComputedStyle::new(), or - if this really is intentional - say so")
    print("explicitly by adding it to scripts/canonical_constructor_baseline.json")
    print("in the same PR. That edit is the receipt.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
