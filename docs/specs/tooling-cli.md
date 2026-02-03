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
# Run with debugger
pleno debug script.pln

# Connect to DAP (Debug Adapter Protocol)
pleno dap --port 4711
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