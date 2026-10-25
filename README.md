# Pleno Programming Language

[![CI](https://github.com/otabekoff/pleno/actions/workflows/ci.yml/badge.svg)](https://github.com/otabekoff/pleno/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](https://github.com/otabekoff/pleno/releases)

Pleno is a modern, high-performance programming language designed for clarity, safety, and expressiveness. It combines the ease of use of dynamic languages with the robustness of modern systems programming features.

## ✨ Key Features

- **🚀 Performance**: Low-level VM implementation in Rust with optimized opcodes.
- **🛡️ Safety**: Optional static typing, deep immutability, and safe navigation.
- **🧵 Concurrency**: Built-in support for `async/await`, `Mutex`, and `Channel` primitives.
- **📦 Expressive Syntax**: Pattern matching, string interpolation, and powerful closures.
- **🧩 Modern OOP**: True encapsulation with private/protected fields and interfaces.

## 🏗️ Architecture

```mermaid
graph TD
    Source[File.pln] --> Lexer
    Lexer --> Parser
    Parser --> AST[Abstract Syntax Tree]
    AST --> Compiler
    Compiler --> Bytecode
    Bytecode --> VM[Pleno VM]
    VM --> Stdlib[Standard Library]
    VM --> Runtime[Native Runtime]
```

## 📦 Workspace Layout

- `crates/pleno-ast` - AST nodes and span tracking.
- `crates/pleno-lexer` - High-performance lexical scanner.
- `crates/pleno-parser` - Recursive descent parser.
- `crates/pleno-compiler` - Bytecode generation and optimization.
- `crates/pleno-vm` - Stack-based virtual machine.
- `crates/pleno-stdlib` - Standard library (Promises, Concurrency, etc.).
- `crates/pleno-cli` - Language runner and dev tools.

## 🚀 Quick Start

### Installation

For Linux and macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/install.sh | sh
```

For Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/install.ps1 | iex
```

The installers put the binaries in `~/.pleno/bin`, add it to your `PATH`, and verify each download's checksum. To uninstall, run `scripts/uninstall.sh` or `scripts/uninstall.ps1` the same way.

Prebuilt archives for Linux, macOS and Windows are on the [Releases page](https://github.com/otabekoff/pleno/releases). See the [Code signing policy](https://pleno.mohirlab.uz/code-signing-policy) for how releases are signed and verified.

To install the current state of a checkout instead of a release (the binaries in `~/.pleno/bin` and the VS Code extension from `ide/vscode`), run this from the repository; run it again after pulling to update:

```bash
scripts/dev-install.sh        # Linux and macOS
scripts\dev-install.ps1       # Windows (PowerShell)
```

It needs Rust, plus Node.js and VS Code's `code` command for the extension (skip that part with `--skip-extension` / `-SkipExtension`).

### Your First Script

Create a file named `hello.pln`:

```pleno
print("Hello, Pleno!");

let name = "Developer";
print("Welcome, ${name}!");
```

Run it:

```bash
pleno run hello.pln
```

## 🛠️ CLI Usage

- `pleno run <file>` - Execute a Pleno script.
- `pleno repl` - Interactive shell.
- `pleno format <path>` - Auto-format your code.
- `pleno lint <path>` - Check for common issues.
- `pleno test <path>` - Run the built-in test suite.

## 📖 Documentation

- [Documentation website](https://pleno.mohirlab.uz/)
- [Language reference](https://pleno.mohirlab.uz/specs/) and [compatibility promise](https://pleno.mohirlab.uz/specs/compatibility)
- [Architecture](ARCHITECTURE.md)
- [Roadmap to 1.0](tasks/v1.md)
- [Changelog](CHANGELOG.md)
- [Contributing](CONTRIBUTING.md), [Governance](GOVERNANCE.md), [Getting help](SUPPORT.md)
- [Development Guide](README.dev.md)

## 🔏 Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org). Read the full [Code signing policy](https://pleno.mohirlab.uz/code-signing-policy), including team roles and the privacy policy.

## 📜 License

Pleno is released under the **MIT License**. See [LICENSE](LICENSE) for details.
