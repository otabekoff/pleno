# Changelog

Notable changes to Pleno. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Before 1.0, any release may change behavior.

## Unreleased

### Added

- `vmStats()` in `std:reflect`: instructions executed, current and peak memory.
- `Error` is a class: `Error(message)` instances print as `Error: message` and have `toString()`; subclasses get their class name as `name`.
- Destructuring in `let`: `let (a, b) = pair;`, `let mut (x, (y, _)) = nested;` and `let [first, ...rest] = items;`, with runtime errors such as `tuple has 2 elements, pattern expects 3`. Tuple type annotations such as `(Int, String)` and `(Int, Int)[]` are checked at runtime.
- `pleno check` and the language server report wrong argument counts and unknown named arguments for functions, constructors and `this` methods declared in the file, literals that cannot match a type annotation (`let x: Int = "hi";`), and, as a warning, unknown methods called on `this`.
- `pleno run file.plc` runs bytecode written by `pleno compile`.
- Return type annotations are checked at runtime, at every `return` and when a function ends without one: `fn f(): Int { return "s"; }` throws `TypeError: f returned String, expected Int`, and falling off the end of a function annotated `Int` throws `f returned Null, expected Int`. This covers functions, methods, getters, lambdas and `async` functions (whose annotation, `Int` or `Promise<Int>`, describes the resolved value). `Void`, `Any` and annotations mentioning a type parameter are not checked. `pleno check` also reports returned literals that cannot match (`f is declared to return Int but returns a String literal`).

### Changed

- Every caught error is an `Error` instance with `name` and `message`: runtime errors, failed type annotations (`TypeError`), denied operations (`SecurityError`) and sandbox limits (`SandboxError`) included, so `catch (e: Error)` catches them and `e.message` always works.
- Printing an error or putting it in a string gives `name: message`, such as `TypeError: parameter 'x' expected Int, got String`, instead of a map.
- Caught errors no longer carry `peakMemory` and `instructions`; `vmStats()` from `std:reflect` returns them.
- Ints and Floats compare by value: `5.0 == 5` is true, so `assert.equal(10 / 2, 5)` passes. Sets, `contains` and `indexOf` follow the same rule (`Set([5, 5.0])` has one element).
- `.plc` files start with a `PLNC` magic number and a bytecode format version, checked on load; files from another Pleno version, older headerless files and truncated files are rejected with an error asking to recompile.
- An `Int` is accepted where a `Float` is annotated and is converted to a `Float` there (parameters, `let`, assignments, fields, return values, `Float?`, unions such as `Float | String`); before, `fn f(x: Float) {} f(3);` and `let y: Float = 3;` threw a `TypeError`. In a union that allows `Int` (`Int | Float`, `Number`) an `Int` stays an `Int`, and `Float[]` accepts `Int` elements without converting them. `pleno check` now reports a `Float` literal given for an `Int` annotation.
- Reassigning a `&mut` parameter (`value = ...`, `value++`) is a compile error, `cannot assign to '&mut' parameter 'value'`: the new value never reached the caller's variable. Changing a value in place (`items.push(x)`) is unchanged.
- The bytecode format version is 2 (a new `ToFloat` instruction); recompile `.plc` files.

### Fixed

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
