#!/usr/bin/env python3
import json
import re
import os
from pathlib import Path

# Base directory
BASE_DIR = Path(__file__).parent.parent.absolute()

def load_metadata():
    with open(BASE_DIR / "pleno.json", "r") as f:
        return json.load(f)

def update_file(path, pattern, replacement):
    full_path = BASE_DIR / path
    if not full_path.exists():
        print(f"Warning: {path} not found.")
        return

    with open(full_path, "r") as f:
        content = f.read()

    new_content = re.sub(pattern, replacement, content, flags=re.MULTILINE)
    
    if new_content != content:
        with open(full_path, "w") as f:
            f.write(new_content)
        print(f"✓ Updated {path}")
    else:
        print(f"– No changes for {path}")

def main():
    metadata = load_metadata()
    version = metadata["version"]
    year = metadata["license_year"]
    
    print(f"Syncing Pleno project metadata (Version: {version}, Year: {year})...")

    # 1. Cargo.toml files (Workspace root and all crates)
    cargo_toml_pattern = r'(^\[(?:workspace\.)?package\]\n(?:.*\n)*?version\s*=\s*")[^"]+(")'
    update_file("Cargo.toml", cargo_toml_pattern, rf'\g<1>{version}\g<2>')
    for cargo_toml in BASE_DIR.glob("crates/*/Cargo.toml"):
        update_file(cargo_toml.relative_to(BASE_DIR), cargo_toml_pattern, rf'\g<1>{version}\g<2>')

    # 2. ide/vscode/package.json
    update_file(
        "ide/vscode/package.json",
        r'("version"\s*:\s*")[^"]+(")',
        rf'\g<1>{version}\g<2>'
    )

    # 3. pleno.toml files
    pleno_toml_pattern = r'(^version\s*=\s*")[^"]+(")'
    for pleno_toml in BASE_DIR.glob("**/pleno.toml"):
        if "node_modules" in str(pleno_toml) or "target" in str(pleno_toml):
            continue
        update_file(pleno_toml.relative_to(BASE_DIR), pleno_toml_pattern, rf'\g<1>{version}\g<2>')

    # 4. SECURITY.md
    update_file(
        "SECURITY.md",
        r'(\| < )[^ ]+( \|)',
        rf'\g<1>{version}\g<2>'
    )

    # 5. README files (README.dev.md, tests/README.md)
    readme_files = ["README.dev.md", "tests/README.md"]
    for readme in readme_files:
        # Tag examples v0.1.0
        update_file(readme, r'(tag: "v)[^"]+(")', rf'\g<1>{version}\g<2>')
        update_file(readme, r'([- ]v)[0-9]+\.[0-9]+\.[0-9]+', rf'\g<1>{version}')
        update_file(readme, r'(:refs/tags/v)[^ \n]+', rf'\g<1>{version}')

    # 6. LICENSE
    update_file(
        "LICENSE",
        r'(Copyright \(c\) )[0-9-]+( Pleno Contributors)',
        rf'\g<1>{year}\g<2>'
    )

    # 7. specs.md
    update_file(
        "specs.md",
        r'(^## Complete Language Specification v)[^ \n]+',
        rf'\g<1>{version}'
    )

    # 8. ide/vscode/src/dashboard.ts (if exists)
    update_file(
        "ide/vscode/src/dashboard.ts",
        r'(Pleno v)[^ ]+',
        rf'\g<1>{version}'
    )

    print("Synchronization complete.")

if __name__ == "__main__":
    main()
