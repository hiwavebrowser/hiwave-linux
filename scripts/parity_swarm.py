#!/usr/bin/env python3
"""
parity_swarm.py - Parallel/sharded parity test execution for Linux

Runs parity tests in parallel using multiprocessing with sharding support
for CI environments. Uses Xvfb for headless display.

Platform: Linux (ported from macOS)

Usage:
    python scripts/parity_swarm.py --jobs 4 --scope all
    python scripts/parity_swarm.py --shard-index 0 --shard-count 4
    python scripts/parity_swarm.py --cases new_tab,about --iterations 5
"""

import argparse
import json
import multiprocessing as mp
import os
import sys
import time
from dataclasses import asdict
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Optional, Tuple

# Add parent to path for imports
sys.path.insert(0, str(Path(__file__).parent))

from parity_lib import (
    REPO_ROOT,
    RESULTS_DIR,
    WorkUnit,
    CaseResult,
    AggregatedResult,
    create_work_units,
    execute_work_unit,
    aggregate_results,
    ensure_display,
)


def worker_init():
    """Initialize worker process with display."""
    ensure_display()


def worker_execute(args: Tuple[WorkUnit, Path, bool]) -> Tuple[str, List[CaseResult]]:
    """Worker function to execute a single work unit."""
    unit, output_dir, verbose = args
    try:
        results = execute_work_unit(unit, output_dir, verbose=verbose)
        return (unit.case_id, results)
    except Exception as e:
        return (unit.case_id, [CaseResult(
            case_id=unit.case_id,
            iteration=0,
            error=str(e),
        )])


def shard_work_units(
    units: List[WorkUnit],
    shard_index: int,
    shard_count: int,
) -> List[WorkUnit]:
    """Shard work units for distributed execution."""
    if shard_count <= 1:
        return units

    # Simple modulo-based sharding
    return [u for i, u in enumerate(units) if i % shard_count == shard_index]


def run_swarm(
    scope: str = "all",
    jobs: int = 4,
    iterations: int = 3,
    shard_index: int = 0,
    shard_count: int = 1,
    case_filter: Optional[List[str]] = None,
    verbose: bool = False,
    run_id: Optional[str] = None,
    threshold: float = 25.0,
) -> Dict:
    """Run parity tests in parallel."""
    # Create output directory
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    run_id = run_id or f"swarm_{timestamp}"
    output_dir = RESULTS_DIR / run_id
    output_dir.mkdir(parents=True, exist_ok=True)

    print("=" * 60)
    print("Parity Swarm - Linux")
    print("=" * 60)
    print(f"Run ID: {run_id}")
    print(f"Scope: {scope}")
    print(f"Jobs: {jobs}")
    print(f"Iterations: {iterations}")
    if shard_count > 1:
        print(f"Shard: {shard_index + 1}/{shard_count}")
    print(f"Output: {output_dir}")
    print()

    # Ensure DISPLAY is set
    ensure_display()
    print(f"DISPLAY: {os.environ.get('DISPLAY', 'not set')}")

    # Create work units
    all_units = create_work_units(scope=scope, iterations=iterations, case_filter=case_filter)

    if not all_units:
        print("No work units to process!")
        return {"error": "No work units"}

    # Apply sharding
    units = shard_work_units(all_units, shard_index, shard_count)
    print(f"Work units: {len(units)} (of {len(all_units)} total)")
    print()

    # Prepare work items
    work_items = [(u, output_dir, verbose) for u in units]

    # Execute in parallel
    start_time = time.time()
    all_results: Dict[str, List[CaseResult]] = {}

    print("Running tests...")
    print("-" * 60)

    if jobs == 1:
        # Sequential execution
        for item in work_items:
            case_id, results = worker_execute(item)
            all_results[case_id] = results
            agg = aggregate_results(results, threshold=threshold)
            status = "+" if agg.passed else "x"
            print(f"  {status} {case_id}: {agg.diff_pct_median:.2f}% (median)")
    else:
        # Parallel execution using fork (default on Linux)
        with mp.Pool(processes=jobs, initializer=worker_init) as pool:
            for case_id, results in pool.imap_unordered(worker_execute, work_items):
                all_results[case_id] = results
                agg = aggregate_results(results, threshold=threshold)
                status = "+" if agg.passed else "x"
                print(f"  {status} {case_id}: {agg.diff_pct_median:.2f}% (median)")

    elapsed = time.time() - start_time
    print("-" * 60)
    print(f"Completed in {elapsed:.1f}s")
    print()

    # Aggregate all results
    aggregated: Dict[str, AggregatedResult] = {}
    for case_id, results in all_results.items():
        aggregated[case_id] = aggregate_results(results, threshold=threshold)

    # Calculate summary statistics
    passed = sum(1 for a in aggregated.values() if a.passed)
    failed = len(aggregated) - passed
    avg_diff = sum(a.diff_pct_median for a in aggregated.values()) / len(aggregated) if aggregated else 100.0
    stable_count = sum(1 for a in aggregated.values() if a.stable)

    # Build report
    report = {
        "run_id": run_id,
        "timestamp": datetime.now().isoformat(),
        "platform": "linux",
        "scope": scope,
        "shard_index": shard_index,
        "shard_count": shard_count,
        "iterations": iterations,
        "threshold": threshold,
        "elapsed_seconds": elapsed,
        "summary": {
            "total": len(aggregated),
            "passed": passed,
            "failed": failed,
            "stable": stable_count,
            "avg_diff_pct": avg_diff,
            "pass_rate": passed / len(aggregated) * 100 if aggregated else 0,
        },
        "results": [asdict(a) for a in aggregated.values()],
    }

    # Save report
    report_path = output_dir / "swarm_report.json"
    with open(report_path, "w") as f:
        json.dump(report, f, indent=2, default=str)

    # Print summary
    print("=" * 60)
    print("Summary")
    print("=" * 60)
    print(f"Total: {len(aggregated)}")
    print(f"Passed: {passed} ({passed / len(aggregated) * 100:.1f}%)" if aggregated else "Passed: 0")
    print(f"Failed: {failed}")
    print(f"Stable: {stable_count}")
    print(f"Average Diff: {avg_diff:.2f}%")
    print()
    print(f"Report saved to: {report_path}")

    # List failures
    failures = [a for a in aggregated.values() if not a.passed]
    if failures:
        print()
        print("Failed cases:")
        for f in sorted(failures, key=lambda x: -x.diff_pct_median):
            print(f"  x {f.case_id}: {f.diff_pct_median:.2f}%")
            if f.error:
                print(f"      Error: {f.error}")

    return report


def main():
    parser = argparse.ArgumentParser(
        description="Parallel parity test execution for Linux"
    )
    parser.add_argument(
        "--scope",
        choices=["all", "builtins", "websuite", "micro"],
        default="all",
        help="Test scope (default: all)",
    )
    parser.add_argument(
        "--jobs", "-j",
        type=int,
        default=4,
        help="Number of parallel jobs (default: 4)",
    )
    parser.add_argument(
        "--iterations", "-n",
        type=int,
        default=3,
        help="Iterations per case (default: 3)",
    )
    parser.add_argument(
        "--shard-index",
        type=int,
        default=0,
        help="Shard index for distributed runs (default: 0)",
    )
    parser.add_argument(
        "--shard-count",
        type=int,
        default=1,
        help="Total shard count (default: 1)",
    )
    parser.add_argument(
        "--cases",
        type=str,
        help="Comma-separated list of specific cases to run",
    )
    parser.add_argument(
        "--run-id",
        type=str,
        help="Custom run identifier",
    )
    parser.add_argument(
        "--threshold",
        type=float,
        default=25.0,
        help="Pass threshold percentage (default: 25.0)",
    )
    parser.add_argument(
        "--verbose", "-v",
        action="store_true",
        help="Verbose output",
    )

    args = parser.parse_args()

    case_filter = args.cases.split(",") if args.cases else None

    report = run_swarm(
        scope=args.scope,
        jobs=args.jobs,
        iterations=args.iterations,
        shard_index=args.shard_index,
        shard_count=args.shard_count,
        case_filter=case_filter,
        verbose=args.verbose,
        run_id=args.run_id,
        threshold=args.threshold,
    )

    # Exit with error code if any failures
    if report.get("summary", {}).get("failed", 0) > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()
