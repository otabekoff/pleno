# Truthful Benchmarking Suite

This directory contains a transparent, multi-language benchmarking suite for the Pleno programming language.

## Design Philosophy

Our goal is not just to show where Pleno is fast, but to honestly measure its performance characteristics against established languages.

**Principles:**
1.  **Direct Comparison**: Equivalent algorithms implemented idiomatically in each language.
2.  **Transparency**: Start times and end times are measured internally or via system tools (`hyperfine`).
3.  **Correctness**: Output is verified to ensure all implementations compute the same result.
4.  **Isolation**: Tests are run in a controlled environment (Release builds for compiled languages).

## Languages Compared

| Language | Run Command (approx) |
|----------|----------------------|
| **Pleno** | `cargo run --release --bin pleno -- bench/src/pleno/fib.pln` |
| **Rust** | `cargo run --release --bin fib` |
| **Python** | `python3 bench/src/python/fib.py` |
| **Go** | `go run bench/src/go/fib.go` |
| **Node.js**| `node bench/src/node/fib.js` |

## Benchmarks

### 1. Fibonacci (Recursion)
Measures raw function call overhead and integer arithmetic.
- Code: `fib(30)` recursive.

### 2. Map/Reduce (Collection Processing)
Measures iteration and closure/lambda performance.
- Code: Create array [1..300,000], map `*2`, filter `%3`, reduce sum.

## Running Benchmarks

```bash
# Run the automated runner
python3 bench/run_comparisons.py
```
