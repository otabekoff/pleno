# Tooling & CLI

The Pleno CLI provides tools for running, building, testing, and formatting Pleno code.

## Running Code

```bash
# Run a script
pleno run script.pln

# Run with arguments
pleno run script.pln arg1 arg2

# Start interactive REPL
pleno repl

# Watch mode (auto-reload on changes)
pleno watch main.pln
```

## Building

```bash
# Compile to bytecode
pleno compile main.pln --output main.plc

# Build optimized binary
pleno build --release --output app

# Cross-compile (future)
pleno build --target wasm32
```

## Type Checking & Linting

```bash
# Type check without running
pleno check main.pln
pleno check src/

# Lint code for style issues
pleno lint src/

# Fix auto-fixable issues
pleno lint --fix src/
```

`pleno check` reports, without running the file:

- syntax and compile errors, and names that are read but never defined;
- calls with the wrong arguments to functions and classes declared at the top level of the file and to methods called on `this`, counting default, variadic (`...rest`), keyword-only (after `*`) and named arguments: `greet expects 1 to 2 arguments, got 3`, `greet has no parameter named 'x'`, `config is missing named argument 'port'`;
- literals that can never match a built-in type annotation: `let x: Int = "hi";` gives `'x' is declared Int but given a String literal`, and `f(1)` for `fn f(s: String)` gives `f expects String for 's', found an Int literal`;
- as a warning, calls to methods on `this` that the class and its superclasses never declare: `Counter has no method 'reset'`.

The checks stay quiet whenever the code could still be right: a name that is bound more than once, a decorated function or class, a class changed by `extend` or inheriting from a class in another file, for methods on `this` also a class that is exported, subclassed or implements an interface, a call with `...spread` or `_` placeholders, a call inside `try` with a `catch` or inside `assert.throws`, and any annotation other than `Int`, `Float`, `Number`, `String`, `Char`, `Bool`, `Null`, `Void` and `Any`. An Int literal is never reported against `Float`. Errors make `pleno check` exit with status 1; warnings do not. `pleno check --json` prints the same problems with `"severity": "error"` or `"warning"`, and the language server shows them as you type.

## Formatting

```bash
# Format a file
pleno format main.pln

# Format directory
pleno format src/

# Check formatting (CI mode)
pleno format --check src/
```

## Testing

```bash
# Run all tests in directory
pleno test tests/

# Run specific test file
pleno test tests/unit/math_test.pln

# Run with coverage
pleno test --coverage tests/
```

## Documentation

```bash
# Generate documentation
pleno doc --output docs/

# Serve docs locally
pleno doc --serve
```

## Sandboxing & Limits

Run untrusted code safely:

```bash
# Run with resource limits
pleno run script.pln \
    --max-instructions 1000000 \
    --max-memory 10485760

# Disable I/O and network
pleno run script.pln \
    --allow-io false \
    --allow-network false

# Timeout execution
pleno run script.pln --timeout 5000
```

## Language Server (LSP)

```bash
# Start LSP for IDE integration
pleno lsp

# LSP with debug logging
pleno lsp --log-level debug
```

## Debugging

```bash
# Start the debug adapter (Debug Adapter Protocol over stdio).
# Editors start it for you: in VS Code, open a .pln file and press F5.
pleno dap
```

## Package Manager

```bash
# Initialize project
plm init

# Install dependencies
plm install

# Add a package
plm install json-parser

# Update dependencies
plm update

# Run a script from pleno.toml
plm run <script>
```

## Global Options

| Flag | Description |
|------|-------------|
| `--help`, `-h` | Show help |
| `--version`, `-v` | Show version |
| `--verbose` | Verbose output |
| `--quiet` | Suppress output |
| `--color <auto\|always\|never>` | Color output |