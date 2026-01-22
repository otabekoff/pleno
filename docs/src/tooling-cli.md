## 17. Tooling & CLI

```bash
# Run Pleno file
pleno run script.pln

# Run with arguments
pleno run script.pln arg1 arg2

# Compile to bytecode
pleno compile main.pln --output main.plc

# Type check without running
pleno check main.pln

# Format code
pleno format src/

# Lint code
pleno lint src/

# Start REPL
pleno

# Watch mode (auto-reload)
pleno watch main.pln

# Build optimized binary
pleno build --release --output app

# Generate documentation
pleno doc --output docs/