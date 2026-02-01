# Truthful Benchmarking Suite

This directory contains a transparent, multi-language benchmarking suite for the Pleno programming language.

## Design Philosophy

Our goal is not just to show where Pleno is fast, but to honestly measure its performance characteristics against established languages.

**Principles:**

1.  **Direct Comparison**: Equivalent algorithms implemented idiomatically in each language.
2.  **Transparency**: Start times and end times are measured internally or via system tools. (`hyperfine` could be used)
3.  **Correctness**: Output is verified to ensure all implementations compute the same result.
4.  **Isolation**: Tests are run in a controlled environment (Release builds for compiled languages).

## Languages Compared (10 Total)

| Language       | Source Dir    | Execution                  |
| -------------- | ------------- | -------------------------- |
| **Pleno**      | `src/pleno/`  | VM execution (release)     |
| **Python**     | `src/python/` | Interpreted                |
| **Node.js**    | `src/node/`   | JIT compiled               |
| **TypeScript** | `src/ts/`     | Transpiled to JS, then JIT |
| **Go**         | `src/go/`     | `go run` (compiled)        |
| **Rust**       | `src/rust/`   | Compiled (`rustc -O`)      |
| **C**          | `src/c/`      | Compiled (`gcc -O3`)       |
| **C++**        | `src/cpp/`    | Compiled (`g++ -O3`)       |
| **Java**       | `src/java/`   | JVM (compiled)             |
| **Kotlin**     | `src/kotlin/` | JVM (compiled)             |

## Benchmarks

### 1. Recursive Fibonacci (30)

Measures raw function call overhead, recursion, and integer arithmetic.

### 2. Loop Sum (10M)

Measures tight loop performance with 10 million iterations.

### 3. Array Ops (1M)

Measures array creation, mapping, filtering, and reduction with 1 million elements.

## Running Benchmarks

```bash
# Run full comparison across all 10 languages (5 iterations each)
python3 bench/run_comparisons.py

# Run external timing verification (shell time vs builtin time)
bash bench/run_external_timing.sh
```

## Reports

- `REPORT.md` - Full benchmark report with rankings
- `EXTERNAL_TIMING_REPORT.md` - Builtin vs external timing comparison

## Technical Analysis: Why is Pleno so fast?

1. **High-Efficiency VM**: The Pleno VM uses a register-based architecture optimized for cache locality.
2. **Low Opcode Overhead**: Instructions are lean with minimal branch mispredictions.
3. **Efficient Collections**: Array and Map operations map directly to optimized Rust implementations.
4. **Real Execution**: No lazy evaluation, no bounds checking overhead for trusted code.
