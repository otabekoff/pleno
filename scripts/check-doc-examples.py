#!/usr/bin/env python3
"""Runs every ```pleno code block in the docs and reports the ones that fail.

Usage: python3 scripts/check-doc-examples.py [path/to/pleno] [docs files or folders...]

Each block runs on its own, in a temporary folder, so it must include its
imports. Mark a block that cannot run by itself (a fragment, or one that needs
a server or other files) as ```pleno norun.
"""
import os
import re
import shutil
import subprocess
import sys
import tempfile

FENCE = re.compile(r"^```(pleno|pln)\b([^\n]*)\n(.*?)^```", re.M | re.S)


def markdown_files(paths):
    for path in paths:
        if os.path.isfile(path):
            yield path
            continue
        for root, dirs, files in os.walk(path):
            dirs[:] = [d for d in dirs if d not in (".vitepress", "node_modules")]
            for name in sorted(files):
                if name.endswith(".md"):
                    yield os.path.join(root, name)


def main():
    args = sys.argv[1:]
    pleno = args.pop(0) if args and not args[0].endswith(".md") and not os.path.isdir(args[0]) else "pleno"
    paths = args or ["docs"]
    # Absolute, because each example runs in its own temporary folder.
    pleno = os.path.abspath(shutil.which(pleno) or pleno)

    passed = failed = skipped = 0
    for path in markdown_files(paths):
        text = open(path, encoding="utf-8").read()
        for match in FENCE.finditer(text):
            if "norun" in match.group(2).split():
                skipped += 1
                continue
            line = text[: match.start()].count("\n") + 1
            with tempfile.TemporaryDirectory() as folder:
                source = os.path.join(folder, "example.pln")
                with open(source, "w", encoding="utf-8") as f:
                    f.write(match.group(3))
                try:
                    result = subprocess.run(
                        [pleno, "run", source],
                        cwd=folder,
                        capture_output=True,
                        text=True,
                        encoding="utf-8",
                        errors="replace",
                        timeout=20,
                        env={**os.environ, "NO_COLOR": "1"},
                    )
                    ok = result.returncode == 0
                    lines = (result.stderr.strip() or result.stdout.strip()).splitlines()
                    message = lines[0] if lines else f"exit code {result.returncode}"
                except subprocess.TimeoutExpired:
                    ok, message = False, "timed out after 20 seconds"
            if ok:
                passed += 1
            else:
                failed += 1
                print(f"{path}:{line}: {message}")

    print(f"{passed} examples ran, {failed} failed, {skipped} marked norun")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
