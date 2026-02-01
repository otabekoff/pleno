#!/usr/bin/env python3
"""
Benchmark Regression Testing for Pleno

This script runs benchmarks and compares them against stored baselines.
If performance degrades by more than the threshold, it fails with an exit code.

Usage:
    python3 bench/regression_test.py                 # Run and compare
    python3 bench/regression_test.py --update        # Update baselines
"""

import subprocess
import time
import json
import os
import sys

BENCH_DIR = os.path.dirname(os.path.abspath(__file__))
CASES_DIR = os.path.join(BENCH_DIR, "cases")
BASELINE_FILE = os.path.join(BENCH_DIR, "baselines.json")
PLENO_BIN = os.path.join(BENCH_DIR, "..", "target", "release", "pleno")

# Maximum allowed regression percentage (10% = 0.10)
REGRESSION_THRESHOLD = 0.15

BENCHMARKS = [
    ("fib", "fib.pln"),
    ("loop", "loop.pln"),
    ("string", "string.pln"),
    ("collection", "collection.pln"),
]

def run_benchmark(pln_file):
    """Run a Pleno benchmark and return execution time in seconds."""
    cmd = [PLENO_BIN, os.path.join(CASES_DIR, pln_file)]
    start = time.perf_counter()
    result = subprocess.run(cmd, capture_output=True, text=True)
    end = time.perf_counter()
    return end - start

def load_baselines():
    """Load baseline times from file."""
    if os.path.exists(BASELINE_FILE):
        with open(BASELINE_FILE, 'r') as f:
            return json.load(f)
    return {}

def save_baselines(baselines):
    """Save baseline times to file."""
    with open(BASELINE_FILE, 'w') as f:
        json.dump(baselines, f, indent=2)
    print(f"Baselines saved to {BASELINE_FILE}")

def main():
    update_mode = "--update" in sys.argv
    
    # Ensure release build exists
    if not os.path.exists(PLENO_BIN):
        print(f"Error: Release binary not found at {PLENO_BIN}")
        print("Run: cargo build --release")
        sys.exit(1)
    
    baselines = load_baselines()
    results = {}
    regressions = []
    
    print("=" * 60)
    print("Pleno Benchmark Regression Test")
    print("=" * 60)
    
    for name, pln_file in BENCHMARKS:
        pln_path = os.path.join(CASES_DIR, pln_file)
        if not os.path.exists(pln_path):
            print(f"  SKIP: {name} (file not found)")
            continue
            
        current_time = run_benchmark(pln_file)
        results[name] = current_time
        
        if name in baselines:
            baseline_time = baselines[name]
            change_pct = (current_time - baseline_time) / baseline_time
            status = "✓" if change_pct <= REGRESSION_THRESHOLD else "✗ REGRESSION"
            
            print(f"  {name}: {current_time:.4f}s (baseline: {baseline_time:.4f}s, {change_pct*100:+.1f}%) {status}")
            
            if change_pct > REGRESSION_THRESHOLD:
                regressions.append((name, baseline_time, current_time, change_pct))
        else:
            print(f"  {name}: {current_time:.4f}s (no baseline)")
    
    print("=" * 60)
    
    if update_mode:
        save_baselines(results)
        print("Baselines updated successfully!")
        sys.exit(0)
    
    if regressions:
        print(f"\n❌ {len(regressions)} regression(s) detected!")
        for name, baseline, current, pct in regressions:
            print(f"   - {name}: {baseline:.4f}s → {current:.4f}s ({pct*100:+.1f}%)")
        print("\nTo update baselines, run: python3 bench/regression_test.py --update")
        sys.exit(1)
    else:
        print("\n✓ All benchmarks within acceptable range")
        sys.exit(0)

if __name__ == "__main__":
    main()
