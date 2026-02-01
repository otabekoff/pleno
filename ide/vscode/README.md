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
- **Smart Completion**: Property-aware completions (e.g. `.length`)
- **Outline view**: Easily navigate classes and functions
- **Formatting**: Auto-format code on save (using `pleno-formatter`)

### Project Management

- **Pleno Project Explorer**: A dedicated sidebar view to explore modules, dependencies, and scripts from your `pleno.toml`.
- **Integrated Tasks**: Automatic detection of build, run, and test tasks for Pleno projects.
- **Project Dashboard**: A beautiful overview of your project health and quick access to resources.
- **Getting Started Walkthrough**: A native VSCode guide to help you set up and write your first Pleno code.

### Commands

- **Run Current File**: Execute the active `.pln` file in the terminal.
- **Open Interactive REPL**: Launch the Pleno REPL for interactive experimentation.
- **Show Dashboard**: Open the Pleno Dashboard for project overview.
- **Restart Server**: Manually restart the Pleno Language Server with a single click.

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


<!-- 
"onStartupFinished",
    "onLanguage:pleno",
    "onView:plenoProject",
    "onView:plenoOutline",
    "onCommand:pleno.showDashboard",
    "onCommand:pleno.showControlPanel",
    "onCommand:pleno.runFile",
    "onCommand:pleno.checkFile",
    "onCommand:pleno.refreshProject",
    "onCommand:pleno.reloadWorkspace",
    "onCommand:pleno.openDocs",
    "onCommand:pleno.openWalkthrough",
    "onCommand:pleno.createProject",
 -->