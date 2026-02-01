#!/bin/bash
# External Timing Benchmark Script
# Compares builtin timing vs external timing for accuracy verification
#
# Usage: ./bench/run_external_timing.sh

set -e

ITERATIONS=5
REPORT_FILE="bench/EXTERNAL_TIMING_REPORT.md"

echo "# External Timing Benchmark Report" > "$REPORT_FILE"
echo "" >> "$REPORT_FILE"
echo "**Date**: $(date '+%Y-%m-%d %H:%M:%S')" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"
echo "This report compares builtin timing (printed by each benchmark) vs external timing (measured by shell)." >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"

# Build Pleno once
echo "Building Pleno in release mode..."
cargo build --release --bin pleno --quiet

PLENO_BIN="./target/release/pleno"

declare -a TESTS=("fib" "loop" "array")
declare -a TEST_NAMES=("Recursive Fib (30)" "Loop Sum (10M)" "Array Ops (1M)")

for i in "${!TESTS[@]}"; do
    test_id="${TESTS[$i]}"
    test_name="${TEST_NAMES[$i]}"
    src_file="bench/src/pleno/${test_id}.pln"
    
    if [ ! -f "$src_file" ]; then
        echo "Skipping $test_name: $src_file not found"
        continue
    fi
    
    echo ""
    echo "=== $test_name ==="
    echo "### $test_name" >> "$REPORT_FILE"
    echo "" >> "$REPORT_FILE"
    echo "| Run | External Time (s) | Builtin Time (ms) |" >> "$REPORT_FILE"
    echo "|-----|-------------------|-------------------|" >> "$REPORT_FILE"
    
    total_external=0
    total_builtin=0
    
    for run in $(seq 1 $ITERATIONS); do
        # External timing using time command
        start=$(date +%s.%N)
        output=$("$PLENO_BIN" run "$src_file" 2>&1)
        end=$(date +%s.%N)
        
        external_time=$(echo "$end - $start" | bc)
        
        # Extract builtin time from output
        builtin_time=$(echo "$output" | grep -oP 'Time:\s*\K[\d.]+' || echo "N/A")
        
        echo "Run $run: External=${external_time}s, Builtin=${builtin_time}ms"
        echo "| $run | $external_time | $builtin_time |" >> "$REPORT_FILE"
        
        if [ "$builtin_time" != "N/A" ]; then
            total_builtin=$(echo "$total_builtin + $builtin_time" | bc)
        fi
        total_external=$(echo "$total_external + $external_time" | bc)
    done
    
    avg_external=$(echo "scale=4; $total_external / $ITERATIONS" | bc)
    avg_builtin=$(echo "scale=2; $total_builtin / $ITERATIONS" | bc)
    
    echo "" >> "$REPORT_FILE"
    echo "**Average External**: ${avg_external}s" >> "$REPORT_FILE"
    echo "**Average Builtin**: ${avg_builtin}ms" >> "$REPORT_FILE"
    echo "" >> "$REPORT_FILE"
    
    echo "Average: External=${avg_external}s, Builtin=${avg_builtin}ms"
done

echo ""
echo "Report generated: $REPORT_FILE"
