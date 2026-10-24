#!/usr/bin/env python3
"""
Benchmark regression check for Pleno.

Runs each benchmark with two builds of `pleno` on the same machine (the base
branch and the change under test) and fails if the change is slower than the
base by more than the threshold. Comparing on the same machine avoids the
noise of stored baselines measured on different hardware.

Usage:
    python3 bench/regression_test.py --base path/to/base/pleno --head path/to/pleno
        [--runs 5] [--threshold 0.15]
"""

import argparse
import os
import subprocess
import sys
import time

BENCH_DIR = os.path.dirname(os.path.abspath(__file__))
CASES_DIR = os.path.join(BENCH_DIR, "cases")

BENCHMARKS = ["fib", "loop", "string", "collection"]


def run_once(pleno, case):
    """Runs one benchmark and returns its wall time in seconds."""
    path = os.path.join(CASES_DIR, f"{case}.pln")
    start = time.perf_counter()
    result = subprocess.run([pleno, "run", path], capture_output=True, text=True)
    elapsed = time.perf_counter() - start
    if result.returncode != 0:
        sys.exit(f"{case} failed with {pleno}:\n{result.stdout}{result.stderr}")
    return elapsed


def best_time(pleno, case, runs):
    """The fastest of several runs: noise from other processes only adds time."""
    run_once(pleno, case)  # warm-up: file cache, page faults
    return min(run_once(pleno, case) for _ in range(runs))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--base", required=True, help="pleno built from the base branch")
    parser.add_argument("--head", required=True, help="pleno built from the change")
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--threshold", type=float, default=0.15,
                        help="allowed slowdown, as a fraction (0.15 = 15%%)")
    args = parser.parse_args()
    args.base, args.head = os.path.abspath(args.base), os.path.abspath(args.head)

    regressions = []
    print(f"{'benchmark':<12}{'base':>10}{'head':>10}{'change':>10}")
    for case in BENCHMARKS:
        base = best_time(args.base, case, args.runs)
        head = best_time(args.head, case, args.runs)
        change = (head - base) / base
        marker = "  REGRESSION" if change > args.threshold else ""
        print(f"{case:<12}{base:>9.3f}s{head:>9.3f}s{change * 100:>+9.1f}%{marker}")
        if change > args.threshold:
            regressions.append(case)

    if regressions:
        print(f"\n{len(regressions)} benchmark(s) slower than the base by more than "
              f"{args.threshold:.0%}: {', '.join(regressions)}")
        sys.exit(1)
    print(f"\nAll benchmarks within {args.threshold:.0%} of the base.")


if __name__ == "__main__":
    main()
