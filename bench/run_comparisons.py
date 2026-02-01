import subprocess
import time
import os
import sys

# Configuration
ITERATIONS = 5
RESULTS = []

benchmarks = [
    {
        "name": "Pleno",
        "cmd": ["cargo", "run", "--quiet", "--release", "--bin", "pleno", "--", "bench/src/pleno/fib.pln"],
        "build_cmd": ["cargo", "build", "--release", "--bin", "pleno"]
    },
    {
        "name": "Rust",
        "cmd": ["./bench/src/rust/fib_rs"],
        "build_cmd": ["rustc", "-O", "bench/src/rust/fib.rs", "-o", "bench/src/rust/fib_rs"]
    },
    {
        "name": "C",
        "cmd": ["./bench/src/c/fib_c"],
        "build_cmd": ["gcc", "-O3", "bench/src/c/fib.c", "-o", "bench/src/c/fib_c"]
    },
    {
        "name": "C++",
        "cmd": ["./bench/src/cpp/fib_cpp"],
        "build_cmd": ["g++", "-O3", "bench/src/cpp/fib.cpp", "-o", "bench/src/cpp/fib_cpp"]
    },
    {
        "name": "Java",
        "cmd": ["java", "-cp", "bench/src/java", "Fib"],
        "build_cmd": ["javac", "bench/src/java/Fib.java"]
    },
    {
        "name": "Kotlin",
        "cmd": ["java", "-jar", "bench/src/kotlin/Fib.jar"],
        "build_cmd": ["kotlinc", "bench/src/kotlin/Fib.kt", "-include-runtime", "-d", "bench/src/kotlin/Fib.jar"]
    },
    {
        "name": "TypeScript",
        "cmd": ["node", "bench/src/ts/fib.js"],
        "build_cmd": ["npx", "-y", "-p", "typescript", "tsc", "bench/src/ts/fib.ts"]
    },
    {
        "name": "Python",
        "cmd": ["python3", "bench/src/python/fib.py"],
        "build_cmd": None
    },
    {
        "name": "Node.js",
        "cmd": ["node", "bench/src/node/fib.js"],
        "build_cmd": None
    },
    {
        "name": "Go",
        "cmd": ["go", "run", "bench/src/go/fib.go"],
        "build_cmd": None
    }
]

def run_bench(bench):
    print(f"Running {bench['name']}...")
    
    # Build if needed
    if bench["build_cmd"]:
        try:
            subprocess.run(bench["build_cmd"], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        except (subprocess.CalledProcessError, FileNotFoundError):
            print(f"  Skipping {bench['name']} (Build failed or tool missing)")
            return None

    total_time = 0
    count = 0
    
    for i in range(ITERATIONS):
        start = time.time()
        # Capture output to verify correctness (optional) but mostly we rely on internal time or external time?
        # Ideally we parse the output for "Time: Xms" if the benchmark prints it, 
        # OR we measure process time. 
        # The benchmarks print "Time: Xms". Let's parse that for internal precision.
        
        proc = subprocess.run(bench["cmd"], capture_output=True, text=True)
        if proc.returncode != 0:
            print(f"Error running {bench['name']}: {proc.stderr}")
            return None

        # Parse output line "Time: 123.45ms"
        for line in proc.stdout.splitlines():
            if line.startswith("Time:"):
                try:
                    t_str = line.split(":")[1].strip().replace("ms", "")
                    t = float(t_str)
                    total_time += t
                    count += 1
                except ValueError:
                    pass
    
    if count == 0:
        return 0
    return total_time / count

def main():
    print(f"Starting Comparisons ({ITERATIONS} runs each)...")
    print("-" * 40)
    
    results = []
    
    for b in benchmarks:
        avg_ms = run_bench(b)
        if avg_ms is not None:
             results.append((b["name"], avg_ms))
    
    # Sort by time
    results.sort(key=lambda x: x[1])
    
    print("\n" + "="*40)
    print("FINAL RESULTS (Lower is Better)")
    print("="*40)
    print(f"{'Language':<15} | {'Avg Time (ms)':<15} | {'Relative'}")
    print("-" * 45)
    
    baseline = results[0][1]
    
    for name, ms in results:
        relative = ms / baseline
        print(f"{name:<15} | {ms:<15.2f} | {relative:.2f}x")
    
    # Generate Markdown Report
    with open("bench/REPORT.md", "w") as f:
        f.write("# Benchmark Report\n\n")
        f.write(f"**Date**: {time.strftime('%Y-%m-%d %H:%M:%S')}\n")
        f.write(f"**Iterations**: {ITERATIONS}\n\n")
        f.write("| Language | Avg Time (ms) | Relative |\n")
        f.write("|----------|---------------|----------|\n")
        for name, ms in results:
            relative = ms / baseline
            f.write(f"| **{name}** | {ms:.2f} | {relative:.2f}x |\n")

if __name__ == "__main__":
    main()
