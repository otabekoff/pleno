# Package Manager (PLM)

PLM is Pleno's package manager for managing dependencies and project configuration.

## Quick Start

```bash
# Initialize new project
plm init

# Install a package
plm install json-parser

# Install specific version
plm install http-client@2.1.0

# Install dev dependency
plm install --dev test-framework

# Update all packages
plm update

# Remove a package
plm remove json-parser
```

## Project Configuration

The `pleno.toml` file defines your project:

```toml
[package]
name = "my-app"
version = "1.0.0"
description = "My Pleno application"
main = "src/main.pln"
authors = ["Your Name <you@example.com>"]
license = "MIT"

[scripts]
start = "pleno run src/main.pln"
build = "pleno build --release --output dist/"
test = "pleno test tests/"
dev = "pleno watch src/main.pln"

[dependencies]
http-client = "^2.1.0"
json-parser = "^1.5.0"
crypto = "1.0.0"

[dev-dependencies]
test-framework = "^3.0.0"
mock = "^1.2.0"
```

## Version Specifiers

| Syntax    | Meaning                  |
| --------- | ------------------------ |
| `1.5.0`   | Exact version            |
| `^1.5.0`  | Compatible (1.x.x)       |
| `~1.5.0`  | Minor-compatible (1.5.x) |
| `>=1.5.0` | Minimum version          |
| `*`       | Any version              |

## CLI Commands

```bash
# Project management
plm init              # Create new project
plm info              # Show project info

# Dependency management
plm install           # Install all dependencies
plm install <pkg>     # Install a package
plm update            # Update dependencies
plm remove <pkg>      # Remove a package

# Run scripts
plm run <script>      # Run a script from pleno.toml
plm run start         # Run 'start' script
plm run test          # Run 'test' script

# Publishing (the registry URL comes from PLENO_REGISTRY)
plm publish           # Publish to registry
plm search <term>     # Search the registry
```

## Lock File

`pleno.lock` ensures reproducible builds by locking exact versions:

```toml
# pleno.lock (auto-generated, do not edit manually)
[[package]]
name = "http-client"
version = "2.1.3"
checksum = "sha256:abc123..."

[[package]]
name = "json-parser"
version = "1.5.2"
checksum = "sha256:def456..."
```

## Workspaces

For monorepo projects with multiple packages:

```toml
# pleno.toml (workspace root)
[workspace]
members = [
    "packages/core",
    "packages/cli",
    "packages/web"
]

[workspace.dependencies]
shared-utils = { path = "../shared" }
```
