# pleno-test-app

Welcome to your new Pleno project!

## About Pleno

Pleno is a dynamic, type-safe programming language designed for production. It combines modern features with a focus on stability and ease of use.

## Getting Started

The entry point of this project is [`src/main.pln`](./src/main.pln).

### Prerequisites

Ensure you have the Pleno toolchain installed. You can verify this by running:

```bash
pleno --version
plm --version
```

### Commands

This project includes a `pleno.toml` file with standard scripts for your convenience:

- **Run in Development**:
  ```bash
  plm run dev
  ```
  Or using the default task:
  ```bash
  plm run
  ```

- **Build**:
  ```bash
  plm build          # Default (Debug)
  plm build --debug  # Debug profile
  plm build --release # Release profile
  ```

- **Test**:
  ```bash
  plm test
  ```
  Runs all tests in the project (including the one in `src/main.pln`).

- **Check**:
  ```bash
  plm run check
  ```
  Checks the syntax and semantics of `src/main.pln`.

- **REPL**:
  ```bash
  plm run repl
  ```
  Starts the Pleno Read-Eval-Print Loop.

## Documentation

For more information about the Pleno programming language, please visit the [official documentation](https://pleno-lang.org/docs). // Replace with actual URL if different
