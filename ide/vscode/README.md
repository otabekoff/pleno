# Pleno Language Support for VS Code

Official VS Code extension for the [Pleno programming language](https://github.com/otabekoff/pleno) - a modern, expressive language designed for simplicity and power.

## Features

### Syntax Highlighting
Full syntax highlighting support for `.pln` files, including:
- Keywords, operators, and control flow
- Functions, classes, and interfaces
- Strings, numbers, and comments
- Type annotations

### Code Snippets
Quick insertion of common Pleno constructs:
| Prefix | Description |
|--------|-------------|
| `fn` | Function declaration |
| `fnr` | Function with return type |
| `class` | Class declaration |
| `classex` | Class with extends |
| `interface` | Interface declaration |
| `enum` | Enum declaration |
| `for` | For-in loop |
| `forr` | For range loop |
| `while` | While loop |
| `if` / `ife` | If / If-else statement |
| `match` | Match expression |
| `import` | Import statement |
| `try` | Try-catch block |
| `let` / `const` / `mut` | Variable declarations |

### Language Server (LSP)
- **Diagnostics**: Real-time syntax error reporting
- **Go to Definition**: Navigate to function, class, and variable definitions (works across files!)
- **Hover Information**: View symbol kind and signature on hover

### Markdown Support
Syntax highlighting in Markdown fenced code blocks using ` ```pleno ` or ` ```pln `.

### File Icons
Custom file icons for `.pln` files (requires selecting "Pleno File Icons" in File Icon Theme settings).

## Installation

### 1. Install Libraries (LSP)
To get the full experience (diagnostics, hover, go-to-definition), you need to install the Pleno binaries.

1.  Clone the repository.
2.  Run the installer:
    ```powershell
    magke install
    # OR 
    pwsh -File tasks/install.ps1
    ```
    This builds the tools and adds them to your `%USERPROFILE%\.pleno\bin` in PATH.

### 2. Install Extension
1.  Run `npm install` and `npm run pack` in `ide/vscode`.
2.  Install the generated `.vsix` in VS Code.

## Configuration

The extension automatically looks for `pleno-lsp` in your system PATH. If not found, it tries to use the local development build in `target/release`.

To enable Pleno file icons:
3. Select "Pleno File Icons" in "Preferences: File Icon Theme".

## Links

- [Pleno Repository](https://github.com/otabekoff/pleno)
- [Language Specification](https://github.com/otabekoff/pleno/blob/main/specs.md)
- [Report Issues](https://github.com/otabekoff/pleno/issues)

## License

MIT
