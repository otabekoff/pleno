# Changelog

Notable changes to Pleno. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Before 1.0, any release may change behavior.

## Unreleased

### Added

- `<`, `<=`, `>` and `>=` compare strings and characters, by Unicode code point (`"abc" < "abd"`); before, they were an error.
- `Comparable` is a built-in constraint: `T: Comparable` accepts `Int`, `Float`, `String`, `Char` and classes that define `operator <` or `operator >`.
- `<`, `<=`, `>` and `>=` compare strings and characters, by Unicode code point (`"abc" < "abd"`); before, they were an error.
- `Comparable` is a built-in constraint: `T: Comparable` accepts `Int`, `Float`, `String`, `Char` and classes that define `operator <` or `operator >`.
- `@deprecated` and `@deprecated("note")` on a function or method print `warning: f is deprecated: note` the first time it is called, and `pleno check` and the editor report calls to a deprecated top-level function. This is how stable APIs will be deprecated in 1.x.
- `pleno new <folder>` (and `plm new`) creates the folder and a package in it, named after the folder or `--name`; it refuses a folder that already has files. `pleno init` still sets up the current folder.
- `pleno-dap --version`, `pleno-repl --version` and `--help` for both.
- `vmStats()` in `std:reflect`: instructions executed, current and peak memory.
- `Error` is a class: `Error(message)` instances print as `Error: message` and have `toString()`; subclasses get their class name as `name`.
- Destructuring in `let`: `let (a, b) = pair;`, `let mut (x, (y, _)) = nested;` and `let [first, ...rest] = items;`, with runtime errors such as `tuple has 2 elements, pattern expects 3`. Tuple type annotations such as `(Int, String)` and `(Int, Int)[]` are checked at runtime.
- `pleno check` and the language server report wrong argument counts and unknown named arguments for functions, constructors and `this` methods declared in the file, literals that cannot match a type annotation (`let x: Int = "hi";`), and, as a warning, unknown methods called on `this`.
- `pleno run file.plc` runs bytecode written by `pleno compile`.
- Destructuring in `for` loops: `for ((k, v) in pairs)`, `for ([a, b] in rows)`, `for ((i, (x, y)) in nested)` and `for ((k, _) in m.entries())` take each item apart like a destructuring `let`, with the same errors (`tuple has 2 elements, pattern expects 3`). Over a map, a pattern matches each `(key, value)` entry. A `for` loop can also iterate an iterator such as `m.entries()` directly; before, it threw `type 'Iterator' is not iterable`.
- Return type annotations are checked at runtime, at every `return` and when a function ends without one: `fn f(): Int { return "s"; }` throws `TypeError: f returned String, expected Int`, and falling off the end of a function annotated `Int` throws `f returned Null, expected Int`. This covers functions, methods, getters, lambdas and `async` functions (whose annotation, `Int` or `Promise<Int>`, describes the resolved value). `Void`, `Any` and annotations mentioning a type parameter are not checked. `pleno check` also reports returned literals that cannot match (`f is declared to return Int but returns a String literal`).
- `Regex(pattern, flags)` applies its flags: `i` (case-insensitive), `m` (multi-line), `s` (`.` matches newlines) and `x` (verbose); `regex.flags` returns them. Before, the second argument was ignored. Unknown flag letters and invalid patterns throw when the regex is created.
- `CSV.parse(text, { header: false })` (or `header: false` as a named argument) returns every row, the first one included, for input without a header line. The default is unchanged: the first row is the header and is dropped.
- Number and Bool literals as map literal keys: `{1: "a", 2.5: "b", true: "c"}` is `{"1": "a", "2.5": "b", "true": "c"}`, since map keys are strings. Reading and writing with such an index (`m[1]`, `m[true] = x`) and `get`, `put`, `with`, `remove` and `containsKey` use the same string, so `m[1]` reads the key `"1"`.

### Changed

- A package created by `pleno new` or `plm init` starts at version 0.1.0 instead of the version of Pleno that created it.
- Programs run with an instruction or memory limit (`--max-instructions`, `--max-memory`, the playground) use the fast interpreter loop too, which checks the limits itself: about 1.6 times faster than before under a limit.
- `pleno format`, `lint`, `check`, `test`, `compile`, `watch`, `doc`, `debug-ast` and `build` reject options and arguments they do not take (`Error: 'compile' has no option '--bogus'`, exit status 2) instead of ignoring them, and print their own usage with `--help`. `pleno compile` and `pleno build` take `-o <file.plc>` and create its folder.
- `pleno build` with a target that is not a `.pln` file is an error; before, it built the package's main file instead.
- The editor's code lenses are "Run File" and "Debug File" above the first statement, instead of a "Run Function" over every function that ran the whole file.
- `plm init` no longer writes `.vscode/tasks.json` (the extension provides those tasks), its `.vscode/settings.json` only sets formatting for Pleno files and nests `pleno.lock` under `pleno.toml`, and `launch.json` drops the unsupported `stopOnEntry`.
- A variable declared without `mut` (or with `const`) can no longer be reassigned from inside a function or closure; before, `let x = 1; fn f() { x = 2; } f();` changed `x`. Assigning to a name declared with `fn`, `class`, `enum` or `interface` says so (`cannot assign to function 'g'`) instead of `unknown local or global`.
- Compile errors in unused functions and classes and in code after a `return` are reported by `pleno run` and `pleno check`; before, the optimizer dropped that code first and its errors went unnoticed until it was used.
- Every caught error is an `Error` instance with `name` and `message`: runtime errors, failed type annotations (`TypeError`), denied operations (`SecurityError`) and sandbox limits (`SandboxError`) included, so `catch (e: Error)` catches them and `e.message` always works.
- Printing an error or putting it in a string gives `name: message`, such as `TypeError: parameter 'x' expected Int, got String`, instead of a map.
- Caught errors no longer carry `peakMemory` and `instructions`; `vmStats()` from `std:reflect` returns them.
- Ints and Floats compare by value: `5.0 == 5` is true, so `assert.equal(10 / 2, 5)` passes. Sets, `contains` and `indexOf` follow the same rule (`Set([5, 5.0])` has one element).
- `.plc` files start with a `PLNC` magic number and a bytecode format version, checked on load; files from another Pleno version, older headerless files and truncated files are rejected with an error asking to recompile.
- Faster execution: recursive `fib(30)` runs in 0.12 s instead of 2.1 s, a 10-million-step loop in 0.26 s instead of 0.9 s, and `map`/`filter`/`reduce` over a million elements about 3 times faster. Calls pass their arguments in registers, global lookups are cached, common instruction pairs run as one, and strings and big integers no longer pile up until the next object collection.
- An `Int` is accepted where a `Float` is annotated and is converted to a `Float` there (parameters, `let`, assignments, fields, return values, `Float?`, unions such as `Float | String`); before, `fn f(x: Float) {} f(3);` and `let y: Float = 3;` threw a `TypeError`. In a union that allows `Int` (`Int | Float`, `Number`) an `Int` stays an `Int`, and `Float[]` accepts `Int` elements without converting them. `pleno check` now reports a `Float` literal given for an `Int` annotation.
- Reassigning a `&mut` parameter (`value = ...`, `value++`) is a compile error, `cannot assign to '&mut' parameter 'value'`: the new value never reached the caller's variable. Changing a value in place (`items.push(x)`) is unchanged.
- Faster still: Pleno now beats CPython 3.14 on every benchmark in `bench/cases` and on `bench/src/array` (`fib(30)` 0.09 s against 0.10 s, the array benchmark 0.16 s against 0.17 s). Method calls such as `items.push(x)` no longer allocate a bound method, `s = s + x` appends in place when nothing else holds the old string (building a string is linear instead of quadratic), values take 16 bytes instead of 40, simple instructions run in a smaller interpreter loop, and calls and `map`/`filter`/`reduce` callbacks set up their frames with less work.
- The bytecode format version is 5 (new `ToFloat`, `CallRegs`, `GetMethod` and `CallMethod` instructions, and a `GetIterator` mode for destructuring `for` loops); recompile `.plc` files.
- `pleno format` (and format on save in the editor) keeps lines within 100 characters, the linter's limit. A statement that does not fit is broken at its outermost construct: call arguments, array, map, set and tuple literals, parameter lists and import and export lists go one element per line with a trailing comma, chains of two or more method calls one call per line, and binary operator chains before each operator. A statement that fits is printed on one line, also when it was written over several; before, every statement was joined into one line however long. Strings and comments are never split.

### Fixed

- A very long chain of operators or calls in one statement (`1 + 1 + ...` with tens of thousands of terms) overflowed the stack while compiling. A statement can now chain up to 1,000 operators and calls; a longer one is a parse error (`expression too long`). The browser playground gets an 8 MB WebAssembly stack instead of 1 MB. Found by the `fuzz_compile` fuzz target.
- Generic constraints were checked only with explicit type arguments: `greet(Rock())` with `fn greet<T: Named>(x: T)` was accepted and failed later. A parameter annotated `T` is now checked against `T`'s constraints, and the error names them (`expected T: Named, got Rock`). `T: Comparable` rejected `Int`.
- Generic constraints were checked only with explicit type arguments: `greet(Rock())` with `fn greet<T: Named>(x: T)` was accepted and failed later. A parameter annotated `T` is now checked against `T`'s constraints, and the error names them (`expected T: Named, got Rock`). `T: Comparable` rejected `Int`.
- Runaway recursion crashed the process: plain recursion grew memory until an allocation failed, and recursion through callbacks (`map`, operators, `test` inside `afterEach`) overflowed the stack. Both now raise a catchable `RecursionError`: calls can nest 100,000 deep, and calls from built-in functions back into Pleno 200 deep (64 in the browser playground). Found by the `fuzz_run` fuzz target.
- String positions and lengths count Unicode characters everywhere, as the reference says: `length`, `s[a..b]`, `indexOf`, `substring`, `charAt`, a regex match's `index` and `assert.lengthOf` counted UTF-8 bytes while `s[i]` counted characters, so `"héllo".length` was 6. `"héllo".substring(0, 2)` and `charAt(1)` stopped the VM with a panic; they now give `hé` and `é`. `split("")` returns the characters (`["a", "b", "c"]` instead of `["", "a", "b", "c", ""]`). `substring` clamps negative positions to 0 and swaps its arguments when `start > end`, like JavaScript (before, both gave `""`), and `repeat` with a negative count throws an error instead of stopping the VM.
- Sandbox path rules (`--allow-read`, `--allow-write`, `allowedReadPaths`, `allowedWritePaths`) compared text prefixes, so `data/../secret.txt` passed a rule for `data/` and `data2/` passed a rule for `data`. They now compare normalized absolute paths, folder by folder. `Path.exists`, `isFile`, `isDir`, `resolve` and `dotenv.config` count as reading, and `process.cwd()` needs I/O.
- A sandbox option with a value that does not parse (`--allow-io maybe`, `--max-memory 10MB`) meant "allowed" or "no limit"; it is now an error.
- A sandbox limit (`--max-instructions`, `--max-memory`, `Sandbox` options) could be caught by `try`/`catch`, so a program could catch it and run on past its limit. Reaching a limit now ends the sandboxed program; the program that started a `Sandbox` still catches it as a `SandboxError` (before, that error arrived named `Error`).
- `a != b` ignored a class's `operator ==`: with `==` overloaded, `V(1) != V(1)` was `true`. It is now `!(a == b)` unless the class defines its own `operator !=`.
- An error thrown inside a callback (`map`, `filter`, ...) and caught outside read `uncaught exception: uncaught exception: ...`; it now has its own message.
- Code in a module could call the top-level functions and classes of the program that imported it (`helper()` in a module found the main program's `helper`). A module now sees its own names, its imports and the built-in globals only, and still gets the built-in `print` or `typeof` when the main program defines its own.
- A callback given to `map`, `filter` and the other natives that call back returned its first argument instead of `null` when that argument was a map with a `variant` key: `[Ok(1)].map(|r| => null)` was `[Ok(1)]`.
- An array or map literal made only of constants (`[]`, `[0, 0]`, `{"n": 0}`) was created once and shared: a function returning `let a = []; a.push(1); return a;` gave the same array on every call, so it grew. Each evaluation now creates a new one.
- Changing an array inside its `reduce` callback, or a set inside its `forEach` or `filter` callback, crashed the VM (`RefCell already borrowed`). These now iterate over the elements as they were when called, like `map` and `forEach` on arrays.
- The linter (and so the editor) no longer reports lambdas (`=>`) and generic types (`Promise<Int>`) as operators missing spaces, type aliases and other names used only in type annotations as unused, or standard library globals such as `Ok`, `Some`, `Set` and `Error` as undefined. Functions can be used before their declaration, the line limit is 100 and skips comments, and every warning points at the whole name or operator.
- Intersection types (`type Person = Nameable & Ageable;`) accept a value that matches every part; before, a class implementing both interfaces was rejected with `expected Person, got Employee`.
- Type aliases inside a larger annotation (`Money?`, `Money | String`, `Money[]`) and as class field types are expanded before checking.
- An import makes visible only the names it binds. Other exports and the private names of an imported module no longer leak into the importing file (`import { PI } from "./m.pln"; square(2);` is now `undefined variable 'square'`), and names a module imports stay in that module.
- A denied field access names the rule instead of `field access denied` or `static field access denied`: `cannot read protected field 'name' of B outside its class`, `cannot assign to private static field 'count' of Counter outside its class`, `cannot read internal field 'f' of Ex2 outside its module`.
- Field visibility is checked against the class that declares the field, also on instances of subclasses: a subclass's methods can no longer use a `private` field of its parent.
- `abstract fn` in a class that is not `abstract` is a compile error.
- A class with an `internal` field can be created and used from another module: `internal` means visible within the declaring module, and the class's constructor and methods run in it.
- Inherited `protected` fields can no longer be read or assigned from outside the class hierarchy.
- Creating a class that does not implement an abstract method of its parent throws `cannot create B: it does not implement abstract method area`, also when constructed with named arguments; constructing an abstract class with named arguments is rejected too.
- `static const` fields cannot be reassigned (`cannot assign to constant MathUtils.PI`), and instance `const` fields can be assigned only by a constructor.
- `let mut y = x;` made `y` share `x`'s storage, so assigning either changed both (in every scope, for numbers and strings alike).
- `CSV.parse` errors no longer repeat their prefix (`CSV error: CSV error: ...`).
- `XML.parse` throws on input without a complete root element: `XML parse error: no root element` for an empty string (or only comments or text) and `XML parse error: element <a> is not closed` for `<a>`. Before, it returned `null`.
- The HTTP server answers `HEAD` on a `get` route like `GET` (same status and headers, `Content-Length` included) without a body; before, it answered 404. Tests now serve real requests to a running server.
- The formatter changed programs and moved comments: `else if` became `else { if ... }`, annotations on methods (`@override`) and the `out`/`in` of generic parameters were dropped, and comments inside a multi-line call, list, method chain, `match` or `enum` moved below the statement (a doc comment on an enum variant ended up on the next declaration). Comments now stay next to the element they belong to.

### Documentation

- The language reference (`docs/specs/`) describes only what Pleno implements and is the basis of the 1.0 specification. Claims that were not true were corrected, among them: `@pure` is a partial runtime check (now marked experimental), `@override`, `@sealed` and `@deprecated` have no effect, generic constraints are checked only with explicit type arguments, strings cannot be iterated, and `pleno check src/`, `lint --fix`, `format --check`, `test --coverage`, `doc --serve`, `run --timeout` and the `--verbose`/`--quiet`/`--color` options do not exist. New: truthiness rules, global functions, a collection method reference and a known-bug note on string positions.
- New page: the [1.x compatibility promise](https://pleno.mohirlab.uz/specs/compatibility), saying what 1.x guarantees and what it does not (bytecode format, error wording, performance, experimental APIs).
- The standard library list marks every module and API as stable or experimental. `std:net`, `std:security`, `std:gc` and `std:db` are experimental, as are the `std:http` server, `Crypto.jwtSign`/`jwtVerify` and `vmStats`.
- Designs that are not implemented moved from `notes/specs.md` and `notes/v2-specs.md` to `notes/v2-draft.md`. The performance, bytecode VM, distribution and release pages are labelled as implementation or planning notes.

## 0.2.0 - 2026-10-24

### Changed

- Reading an undefined name is an error (`undefined variable 'foo'`) instead of `null`, and `pleno check` reports undefined names.
- An unhandled promise rejection ends the program with an error instead of a warning.
- A failing `std:test` test fails `pleno test`, `plm test` and `pleno run`.
- Relative imports resolve from the importing file's folder, not the working directory.
- `plm publish` and `plm search` need `PLENO_REGISTRY`; there is no default registry.
- Uncaught exceptions print their message instead of the error map.
- Division and modulo by zero are errors for every Int and Float combination.
- A program whose top-level `await` can never finish ends with an error instead of exiting silently.
- Calling a method a map does not have names the method (`Map has no method 'contains'`) instead of `Null is not callable`.

### Added

- Backtick strings, generic type aliases, `extend <T> Array<T>`, enum methods after `;`, and `return` in match arms.
- `Result` and `Option` type annotations.
- Every standard library module can be imported as a namespace (`import { Path } from "std:path"`), plus `std:core` and `std:promise`.
- `DateTime` and `Duration`, `JSON.stringifyPretty`, `Regex.split`, `Benchmark.measure`, `Log.setLevel`, prefixed loggers, and collection helpers.
- `pleno check --json`; colored output on terminals (`NO_COLOR` and `FORCE_COLOR` are respected).
- VS Code extension 0.2.0, which can install the toolchain.
- A new documentation site with a tutorial and a playground that runs Pleno in the browser (WebAssembly). Every example in the docs is checked in CI.

### Fixed

- Infinity and NaN, 64-bit integers, enum type annotations, typed `catch` branches, contravariant interfaces, the pipeline operator with arguments, and iterating a channel.
- The formatter keeps comments, escapes and syntax it used to drop or break.
- Crashes on deeply nested code in the language server, and on malformed generic annotations in the compiler.
- A VM crash (index out of bounds) when a plain function returned a call to an async function that was waiting, and async functions called from `map`, `filter` and other built-ins returning `null` instead of their promise.
- `test()` with an `async fn` body reports its result when the body finishes, and a failure after an `await` fails the run.
- `Promise.all` put results in the wrong places, dropped some, and never finished when it mixed promises with plain values.
- Built-in functions called through a value (`xs |> map(f)`) could call the wrong function in the WebAssembly build.
- `std:test` output now goes wherever the program's output goes (the playground, the debugger).
- `Path.resolve` no longer returns `\\?\` paths or mixed separators on Windows; `Math.randomInt(n, n)` returns `n`; `unwrap()` on an `Err` shows the error.

## 0.1.0 - 2026-02-03

First release: the compiler, bytecode VM, standard library, `plm` package manager, language server, REPL and VS Code extension.
