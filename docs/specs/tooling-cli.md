# Tooling & CLI

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

# Run with Sandboxing & Resource Limits
pleno run script.pln --max-instructions 1000000 --max-memory 10485760 --allow-io false --allow-network false

# Start REPL
pleno

# Watch mode (auto-reload)
pleno watch main.pln

# Build optimized binary
pleno build --release --output app

# Generate documentation
pleno doc --output docs/

# Start Language Server (LSP)
pleno lsp

# Package Manager Commands
plm init            # Initialize project
plm install         # Install dependencies
plm update          # Update dependencies
plm run <script>    # Run a script from package.pln
```