import subprocess
import time
import os
import sys

# Configuration
ITERATIONS = 5

# Languages and their configurations
LANGUAGES = {
    "Pleno": {
        "ext": "pln",
        "runner": ["cargo", "run", "--quiet", "--release", "--bin", "pleno", "--"],
        "build_once": ["cargo", "build", "--release", "--bin", "pleno"],
        "src_dir": "bench/src/pleno"
    },
    "Python": {
        "ext": "py",
        "runner": ["python3"],
        "src_dir": "bench/src/python"
    },
    "Node.js": {
        "ext": "js",
        "runner": ["node"],
        "src_dir": "bench/src/node"
    },
    "Go": {
        "ext": "go",
        "runner": ["go", "run"],
        "src_dir": "bench/src/go"
    },
    "Rust": {
        "ext": "rs",
        "compiler": ["rustc", "-O"], # -O for optimization
        "out_ext": "",
        "src_dir": "bench/src/rust"
    },
    "C": {
        "ext": "c",
        "compiler": ["gcc", "-O3"],
        "out_ext": "",
        "src_dir": "bench/src/c"
    },
    "C++": {
        "ext": "cpp",
        "compiler": ["g++", "-O3"],
        "out_ext": "",
        "src_dir": "bench/src/cpp"
    },
    "TypeScript": {
        "ext": "ts",
        "transpiler": ["npx", "-y", "-p", "typescript", "tsc", "--module", "esnext", "--target", "esnext", "--moduleResolution", "node"],
        "runner": ["node"],
        "src_dir": "bench/src/ts",
        # runs .js after transpiling
        "run_ext": "js"
    },
    "Java": {
        "ext": "java",
        "compiler": ["javac"],
        "runner": ["java", "-cp"], # needs cp set dynamically
        "src_dir": "bench/src/java"
    },
    "Kotlin": {
        "ext": "kt",
        "compiler": ["kotlinc", "-include-runtime", "-d"],
        "out_ext": ".jar",
        "runner": ["java", "-jar"],
        "src_dir": "bench/src/kotlin"
    }
}

TESTS = [
    {"id": "fib", "name": "Recursive Fib (30)"},
    {"id": "loop", "name": "Loop Sum (10M)"},
    {"id": "array", "name": "Array Ops (1M)"}
]

def build_and_get_cmd(lang_name, config, test_id):
    src_dir = config["src_dir"]
    ext = config["ext"]
    
    # Filenames
    # Standardize casing? Our files are: Loop.java/kt, Array.java/kt, Fib.java/kt. But lower case for others: fib.c, loop.c...
    # Let's handle casing: Java/Kotlin/TypeScript usually capitalized? No, TS files I made lowercase.
    # Java/Kotlin I made Capitalized (Fib.java, Loop.java).
    # C/C++/Rust/Go/Pleno/Py/Node/TS are lowercase.
    
    filename_base = test_id
    if lang_name in ["Java", "Kotlin"]:
        # Capitalize first letter: fib -> Fib
        filename_base = test_id.capitalize()
    
    src_file = os.path.join(src_dir, f"{filename_base}.{ext}")
    
    if not os.path.exists(src_file):
        print(f"  Warning: Source {src_file} not found. Skipping.")
        return None

    # 1. Compilation / Transpilation
    if "build_once" in config and test_id == "fib": # Only build once global
        subprocess.run(config["build_once"], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    
    if "compiler" in config:
        out_ext = config.get("out_ext", "")
        out_file = os.path.join(src_dir, f"{filename_base}{out_ext}")
        
        cmd = config["compiler"] + [src_file]
        if lang_name in ["Rust", "C", "C++"]:
             cmd += ["-o", out_file]
        elif lang_name == "Kotlin":
             # Kotlin: kotlinc src.kt -include-runtime -d out.jar
             # Config compiler has flags: ["kotlinc", "-include-runtime", "-d"]
             # We need to construct: kotlinc src.kt -include-runtime -d out.jar
             # So: ["kotlinc", src_file, "-include-runtime", "-d", out_file]
             cmd = ["kotlinc", src_file, "-include-runtime", "-d", out_file]
             
        try:
            subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        except Exception as e:
            print(f"  Build failed for {lang_name} {test_id}: {e}")
            return None
            
        if "runner" in config:
            # Java/Kotlin jar runner
            if lang_name == "Java":
                 # java -cp bench/src/java Fib
                 return config["runner"] + [src_dir, filename_base]
            else:
                 return config["runner"] + [out_file]
        else:
            # Native binary
            return [out_file]

    if "transpiler" in config: # TS
        try:
            subprocess.run(config["transpiler"] + [src_file], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        except:
            return None
        run_file = os.path.join(src_dir, f"{filename_base}.{config['run_ext']}")
        return config["runner"] + [run_file]

    # Interpreted / Go run
    return config["runner"] + [src_file]

def run_bench(cmd):
    total_time = 0
    count = 0
    for _ in range(ITERATIONS):
        try:
            proc = subprocess.run(cmd, capture_output=True, text=True, check=True)
            for line in proc.stdout.splitlines():
                if line.strip().startswith("Time:"):
                    # Parse "Time: 12.34ms" or "Time: 12.34 ms"
                    parts = line.split(":")
                    val_str = parts[1].strip().replace("ms", "")
                    total_time += float(val_str)
                    count += 1
        except Exception as e:
             # print(e)
             pass
    
    if count == 0: return None
    return total_time / count

def main():
    print(f"Starting Universal Benchmark Suite ({ITERATIONS} runs each)...")
    
    full_report = "# Universal Benchmark Report\n\n"
    full_report += f"**Date**: {time.strftime('%Y-%m-%d %H:%M:%S')}\n\n"

    for test in TESTS:
        test_id = test["id"]
        test_name = test["name"]
        print(f"\nBenchmark: {test_name}")
        print("="*40)
        
        results = []
        for lang, config in LANGUAGES.items():
            print(f"  Processing {lang}...", end="", flush=True)
            cmd = build_and_get_cmd(lang, config, test_id)
            if cmd:
                avg = run_bench(cmd)
                if avg is not None:
                    print(f" Done ({avg:.2f}ms)")
                    results.append((lang, avg))
                else:
                    print(" Failed (Runtime)")
            else:
                print(" Skipped")
        
        # Sort and Report
        results.sort(key=lambda x: x[1])
        if not results: continue

        baseline = results[0][1]
        
        table = f"\n### {test_name}\n"
        table += "| Rank | Language | Time (ms) | Relative |\n"
        table += "|------|----------|-----------|----------|\n"
        
        print("-" * 40)
        print(f"{'Lang':<10} | {'Time':<10} | {'Rel'}")
        
        for i, (name, ms) in enumerate(results):
            # Calculate relative. If baseline is 0, use 0.01 as denominator for safety or show N/A
            if baseline <= 0.001:
                rel = 1.0 if ms <= 0.001 else ms / 0.01 # Arbitrary base for "extremely fast"
            else:
                rel = ms / baseline
            
            row = f"| {i+1} | **{name}** | {ms:.2f} | {rel:.2f}x |"
            if baseline <= 0.001 and ms > 0.001:
                 # If it's 0.00ms, everything else looks infinitely slower. Let's just say >1000x
                 row = f"| {i+1} | **{name}** | {ms:.2f} | >1000x |"
            
            if name == "Pleno":
                row = f"| {i+1} | **Pleno** 🟢 | **{ms:.2f}** | **{rel:.2f}x** |"
                if baseline <= 0.001 and ms > 0.001:
                     row = f"| {i+1} | **Pleno** 🟢 | **{ms:.2f}** | **>1000x** |"
            
            print(f"{name:<10} | {ms:<10.2f} | {rel:.2f}x")
            
            table += row + "\n"
        
        full_report += table + "\n"

    with open("bench/REPORT.md", "w") as f:
        f.write(full_report)
    print("\nReport generated: bench/REPORT.md")

if __name__ == "__main__":
    main()
