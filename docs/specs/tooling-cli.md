# Tooling & CLI

The Pleno CLI provides tools for running, building, testing, and formatting Pleno code. `pleno --help` lists the commands, and `pleno <command> --help` shows the options of one. A command rejects options it does not take (`Error: 'format' has no option '--check'`, exit status 2).

## Running Code

```bash
# Run a script
pleno run script.pln

# Run with arguments; Process.args() returns [<path of pleno>, "run", "script.pln", "arg1", "arg2"]
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
| 4-7 | The bytecode format version, a little-endian 32-bit integer (currently `5`) |
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

- syntax and compile errors (such as `cannot assign to immutable binding`), and names that are read but never defined. Unused functions are compiled too. A file that exports names or declares `module name;` is compiled as a module, the way importing it compiles it;
- calls with the wrong arguments, counting default, variadic (`...rest`), keyword-only (after `*`) and named arguments: `greet expects 1 to 2 arguments, got 3`, `greet has no parameter named 'x'`, `config is missing named argument 'port'`. This covers functions and classes declared at the top level of the file, methods called on `this` or on a value of a known class (`p.shift(1)` gives `Point.shift expects 2 arguments, got 1`), static methods (`Point.origin(1)`), lambdas bound with `let` (`let f = (x) => x; f(1, 2)`), and functions imported from another `.pln` file, by name or through the module (`m.helper()`);
- values that can never match a built-in type annotation: `let x: Int = "hi";` gives `'x' is declared Int but given a String literal`, and `f(1)` for `fn f(s: String)` gives `f expects String for 's', found an Int literal`. Besides literals, a value's type is known when it is an instance made by calling a class declared in the file (`a Point instance`), the result of a function declared in the file with an `Int`, `Float`, `String` or `Bool` return annotation (`an Int value`; the VM checks every return against it), or a name bound once with `let` (without `mut`) to one of these;
- as a warning, calls to methods that the class and its superclasses never declare, on `this`, on a value of a known class (`Point has no method 'shfit'; did you mean 'shift'?`) or on the class itself (`Point has no static method 'orgin'; did you mean 'origin'?`);
- as a warning, names imported from another `.pln` file that it does not export: `import { nmae } from "./m.pln";` gives `'./m.pln' does not export 'nmae'; did you mean 'name'?` (the import gives `null`). Only imports with a relative path (`./`, `../`) are read, from beside the checked file, with `.pln` added when the path has no extension. A file that cannot be read or parsed is not checked against.

The checks stay quiet whenever the code could still be right: a name that is bound more than once in the file or with `let mut`, a decorated function or class, a class changed by `extend`, inheriting from a class in another file, implementing an interface or exported (an importer could extend it), for methods on `this` also a subclassed class, a member that is a field or property, or that is assigned or named in a string anywhere in the file, a file that uses `reflect`, a call with `...spread` or `_` placeholders, a call inside `try` with a `catch`, inside `assert.throws`, or in a function given to a function that calls its argument inside a `try` with a `catch`, and any annotation other than `Int`, `Float`, `Number`, `String`, `Char`, `Bool`, `Null`, `Void` and `Any`. An Int is never reported against `Float`. `@inline` and `async` functions do not give their result a known type. Errors make `pleno check` exit with status 1; warnings do not. `pleno check --json` prints the same problems with `"severity": "error"` or `"warning"`, and the language server shows them as you type, compile errors included.

## Formatting

```bash
# Format a file in place (`pleno fmt` is the same command)
pleno format main.pln

# Format every .pln file in a folder
pleno format src/

# Print the formatted file instead of writing it
pleno format --stdout main.pln
```

The editor's **Format Document** and format on save use the same formatter.

Lines are kept within 100 characters, the limit the linter's `max-line-length` uses. A statement that fits on one line is printed on one line, also when it was written over several lines. A statement that does not fit is broken at its outermost construct first, and an element that is still too long is broken the same way:

- call arguments, array, map, set and tuple literals, parameter lists of functions and methods, and `import { ... }` and `export { ... }` lists put each element on its own line, indented one level, with a comma after every element (except after a variadic `...rest` parameter, which must come last);
- a chain of two or more method calls puts each `.call(...)` on its own line;
- a chain of binary operators breaks before each operator, and `a ? b : c` before `?` and `:`.

```pleno norun
let report = summarizeMeasurements(
    temperatureReadings,
    pressureReadings,
    humidityReadings,
    options.precision,
);

let names = people
    .filter((person) => person.age >= 18)
    .map((person) => person.fullName)
    .sort()
    .join(", ");

let valid = isWellFormed(input)
    && hasNoDuplicateKeys(input)
    && matchesTheSchema(input, schema)
    && isFresh(input);
```

Strings, template literals and comments are never split, so a line that holds a long one can stay longer than the limit. A comment in a broken list stays with the element it follows or precedes, and a comment between elements keeps the list broken. Formatting a formatted file again changes nothing.

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

# Add a package from a Git repository (a version is a tag vX.Y.Z)
plm add github:ada/greet@^1.2

# Install what pleno.lock records
plm install

# Update dependencies, and show the tree
plm update
plm tree

# Check the package and print the commands that tag a release
plm publish

# Run a script from pleno.toml
plm run <script>
```

## Global Options

| Flag | Description |
|------|-------------|
| `--help`, `-h` | Show help |
| `--version`, `-v` | Show version |

Set the `NO_COLOR` environment variable to turn off colored output.