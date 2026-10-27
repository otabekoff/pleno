# Introduction

Pleno is a dynamically typed language with optional type annotations that are checked when the program runs. It compiles to bytecode for a register-based virtual machine written in Rust, and comes with a package manager, a formatter, a linter, a test runner, a language server, a debugger and a VS Code extension.

```pleno
class Account {
    private balance: Float = 0.0;

    fn deposit(amount: Float) {
        if (amount <= 0.0) {
            throw Error("deposit must be positive");
        }
        this.balance += amount;
    }

    fn total(): Float {
        return this.balance;
    }
}

let account = Account();
account.deposit(25.0);
print(`Balance: ${account.total()}`);
```

## What it offers

- **Annotations you can add gradually.** `let x = 5` works; `let x: Int = 5` is checked, and so are parameters, return values, fields, generics with `in`/`out` variance, `Result` and `Option`.
- **Several styles in one language.** Classes with real `private` fields, interfaces, enums with data and methods, pattern matching, closures, the pipeline operator and partial application.
- **Async built in.** `async`/`await`, promises, `spawn`, channels and mutexes.
- **A standard library for everyday programs.** Files, paths, processes, HTTP client and server, JSON, YAML, TOML, XML, CSV, regular expressions, dates, cryptography, SQLite, logging and testing.
- **Tools that come together.** `pleno` runs, checks, formats, lints and tests; `plm` creates projects and manages dependencies; the VS Code extension installs everything.

## Status

Pleno is at version 0.3, the last step before 1.0. The language can still change before 1.0, and the [changelog](https://github.com/otabekoff/pleno/blob/main/CHANGELOG.md) lists every change. The [1.0 plan](https://github.com/otabekoff/pleno/blob/main/tasks/v1.md) lists what is left.

- **Speed:** the interpreter is faster than CPython 3.14 on every benchmark in the repository (recursive `fib(30)`: 0.09 s against 0.10 s; a million-step loop: 0.03 s against 0.08 s).
- **Static checking:** `pleno check` and the editor find syntax and compile errors, undefined names, wrong argument counts, unknown methods and literals that contradict their annotations, but most type errors appear only when the code runs.
- **Packages** are Git repositories versioned by tags, with a lockfile. There is no central registry, by design.
- The [language reference](/specs/) describes only what exists; designs for later versions are in the [version 2 draft](https://github.com/otabekoff/pleno/blob/main/notes/v2-draft.md), and the [compatibility promise](/specs/compatibility) says what 1.x will guarantee.

Ready to try it? [Install Pleno](./installation).
