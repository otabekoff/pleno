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
# Compile to bytecode (writes main.plc next to main.pln)
pleno compile main.pln

# Run the compiled bytecode
pleno run main.plc

# Build optimized binary
pleno build --release --output app

# Cross-compile (future)
pleno build --target wasm32
```

### The `.plc` format

A `.plc` file starts with an 8-byte header, followed by the compiled program:

| Bytes | Contents |
|-------|----------|
| 0-3 | The magic bytes `PLNC` |
| 4-7 | The bytecode format version, a little-endian 32-bit integer (currently `1`) |
| 8- | The compiled program (bincode-encoded) |

`pleno run` checks the header before it reads anything else. A file that does not start with
`PLNC` fails with `not a Pleno bytecode file`. A file with another format version fails with
`` this .plc file was compiled by a different Pleno version (bytecode format N, this pleno reads M); recompile it with `pleno compile` ``,
and a cut-off file fails with an error that says it is truncated. Bytecode is never misread.

The format version goes up whenever the layout of compiled programs changes, which can happen in
any release. A `.plc` file only runs on a `pleno` that reads the same format version, so keep the
`.pln` sources and recompile after upgrading. `.plc` files written before Pleno 0.3 have no header
and must be recompiled.

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