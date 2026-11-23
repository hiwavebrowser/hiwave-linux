#!/usr/bin/env python3
"""
display_guard.py: a test or capture run never draws on someone's live desktop.

Some tests and every capture open real windows when they find a display
(rustkit-engine's `linux_carried_tests::x11_content_path` does; with no
DISPLAY they skip). A run that inherits the shell's DISPLAY=:0 or
WAYLAND_DISPLAY puts those windows on the live desktop of whoever is logged
in. So a run on a workstation names its display: a private Xvfb (for example
`DISPLAY=:99`, see the gate-runner README), or no display at all, as CI runs.

Refused:
  - WAYLAND_DISPLAY set (a live Wayland session; a test run has no use for it);
  - DISPLAY naming display 0 or 1 (`:0`, `:1.0`, `unix:0`, `localhost:1`): the
    numbers a desktop session takes.
Allowed: DISPLAY unset or empty; any other display number.
HIWAVE_ALLOW_LIVE_DISPLAY=1 lets a person who wants to watch the windows through.

Used by scripts/collect_metrics.py (before `cargo test --workspace`) and
scripts/chrome_board.py (before captures). From a shell:
    python3 scripts/display_guard.py && cargo test ...
exits 1 with the reason when refused.
"""

import os
import re
import sys
from typing import Mapping, Optional

# Display numbers a desktop session takes; a private Xvfb is given another.
LIVE_DISPLAY_NUMBERS = {0, 1}


def display_number(display: str) -> Optional[int]:
    """The display number of an X DISPLAY string (`host:N.S`), or None."""
    m = re.fullmatch(r"[^:]*:(\d+)(?:\.\d+)?", display.strip())
    return int(m.group(1)) if m else None


def live_display_reason(environ: Mapping[str, str]) -> Optional[str]:
    """Why this environment would draw on a live desktop, or None when it would not."""
    if environ.get("HIWAVE_ALLOW_LIVE_DISPLAY") == "1":
        return None
    wayland = environ.get("WAYLAND_DISPLAY", "")
    if wayland:
        return (f"WAYLAND_DISPLAY={wayland} is set (a live Wayland session); "
                "unset it for a test or capture run")
    display = environ.get("DISPLAY", "")
    if display:
        n = display_number(display)
        if n is None:
            return f"DISPLAY={display} is not a display this guard can read; name a private Xvfb (DISPLAY=:99)"
        if n in LIVE_DISPLAY_NUMBERS:
            return (f"DISPLAY={display} is a desktop session's display; "
                    "run on a private Xvfb (DISPLAY=:99) or with DISPLAY unset")
    return None


def refuse_live_display(environ: Optional[Mapping[str, str]] = None, tool: str = "this run") -> None:
    """Exit with status 1 and the reason when `environ` would draw on a live desktop."""
    reason = live_display_reason(os.environ if environ is None else environ)
    if reason:
        print(f"{tool}: refused: {reason} (HIWAVE_ALLOW_LIVE_DISPLAY=1 to override)", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    refuse_live_display(tool="display_guard")
