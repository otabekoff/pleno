# Package Manager (PLM)

PLM is Pleno's package manager for managing dependencies and project configuration. `plm <command>` and `pleno plm <command>` are the same.

There is no public package registry yet. Dependencies come from a local path or a Git repository; installing by name and version works only with a registry server whose URL is in the `PLENO_REGISTRY` environment variable.

## Quick Start

```bash
# Create a new project in a new folder, or in the current one
plm new my-app
plm init

# Install a package from Git
plm install https://github.com/user/http-utils

# Install by name and version (needs PLENO_REGISTRY)
plm install http-client@2.1.0

# Install dev dependency
plm install --dev test-framework

# Update all packages
plm update

# Remove a package
plm remove http-utils
```

## Project Configuration

The `pleno.toml` file defines your project:

```toml
[package]
name = "my-app"
version = "1.0.0"
description = "My Pleno application"
main = "src/main.pln"

[scripts]
start = "pleno run src/main.pln"
build = "pleno build --release -o dist/app.plc"
test = "pleno test tests/"
dev = "pleno watch src/main.pln"

[dependencies]
greet = { path = "../greet" }
http-utils = { git = "https://github.com/user/http-utils", tag = "v1.2.0" }
json-parser = "^1.5.0"          # from the registry

[dev-dependencies]
mock = { git = "https://github.com/user/mock", branch = "main" }
```

`[package]` takes `name`, `version`, `description` and `main` (default `src/main.pln`); other keys, such as `authors` or `license`, are accepted and ignored. A Git dependency can name a `tag`, `branch` or `rev`.

## Version Specifiers

Version requirements apply to registry dependencies and follow Cargo's rules:

| Syntax    | Meaning                                |
| --------- | -------------------------------------- |
| `=1.5.0`  | Exactly 1.5.0                          |
| `1.5.0`   | Same as `^1.5.0`                       |
| `^1.5.0`  | Compatible: at least 1.5.0, below 2.0.0 |
| `~1.5.0`  | At least 1.5.0, below 1.6.0            |
| `>=1.5.0` | Minimum version                        |
| `*`       | Any version                            |

## CLI Commands

```bash
# Project management
plm new <folder>      # Create a folder with a new project
plm init              # Create a project in the current folder
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

# Building and testing the package
plm build
plm test

# Publishing (the registry URL comes from PLENO_REGISTRY)
plm publish           # Publish to registry
plm search <term>     # Search the registry
```

## Lock File

`plm install` writes `pleno.lock`, which records what was installed so the next install gets the same code: the version, the source, the Git commit for Git dependencies and a checksum for downloaded ones. Do not edit it by hand.

```toml
[[package]]
name = "http-utils"
version = "1.2.0"
source = "git+https://github.com/user/http-utils"
commit = "3f2c9e1..."
```
