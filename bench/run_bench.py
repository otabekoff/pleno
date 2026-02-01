import subprocess
import time
import os

BENCH_DIR = os.path.dirname(os.path.abspath(__file__))
CASES_DIR = os.path.join(BENCH_DIR, "cases")
PLENO_BIN = os.path.join(BENCH_DIR, "..", "target", "release", "pleno")

def run_command(cmd):
    start = time.perf_counter()
    result = subprocess.run(cmd, capture_output=True, text=True)
    end = time.perf_counter()
    return end - start, result.stdout.strip()

def main():
    print("-" * 50)
    print("Running Benchmarks (Recursive Fibonacci 30)...")
    print("-" * 50)
    
    pleno_time, pleno_out = run_command([PLENO_BIN, os.path.join(CASES_DIR, "fib.pln")])
    py_time, py_out = run_command(["python3", os.path.join(CASES_DIR, "fib.py")])
    node_time, node_out = run_command(["node", os.path.join(CASES_DIR, "fib.js")])
    
    print(f"Pleno: {pleno_time:.4f}s")
    print(f"Python: {py_time:.4f}s")
    print(f"Node.js: {node_time:.4f}s")
    print(f"Pleno vs Python: {py_time/pleno_time:.2f}x faster")
    print(f"Pleno vs Node.js: {node_time/pleno_time:.2f}x faster")

    print("-" * 50)
    print("Running Benchmarks (Million Iterations Loop)...")
    print("-" * 50)
    
    pleno_time, pleno_out = run_command([PLENO_BIN, os.path.join(CASES_DIR, "loop.pln")])
    py_time, py_out = run_command(["python3", os.path.join(CASES_DIR, "loop.py")])
    node_time, node_out = run_command(["node", os.path.join(CASES_DIR, "loop.js")])
    
    print(f"Pleno: {pleno_time:.4f}s")
    print(f"Python: {py_time:.4f}s")
    print(f"Node.js: {node_time:.4f}s")
    print(f"Pleno vs Python: {py_time/pleno_time:.2f}x faster")
    print(f"Pleno vs Node.js: {node_time/pleno_time:.2f}x faster")

if __name__ == "__main__":
    main()
