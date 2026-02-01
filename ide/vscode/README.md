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
| `prop` | Property declaration |
| `get` / `set` | Getter / Setter declaration |
| `op` | Operator overload |
| `macro` | Macro declaration |
| `for` | For-in loop |
| `while` | While loop |
| `if` / `ife` | If / If-else statement |
| `tryf` | Try-catch-finally block |

### Language Server (LSP)

- **Diagnostics**: Real-time syntax error reporting
- **Go to Definition**: Navigate to function, class, and variable definitions
- **Hover Information**: View symbol signatures on hover
- **Activation Alert**: Notifies you if the LSP is found and where

### Commands

- **Run Current File**: Execute the active `.pln` file in the terminal.
- **Check Current File**: Perform a static check of the active file.
- **Restart Server**: Manually restart the Pleno Language Server.

### Markdown Support

Full syntax highlighting in Markdown fenced code blocks using ` ```pleno ` or ` ```pln `.

### File Icons

Custom file icons for `.pln` and `.pleno` files.

## Installation

### 1. Install Pleno CLI & LSP

The extension requires `pleno` and `pleno-lsp` to be in your PATH.

1.  Clone the [Pleno repository](https://github.com/otabekoff/pleno).
2.  Install via Makefile: `make install`.

### 2. Install Extension

1.  Open the "Extensions" view in VS Code (`Ctrl+Shift+X`).
2.  Search for "Pleno" or install from VSIX.

## Configuration

You can configure the extension in `settings.json`:

```json
{
  "pleno.lsp.path": "pleno-lsp",
  "pleno.trace.server": "messages"
}
```

## Icons

To enable Pleno file icons:

1. Select "Pleno" in "Preferences: File Icon Theme".

## Links

- [Pleno Repository](https://github.com/otabekoff/pleno)
- [Language Specification](https://github.com/otabekoff/pleno/blob/main/specs.md)
- [Report Issues](https://github.com/otabekoff/pleno/issues)

## License

MIT
