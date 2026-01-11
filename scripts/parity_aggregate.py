#!/usr/bin/env python3
"""
parity_aggregate.py - Aggregate parity results from multiple runs

Combines results from swarm runs into a global report with fix scoreboard.

Platform: Linux (ported from macOS)

Usage:
    python scripts/parity_aggregate.py
    python scripts/parity_aggregate.py --input-dir parity-results --output report.json
    python scripts/parity_aggregate.py --compare previous_report.json
"""

import argparse
import json
import sys
from collections import defaultdict
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Optional, Any

# Repository root
REPO_ROOT = Path(__file__).parent.parent.resolve()
RESULTS_DIR = REPO_ROOT / "parity-results"


def find_swarm_reports(input_dir: Path) -> List[Path]:
    """Find all swarm_report.json files in the input directory."""
    reports = []
    for path in input_dir.rglob("swarm_report.json"):
        reports.append(path)
    return sorted(reports, key=lambda p: p.stat().st_mtime, reverse=True)


def load_report(path: Path) -> Optional[Dict]:
    """Load a JSON report file."""
    try:
        with open(path) as f:
            return json.load(f)
    except (json.JSONDecodeError, OSError) as e:
        print(f"Warning: Failed to load {path}: {e}")
        return None


def merge_results(reports: List[Dict]) -> Dict[str, List[Dict]]:
    """Merge results from multiple reports by case_id."""
    merged = defaultdict(list)
    for report in reports:
        for result in report.get("results", []):
            case_id = result.get("case_id")
            if case_id:
                merged[case_id].append(result)
    return dict(merged)


def compute_best_result(results: List[Dict]) -> Dict:
    """Compute the best result across multiple runs for a case."""
    if not results:
        return {"error": "No results"}

    # Filter valid results
    valid = [r for r in results if r.get("diff_pct_median") is not None]
    if not valid:
        return results[0]  # Return first even if error

    # Find best (lowest diff)
    best = min(valid, key=lambda r: r.get("diff_pct_median", 100.0))
    return best


def generate_fix_scoreboard(results: Dict[str, Dict], threshold: float = 25.0) -> List[Dict]:
    """Generate a scoreboard of cases that need fixing, sorted by impact."""
    scoreboard = []

    for case_id, result in results.items():
        diff = result.get("diff_pct_median", 100.0)
        if diff > threshold:
            # Calculate fix impact: how much would parity improve if fixed
            fix_impact = diff - threshold

            scoreboard.append({
                "case_id": case_id,
                "current_diff": diff,
                "fix_impact": fix_impact,
                "error": result.get("error"),
                "stable": result.get("stable", False),
            })

    # Sort by fix impact (highest first)
    return sorted(scoreboard, key=lambda x: -x["fix_impact"])


def compute_taxonomy_breakdown(results: Dict[str, Dict]) -> Dict[str, float]:
    """Compute breakdown of diff by taxonomy category."""
    taxonomy = defaultdict(float)

    for case_id, result in results.items():
        raw = result.get("raw_results", [])
        for r in raw:
            # Extract taxonomy if available
            tax = r.get("taxonomy", {})
            for category, pct in tax.items():
                taxonomy[category] += pct

    # Normalize
    total = sum(taxonomy.values()) or 1
    return {k: v / total * 100 for k, v in taxonomy.items()}


def aggregate_reports(
    input_dir: Path,
    output_path: Optional[Path] = None,
    compare_path: Optional[Path] = None,
    threshold: float = 25.0,
) -> Dict:
    """Aggregate all swarm reports into a global report."""
    print("=" * 60)
    print("Parity Aggregate - Linux")
    print("=" * 60)
    print(f"Input: {input_dir}")
    print()

    # Find all swarm reports
    report_paths = find_swarm_reports(input_dir)
    print(f"Found {len(report_paths)} swarm report(s)")

    if not report_paths:
        print("No reports found!")
        return {"error": "No reports found"}

    # Load reports
    reports = []
    for path in report_paths:
        report = load_report(path)
        if report:
            reports.append(report)
            print(f"  Loaded: {path.parent.name}/{path.name}")

    if not reports:
        print("No valid reports loaded!")
        return {"error": "No valid reports"}

    print()

    # Merge results by case
    merged = merge_results(reports)
    print(f"Cases: {len(merged)}")

    # Compute best result for each case
    best_results = {case_id: compute_best_result(results) for case_id, results in merged.items()}

    # Calculate statistics
    diffs = [r.get("diff_pct_median", 100.0) for r in best_results.values() if r.get("diff_pct_median") is not None]
    passed = sum(1 for d in diffs if d <= threshold)
    failed = len(diffs) - passed
    avg_diff = sum(diffs) / len(diffs) if diffs else 100.0
    visual_parity = 100.0 - avg_diff

    # Generate fix scoreboard
    scoreboard = generate_fix_scoreboard(best_results, threshold=threshold)

    # Compute taxonomy
    taxonomy = compute_taxonomy_breakdown(best_results)

    # Build global report
    global_report = {
        "timestamp": datetime.now().isoformat(),
        "platform": "linux",
        "input_dir": str(input_dir),
        "report_count": len(reports),
        "threshold": threshold,
        "summary": {
            "total_cases": len(best_results),
            "passed": passed,
            "failed": failed,
            "pass_rate": passed / len(best_results) * 100 if best_results else 0,
            "avg_diff_pct": avg_diff,
            "visual_parity": visual_parity,
        },
        "results": best_results,
        "fix_scoreboard": scoreboard[:20],  # Top 20
        "taxonomy": taxonomy,
    }

    # Compare with previous if provided
    if compare_path and compare_path.exists():
        previous = load_report(compare_path)
        if previous:
            regressions = detect_regressions(best_results, previous.get("results", {}), threshold=1.0)
            improvements = detect_improvements(best_results, previous.get("results", {}), threshold=1.0)
            global_report["comparison"] = {
                "previous_report": str(compare_path),
                "regressions": regressions,
                "improvements": improvements,
            }

    # Save report
    if output_path:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        with open(output_path, "w") as f:
            json.dump(global_report, f, indent=2, default=str)
        print(f"\nReport saved to: {output_path}")

    # Print summary
    print()
    print("=" * 60)
    print("Summary")
    print("=" * 60)
    print(f"Visual Parity: {visual_parity:.1f}%")
    print(f"Passed: {passed}/{len(best_results)} ({passed / len(best_results) * 100:.1f}%)" if best_results else "No results")
    print(f"Average Diff: {avg_diff:.2f}%")

    # Print top fixes needed
    if scoreboard:
        print()
        print("Top Fixes Needed:")
        for item in scoreboard[:10]:
            print(f"  {item['case_id']}: {item['current_diff']:.1f}% (impact: +{item['fix_impact']:.1f}%)")

    return global_report


def detect_regressions(
    current: Dict[str, Dict],
    previous: Dict[str, Dict],
    threshold: float = 1.0,
) -> List[Dict]:
    """Detect cases that regressed compared to previous run."""
    regressions = []

    for case_id, curr_result in current.items():
        prev_result = previous.get(case_id)
        if not prev_result:
            continue

        curr_diff = curr_result.get("diff_pct_median", 100.0)
        prev_diff = prev_result.get("diff_pct_median", 100.0)
        delta = curr_diff - prev_diff

        if delta > threshold:
            regressions.append({
                "case_id": case_id,
                "previous_diff": prev_diff,
                "current_diff": curr_diff,
                "delta": delta,
            })

    return sorted(regressions, key=lambda x: -x["delta"])


def detect_improvements(
    current: Dict[str, Dict],
    previous: Dict[str, Dict],
    threshold: float = 1.0,
) -> List[Dict]:
    """Detect cases that improved compared to previous run."""
    improvements = []

    for case_id, curr_result in current.items():
        prev_result = previous.get(case_id)
        if not prev_result:
            continue

        curr_diff = curr_result.get("diff_pct_median", 100.0)
        prev_diff = prev_result.get("diff_pct_median", 100.0)
        delta = prev_diff - curr_diff  # Positive means improvement

        if delta > threshold:
            improvements.append({
                "case_id": case_id,
                "previous_diff": prev_diff,
                "current_diff": curr_diff,
                "improvement": delta,
            })

    return sorted(improvements, key=lambda x: -x["improvement"])


def main():
    parser = argparse.ArgumentParser(
        description="Aggregate parity results from multiple runs"
    )
    parser.add_argument(
        "--input-dir",
        type=Path,
        default=RESULTS_DIR,
        help=f"Directory containing swarm reports (default: {RESULTS_DIR})",
    )
    parser.add_argument(
        "--output", "-o",
        type=Path,
        help="Output path for aggregated report",
    )
    parser.add_argument(
        "--compare",
        type=Path,
        help="Previous report to compare against for regression detection",
    )
    parser.add_argument(
        "--threshold",
        type=float,
        default=25.0,
        help="Pass threshold percentage (default: 25.0)",
    )

    args = parser.parse_args()

    # Default output path
    output_path = args.output or (args.input_dir / "global_report.json")

    report = aggregate_reports(
        input_dir=args.input_dir,
        output_path=output_path,
        compare_path=args.compare,
        threshold=args.threshold,
    )

    # Exit with error code if overall parity is low
    visual_parity = report.get("summary", {}).get("visual_parity", 0)
    if visual_parity < 50:
        sys.exit(1)


if __name__ == "__main__":
    main()
