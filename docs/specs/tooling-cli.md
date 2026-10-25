# Tooling & CLI

The Pleno CLI provides tools for running, building, testing, and formatting Pleno code. `pleno --help` lists the commands, and `pleno <command> --help` shows the options of one. A command rejects options it does not take (`Error: 'format' has no option '--check'`, exit status 2).

## Running Code

```bash
# Run a script
pleno run script.pln

# Run with arguments; Process.args() returns [pleno, run, script.pln, arg1, arg2]
pleno run script.pln arg1 arg2

# Start interactive REPL
pleno repl

# Watch mode (auto-reload on changes)
pleno watch main.pln
```

## Building

```bash
# Compile to bytecode (writes main.plc next to main.pln, or where -o says)
pleno compile main.pln
pleno compile main.pln -o build/app.plc

# Run the compiled bytecode
pleno run main.plc

# Compile the package's main file (from pleno.toml) with the release profile
pleno build --release -o build/app.plc
```

### The `.plc` format

A `.plc` file starts with an 8-byte header, followed by the compiled program:

| Bytes | Contents |
|-------|----------|
| 0-3 | The magic bytes `PLNC` |
| 4-7 | The bytecode format version, a little-endian 32-bit integer (currently `4`) |
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
# Check one file without running it
pleno check main.pln
pleno check --json main.pln

# Lint a file, or every .pln file in a folder, for style issues
pleno lint src/
```

`pleno check` reports, without running the file:

- syntax and compile errors, and names that are read but never defined;
- calls with the wrong arguments to functions and classes declared at the top level of the file and to methods called on `this`, counting default, variadic (`...rest`), keyword-only (after `*`) and named arguments: `greet expects 1 to 2 arguments, got 3`, `greet has no parameter named 'x'`, `config is missing named argument 'port'`;
- literals that can never match a built-in type annotation: `let x: Int = "hi";` gives `'x' is declared Int but given a String literal`, and `f(1)` for `fn f(s: String)` gives `f expects String for 's', found an Int literal`;
- as a warning, calls to methods on `this` that the class and its superclasses never declare: `Counter has no method 'reset'`.

The checks stay quiet whenever the code could still be right: a name that is bound more than once, a decorated function or class, a class changed by `extend` or inheriting from a class in another file, for methods on `this` also a class that is exported, subclassed or implements an interface, a call with `...spread` or `_` placeholders, a call inside `try` with a `catch` or inside `assert.throws`, and any annotation other than `Int`, `Float`, `Number`, `String`, `Char`, `Bool`, `Null`, `Void` and `Any`. An Int literal is never reported against `Float`. Errors make `pleno check` exit with status 1; warnings do not. `pleno check --json` prints the same problems with `"severity": "error"` or `"warning"`, and the language server shows them as you type.

## Formatting

```bash
# Format a file in place
pleno format main.pln

# Format every .pln file in a folder
pleno format src/

# Print the formatted file instead of writing it
pleno format --stdout main.pln
```

There is no check-only mode; to check formatting in CI, run `pleno format` and fail if `git diff` shows changes.

## Testing

```bash
# Run all tests in directory
pleno test tests/

# Run specific test file
pleno test tests/unit/math_test.pln

# Print the results as JSON
pleno test --json tests/
```

## Documentation

```bash
# Generate HTML from the doc comments (///) in src/ into docs_out/
pleno doc src docs_out
```

Both arguments are optional: the path defaults to `.` and the output folder to `docs_out`.

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

# Allow reading and writing only inside the given folders
pleno run script.pln --allow-read data/ --allow-write out/
```

There is no time limit; `--max-instructions` bounds how long a program runs. See [Security](./security-features.md) for what each flag does and its limits.

## Language Server (LSP)

```bash
# Start the language server on stdio, for editors
pleno lsp
```

## Debugging

```bash
# Start the debug adapter (Debug Adapter Protocol over stdio).
# Editors start it for you: in VS Code, open a .pln file and press F5.
pleno dap
```

## Package Manager

```bash
# Create a project in a new folder, or in the current one
pleno new my-app
pleno init

# Install dependencies
plm install

# Add a package from Git (or by name, from the registry in PLENO_REGISTRY)
plm install https://github.com/user/http-utils

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

Set the `NO_COLOR` environment variable to turn off colored output.