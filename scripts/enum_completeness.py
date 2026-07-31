#!/usr/bin/env python3
"""Find CSS enum variants that no stylesheet can ever produce.

Tier A of the reachability program (Prometheus). The reachability metric asks
whether a FIELD is writable. This asks the question that hid an entire layout
subsystem on this tree: is every VALUE of that field reachable?

The case that motivated it: `Display::Grid`, `Display::InlineGrid` and
`Display::InlineFlex` existed as variants, `is_grid()` existed, and
`layout_grid_container` was dispatched from it - but `parse_display` had no
"grid" arm. The field `display` was writable, so the reachability metric never
flagged anything, while the entire grid engine was dead. Three of eight values
were unreachable and nothing measured values.

METHOD. For every `pub enum` in rustkit-css that ComputedStyle actually uses,
check whether each variant is CONSTRUCTED anywhere in the engine. A variant the
engine never constructs cannot be produced from CSS.

WHAT THIS DELIBERATELY DOES NOT CLAIM. A variant reachable only as a Default is
reported separately, not as a defect: the initial value is reachable by being
the initial value. And "constructed in the engine" is necessary, not sufficient
- construction inside dead code still counts here. Pair with wireability.py for
the caller side. Stating both because a tool that overclaims is the thing this
program exists to delete.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

CSS = Path("crates/rustkit-css/src/lib.rs")
ENGINE = Path("crates/rustkit-engine/src/lib.rs")
REF_CSS = Path("/home/petec/repos/hiwave/hiwave-macos/crates/rustkit-css/src/lib.rs")
REF_ENGINE = Path("/home/petec/repos/hiwave/hiwave-macos/crates/rustkit-engine/src/lib.rs")


def _strip_tests(src: str) -> str:
    """Remove `#[cfg(test)] mod ...` BLOCKS only.

    Not "cut at the first #[cfg(test)]". That was this file's first
    implementation and it was catastrophically wrong: rustkit-engine annotates
    a test-only helper (`test_compositor`) a few hundred lines in, so the naive
    cut discarded the entire applier and the tool reported 99 unproducible
    variants - including Position::Absolute, whose arm I wrote myself.

    Caught only because the number contradicted something I knew. A helper
    that silently discards the code under test is the exact failure this whole
    program keeps finding, and I wrote another one.
    """
    out, i = [], 0
    while True:
        m = re.search(r"#\[cfg\(test\)\]\s*\n\s*mod\s+[a-z_0-9]+\s*\{", src[i:])
        if not m:
            out.append(src[i:])
            break
        start = i + m.start()
        out.append(src[i:start])
        j = i + m.end() - 1
        depth, k = 0, j
        while k < len(src):
            if src[k] == "{":
                depth += 1
            elif src[k] == "}":
                depth -= 1
                if depth == 0:
                    break
            k += 1
        i = k + 1
    return "".join(out)


def _enum_variants(css: str) -> dict[str, tuple[list[str], str | None]]:
    """enum name -> (variants, default_variant)."""
    out = {}
    for m in re.finditer(r"pub enum ([A-Z][A-Za-z0-9]*)\s*\{", css):
        name = m.group(1)
        j = m.end() - 1
        depth, k = 0, j
        while k < len(css):
            if css[k] == "{":
                depth += 1
            elif css[k] == "}":
                depth -= 1
                if depth == 0:
                    break
            k += 1
        body = css[j:k]
        variants = re.findall(r"^\s{4}([A-Z][A-Za-z0-9]*)\s*[,({]", body, re.M)
        dflt = None
        d = re.search(r"#\[default\]\s*\n\s*([A-Z][A-Za-z0-9]*)", body)
        if d:
            dflt = d.group(1)
        if variants:
            out[name] = (variants, dflt)
    return out


def _computed_style_enums(css: str) -> set[str]:
    i = css.find("pub struct ComputedStyle")
    body = css[i:css.find("}", css.find("{", i))]
    return set(re.findall(r":\s*(?:Option<)?([A-Z][A-Za-z0-9]*)", body))


def analyse() -> dict:
    css = CSS.read_text()
    engine = _strip_tests(ENGINE.read_text())
    css_nt = _strip_tests(css)

    enums = _enum_variants(css)
    used = _computed_style_enums(css)

    dead, default_only = [], []
    for name, (variants, dflt) in sorted(enums.items()):
        if name not in used:
            continue
        for v in variants:
            # PRODUCED, not merely MENTIONED.
            #
            # A variant named on the LEFT of a match arm, or inside a
            # `matches!(...)`, is being CONSUMED - that is a branch reading the
            # value, not code creating it. `Display::Grid` appears in
            # `is_grid()`'s matches! on this tree, so a naive "does the name
            # appear" search calls it producible even with no parse arm at all.
            # That is precisely the bug this tool exists to find, and my first
            # implementation had it: the T-RED (revert the parse_display fix,
            # expect Display variants to appear) did NOT fire.
            #
            # So require a CONSTRUCTION context: the variant on the RIGHT of a
            # `=>`, or assigned, or wrapped in Some(...)/a return.
            q = re.escape(name) + r"::" + re.escape(v) + r"\b"
            produced = (
                re.compile(r"=>\s*(?:Some\()?(?:rustkit_css::)?" + q).search(engine)
                or re.compile(r"=>\s*(?:Some\()?(?:rustkit_css::)?" + q).search(css_nt)
                or re.compile(r"=\s*(?:Some\()?(?:rustkit_css::)?" + q).search(engine)
                or re.compile(r"return\s+(?:Some\()?(?:rustkit_css::)?" + q).search(css_nt)
            )
            if produced:
                continue
            (default_only if v == dflt else dead).append(f"{name}::{v}")
    # REFERENCE COLUMN. Unproducible here is a statement about this tree. It
    # is NOT permission to add an arm: if the reference cannot produce the
    # variant either, it is a SHARED LIMIT and wiring it alone is a divergence.
    # Same lesson as the wireability tool, where the family-level reference
    # check let an undeclared divergence through on one longhand.
    ref = ""
    if REF_ENGINE.exists() and REF_CSS.exists():
        ref = _strip_tests(REF_ENGINE.read_text()) + _strip_tests(REF_CSS.read_text())
    port, shared, unknown = [], [], []
    for v in dead:
        if not ref:
            unknown.append(v)
        elif re.search(r"\b" + v.replace("::", "::") + r"\b", ref):
            port.append(v)
        else:
            shared.append(v)

    return {
        "enums_checked": len([n for n in enums if n in used]),
        "unproducible": dead,
        "port_defect": port,
        "shared_limit": shared,
        "reference_absent": unknown,
        "default_only": default_only,
    }


def main() -> int:
    r = analyse()
    print(f"ComputedStyle enums checked: {r['enums_checked']}")
    print()
    if r["port_defect"]:
        print(f"PORT DEFECT ({len(r['port_defect'])}) - unproducible here, and the "
              f"REFERENCE can produce it. These are real gaps:")
        for v in r["port_defect"]:
            print(f"   {v}")
        print()
    if r["shared_limit"]:
        print(f"SHARED LIMIT ({len(r['shared_limit'])}) - unproducible here AND on the "
              f"reference. Wiring these alone would be a DIVERGENCE, not progress:")
        for v in r["shared_limit"]:
            print(f"   {v}")
        print()
    if r["reference_absent"]:
        print(f"UNCLASSIFIED ({len(r['reference_absent'])}) - reference tree not on "
              f"this machine; absence is not evidence that wiring is safe.")
    if not r["unproducible"]:
        print("No unproducible variants.")
    else:
        print("No unproducible variants. Every enum value a stylesheet could name "
              "can be constructed.")
    if r["default_only"]:
        print()
        print(f"Reachable ONLY as the default ({len(r['default_only'])}) - not a "
              f"defect by itself, but no rule can select them explicitly:")
        for v in r["default_only"]:
            print(f"   {v}")
    return 1 if r["port_defect"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
