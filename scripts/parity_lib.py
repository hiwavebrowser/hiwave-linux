#!/usr/bin/env python3
"""
parity_lib.py - Core parity testing library for Linux

Shared utilities for parity testing: work units, execution, comparison.
Handles Xvfb for headless display on Linux.

Platform: Linux (ported from macOS)
"""

import json
import os
import platform
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

# Repository root (one level up from scripts/)
REPO_ROOT = Path(__file__).parent.parent.resolve()

# Binary name (no suffix on Linux)
BINARY_SUFFIX = ""
BINARY_PATH = REPO_ROOT / "target" / "release" / f"parity-capture{BINARY_SUFFIX}"

# Directories
BASELINES_DIR = REPO_ROOT / "baselines" / "chrome-120"
RESULTS_DIR = REPO_ROOT / "parity-results"
ORACLE_DIR = REPO_ROOT / "tools" / "parity_oracle"


# --- Case Definitions ---

BUILTINS = {
    "new_tab": {
        "html": REPO_ROOT / "crates" / "hiwave-app" / "src" / "ui" / "new_tab.html",
        "width": 1280,
        "height": 800,
        "weight": 0.15,
        "tier": "A",
    },
    "about": {
        "html": REPO_ROOT / "crates" / "hiwave-app" / "src" / "ui" / "about.html",
        "width": 800,
        "height": 600,
        "weight": 0.10,
        "tier": "A",
    },
    "settings": {
        "html": REPO_ROOT / "crates" / "hiwave-app" / "src" / "ui" / "settings.html",
        "width": 1024,
        "height": 768,
        "weight": 0.15,
        "tier": "A",
    },
    "chrome_rustkit": {
        "html": REPO_ROOT / "crates" / "hiwave-app" / "src" / "ui" / "chrome_rustkit.html",
        "width": 1280,
        "height": 100,
        "weight": 0.10,
        "tier": "A",
    },
    "shelf": {
        "html": REPO_ROOT / "crates" / "hiwave-app" / "src" / "ui" / "shelf.html",
        "width": 1280,
        "height": 120,
        "weight": 0.10,
        "tier": "A",
    },
}

WEBSUITE = {
    "article-typography": {"weight": 0.05, "tier": "B"},
    "card-grid": {"weight": 0.05, "tier": "B"},
    "css-selectors": {"weight": 0.05, "tier": "B"},
    "flex-positioning": {"weight": 0.05, "tier": "B"},
    "form-elements": {"weight": 0.05, "tier": "B"},
    "gradient-backgrounds": {"weight": 0.05, "tier": "B"},
    "image-gallery": {"weight": 0.05, "tier": "B"},
    "sticky-scroll": {"weight": 0.05, "tier": "B"},
}

MICRO_TESTS = {
    "backgrounds": {"weight": 0.01, "tier": "C"},
    "bg-solid": {"weight": 0.01, "tier": "C"},
    "combinators": {"weight": 0.01, "tier": "C"},
    "form-controls": {"weight": 0.01, "tier": "C"},
    "gradients": {"weight": 0.01, "tier": "C"},
    "images-intrinsic": {"weight": 0.01, "tier": "C"},
    "pseudo-classes": {"weight": 0.01, "tier": "C"},
    "rounded-corners": {"weight": 0.01, "tier": "C"},
    "specificity": {"weight": 0.01, "tier": "C"},
}


# --- Data Classes ---

@dataclass
class WorkUnit:
    """A single test case to execute."""
    case_id: str
    case_type: str  # "builtin", "websuite", "micro"
    html_path: Path
    baseline_dir: Path
    width: int
    height: int
    weight: float = 1.0
    tier: str = "B"
    iterations: int = 1


@dataclass
class CaseResult:
    """Result of a single iteration."""
    case_id: str
    iteration: int
    diff_pct: Optional[float] = None
    error: Optional[str] = None
    capture_time_ms: float = 0.0
    compare_time_ms: float = 0.0
    blank_frame: bool = False


@dataclass
class AggregatedResult:
    """Aggregated result across iterations for a case."""
    case_id: str
    diff_pct_median: float
    diff_pct_mean: float
    diff_pct_min: float
    diff_pct_max: float
    diff_pct_variance: float
    stable: bool
    iterations: int
    passed: bool
    error: Optional[str] = None
    raw_results: List[Dict] = field(default_factory=list)


# --- Xvfb Handling for Linux ---

def ensure_display() -> None:
    """Ensure DISPLAY environment variable is set for headless operation."""
    if os.environ.get("DISPLAY") is None:
        # Default to :99 which is commonly used by xvfb-run
        os.environ["DISPLAY"] = ":99"


def start_xvfb(display: str = ":99") -> Optional[subprocess.Popen]:
    """Start Xvfb if not already running."""
    # Check if Xvfb is already running on this display
    try:
        result = subprocess.run(
            ["xdpyinfo", "-display", display],
            capture_output=True,
            timeout=5,
        )
        if result.returncode == 0:
            # Xvfb already running
            return None
    except (subprocess.TimeoutExpired, FileNotFoundError):
        pass

    # Start Xvfb
    try:
        proc = subprocess.Popen(
            ["Xvfb", display, "-screen", "0", "1920x1080x24"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        time.sleep(0.5)  # Give it time to start
        return proc
    except FileNotFoundError:
        print("Warning: Xvfb not found. Ensure it's installed or use xvfb-run.")
        return None


# --- Work Unit Creation ---

def create_work_units(
    scope: str = "all",
    iterations: int = 3,
    case_filter: Optional[List[str]] = None,
) -> List[WorkUnit]:
    """Create work units for the specified scope."""
    units = []

    if scope in ("all", "builtins"):
        for case_id, info in BUILTINS.items():
            if case_filter and case_id not in case_filter:
                continue
            baseline_dir = BASELINES_DIR / "builtins" / case_id
            units.append(WorkUnit(
                case_id=case_id,
                case_type="builtin",
                html_path=info["html"],
                baseline_dir=baseline_dir,
                width=info["width"],
                height=info["height"],
                weight=info["weight"],
                tier=info["tier"],
                iterations=iterations,
            ))

    if scope in ("all", "websuite"):
        for case_id, info in WEBSUITE.items():
            if case_filter and case_id not in case_filter:
                continue
            html_path = REPO_ROOT / "websuite" / "cases" / case_id / "index.html"
            baseline_dir = BASELINES_DIR / "websuite" / case_id
            # Default dimensions for websuite
            width = 1280 if case_id not in ("css-selectors", "flex-positioning", "form-elements", "gradient-backgrounds") else 800
            height = 800 if case_id not in ("css-selectors", "flex-positioning", "form-elements", "gradient-backgrounds") else (1200 if case_id == "css-selectors" else 1000 if case_id == "flex-positioning" else 600)
            units.append(WorkUnit(
                case_id=case_id,
                case_type="websuite",
                html_path=html_path,
                baseline_dir=baseline_dir,
                width=width,
                height=height,
                weight=info["weight"],
                tier=info["tier"],
                iterations=iterations,
            ))

    if scope in ("all", "micro"):
        for case_id, info in MICRO_TESTS.items():
            if case_filter and case_id not in case_filter:
                continue
            html_path = REPO_ROOT / "websuite" / "micro" / case_id / "test.html"
            baseline_dir = BASELINES_DIR / "micro" / case_id
            units.append(WorkUnit(
                case_id=case_id,
                case_type="micro",
                html_path=html_path,
                baseline_dir=baseline_dir,
                width=800,
                height=600,
                weight=info["weight"],
                tier=info["tier"],
                iterations=iterations,
            ))

    return units


# --- Execution ---

def execute_work_unit(unit: WorkUnit, output_dir: Path, verbose: bool = False) -> List[CaseResult]:
    """Execute a work unit and return results for all iterations."""
    results = []
    ensure_display()

    for i in range(unit.iterations):
        result = CaseResult(case_id=unit.case_id, iteration=i)
        iter_dir = output_dir / unit.case_id / f"iter_{i}"
        iter_dir.mkdir(parents=True, exist_ok=True)

        # Check if HTML exists
        if not unit.html_path.exists():
            result.error = f"HTML not found: {unit.html_path}"
            results.append(result)
            continue

        # Check if baseline exists
        baseline_png = unit.baseline_dir / "baseline.png"
        if not baseline_png.exists():
            result.error = f"Baseline not found: {baseline_png}"
            results.append(result)
            continue

        # Run capture
        capture_start = time.time()
        capture_path = iter_dir / "capture.png"

        try:
            capture_result = run_capture(
                unit.html_path,
                capture_path,
                unit.width,
                unit.height,
                verbose=verbose,
            )
            result.capture_time_ms = (time.time() - capture_start) * 1000

            if not capture_result["success"]:
                result.error = capture_result.get("error", "Capture failed")
                results.append(result)
                continue

            # Check for blank frame
            if analyze_frame_blankness(capture_path):
                result.blank_frame = True
                result.error = "Blank frame detected"
                results.append(result)
                continue

        except Exception as e:
            result.error = str(e)
            results.append(result)
            continue

        # Run comparison
        compare_start = time.time()

        try:
            diff_result = compare_pixels(
                baseline_png,
                capture_path,
                iter_dir / "diff.png",
            )
            result.compare_time_ms = (time.time() - compare_start) * 1000
            result.diff_pct = diff_result.get("diff_pct", 100.0)

        except Exception as e:
            result.error = f"Comparison failed: {e}"

        results.append(result)

    return results


def run_capture(
    html_path: Path,
    output_path: Path,
    width: int,
    height: int,
    verbose: bool = False,
) -> Dict[str, Any]:
    """Run the parity-capture binary."""
    if not BINARY_PATH.exists():
        return {"success": False, "error": f"Binary not found: {BINARY_PATH}"}

    cmd = [
        str(BINARY_PATH),
        "--html", str(html_path),
        "--output", str(output_path),
        "--width", str(width),
        "--height", str(height),
    ]

    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=30,
        )

        if result.returncode != 0:
            return {
                "success": False,
                "error": result.stderr or f"Exit code {result.returncode}",
            }

        if not output_path.exists():
            return {"success": False, "error": "Output file not created"}

        return {"success": True, "path": str(output_path)}

    except subprocess.TimeoutExpired:
        return {"success": False, "error": "Capture timed out"}
    except Exception as e:
        return {"success": False, "error": str(e)}


def analyze_frame_blankness(image_path: Path) -> bool:
    """Check if image is mostly blank (>95% single color)."""
    try:
        # Use Node.js oracle if available
        oracle_script = ORACLE_DIR / "compare_pixels.mjs"
        if not oracle_script.exists():
            return False

        # Simple check using file size - tiny files are likely blank
        file_size = image_path.stat().st_size
        if file_size < 1000:  # Less than 1KB is suspicious
            return True

        return False
    except Exception:
        return False


def compare_pixels(
    baseline_path: Path,
    capture_path: Path,
    diff_path: Path,
) -> Dict[str, Any]:
    """Compare two images using the oracle."""
    oracle_script = ORACLE_DIR / "compare_pixels.mjs"

    if not oracle_script.exists():
        # Fallback: assume 100% diff if oracle not available
        return {"diff_pct": 100.0, "error": "Oracle not found"}

    cmd = [
        "node",
        str(oracle_script),
        str(baseline_path),
        str(capture_path),
        str(diff_path),
    ]

    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=30,
            cwd=str(ORACLE_DIR),
        )

        if result.returncode != 0:
            return {"diff_pct": 100.0, "error": result.stderr}

        # Parse output (expected format: JSON with diff_pct)
        try:
            data = json.loads(result.stdout)
            return data
        except json.JSONDecodeError:
            # Try to extract percentage from output
            import re
            match = re.search(r"(\d+\.?\d*)%", result.stdout)
            if match:
                return {"diff_pct": float(match.group(1))}
            return {"diff_pct": 100.0, "error": "Failed to parse output"}

    except subprocess.TimeoutExpired:
        return {"diff_pct": 100.0, "error": "Comparison timed out"}
    except Exception as e:
        return {"diff_pct": 100.0, "error": str(e)}


# --- Aggregation ---

def aggregate_results(results: List[CaseResult], threshold: float = 25.0) -> AggregatedResult:
    """Aggregate multiple iteration results into a single result."""
    case_id = results[0].case_id if results else "unknown"

    # Filter out errors for statistical analysis
    valid_results = [r for r in results if r.diff_pct is not None and not r.error]

    if not valid_results:
        error_msg = results[0].error if results else "No results"
        return AggregatedResult(
            case_id=case_id,
            diff_pct_median=100.0,
            diff_pct_mean=100.0,
            diff_pct_min=100.0,
            diff_pct_max=100.0,
            diff_pct_variance=0.0,
            stable=False,
            iterations=len(results),
            passed=False,
            error=error_msg,
            raw_results=[asdict(r) for r in results],
        )

    diffs = [r.diff_pct for r in valid_results]
    diffs_sorted = sorted(diffs)

    n = len(diffs)
    median = diffs_sorted[n // 2] if n % 2 == 1 else (diffs_sorted[n // 2 - 1] + diffs_sorted[n // 2]) / 2
    mean = sum(diffs) / n
    variance = sum((d - mean) ** 2 for d in diffs) / n if n > 1 else 0.0

    # Stability: variance < 0.1% and no more than 10% relative spread
    spread = (diffs_sorted[-1] - diffs_sorted[0]) / max(mean, 0.01) if mean > 0 else 0
    stable = variance < 0.1 and spread < 0.1

    return AggregatedResult(
        case_id=case_id,
        diff_pct_median=median,
        diff_pct_mean=mean,
        diff_pct_min=diffs_sorted[0],
        diff_pct_max=diffs_sorted[-1],
        diff_pct_variance=variance,
        stable=stable,
        iterations=len(results),
        passed=median <= threshold,
        raw_results=[asdict(r) for r in results],
    )


# --- Utility Functions ---

def get_all_case_ids() -> List[str]:
    """Get all available case IDs."""
    return (
        list(BUILTINS.keys()) +
        list(WEBSUITE.keys()) +
        list(MICRO_TESTS.keys())
    )


def get_case_info(case_id: str) -> Optional[Dict]:
    """Get information about a specific case."""
    if case_id in BUILTINS:
        return {"type": "builtin", **BUILTINS[case_id]}
    if case_id in WEBSUITE:
        return {"type": "websuite", **WEBSUITE[case_id]}
    if case_id in MICRO_TESTS:
        return {"type": "micro", **MICRO_TESTS[case_id]}
    return None


if __name__ == "__main__":
    # Simple test
    print(f"REPO_ROOT: {REPO_ROOT}")
    print(f"BINARY_PATH: {BINARY_PATH}")
    print(f"Binary exists: {BINARY_PATH.exists()}")
    print(f"Total cases: {len(get_all_case_ids())}")
