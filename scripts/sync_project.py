#!/usr/bin/env python3
"""Copies the version and license year from pleno.json into the files that repeat them.

Run by `make sync` (and so by `make build` and `make check`). The places it updates:

- the workspace version in Cargo.toml (every crate inherits it);
- the version of the VS Code extension in ide/vscode/package.json;
- the supported versions in SECURITY.md;
- the copyright years in LICENSE.

Package manifests (pleno.toml) are left alone: a package's version is its own.
"""
import json
import re
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent


def load_metadata():
    with open(BASE_DIR / "pleno.json", encoding="utf-8") as f:
        return json.load(f)


def update_file(path, pattern, replacement):
    full_path = BASE_DIR / path
    if not full_path.exists():
        print(f"Warning: {path} not found.")
        return

    # newline="" keeps each file's line endings as they are.
    with open(full_path, encoding="utf-8", newline="") as f:
        content = f.read()

    new_content = re.sub(pattern, replacement, content, flags=re.MULTILINE)

    if new_content != content:
        with open(full_path, "w", encoding="utf-8", newline="") as f:
            f.write(new_content)
        print(f"Updated {path}")
    else:
        print(f"No changes for {path}")


def main():
    metadata = load_metadata()
    version = metadata["version"]
    year = metadata["license_year"]
    series = ".".join(version.split(".")[:2])

    print(f"Syncing Pleno project metadata (version {version}, years {year})...")

    # 1. The workspace version, which every crate inherits.
    update_file(
        "Cargo.toml",
        r'(^\[workspace\.package\]\r?\n(?:.*\r?\n)*?version\s*=\s*")[^"]+(")',
        rf"\g<1>{version}\g<2>",
    )

    # 2. The extension's own version (the top-level key, indented by two spaces).
    update_file(
        "ide/vscode/package.json",
        r'^(  "version"\s*:\s*")[^"]+(")',
        rf"\g<1>{version}\g<2>",
    )

    # 3. SECURITY.md: the supported series (such as 0.3.x) and the versions before it.
    update_file("SECURITY.md", r"^(\| )[0-9]+\.[0-9]+\.x( )", rf"\g<1>{series}.x\g<2>")
    update_file("SECURITY.md", r"^(\| < )[0-9.]+( )", rf"\g<1>{series}.0\g<2>")

    # 4. LICENSE
    update_file(
        "LICENSE",
        r"(Copyright \(c\) )[0-9-]+( Pleno Contributors)",
        rf"\g<1>{year}\g<2>",
    )

    print("Synchronization complete.")


if __name__ == "__main__":
    main()
