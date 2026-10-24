# Getting Started with Pleno

This walkthrough sets up the toolchain and runs your first Pleno program.

## 1. Install the Toolchain

Run **Pleno: Install or Update Toolchain** from the Command Palette. It downloads the latest release for your system from GitHub, checks its SHA-256 checksum, and puts `pleno`, `plm`, `pleno-lsp`, `pleno-dap` and `pleno-repl` on PATH in VS Code's terminals.

[Install the Toolchain](command:pleno.installToolchain)

Prefer a system-wide install? See the [installation instructions](https://github.com/otabekoff/pleno#installation).

## 2. Create a Project

Open an empty folder and run **Pleno: Create New Project**, or in a terminal:

```bash
plm init my-project
```

## 3. Your First Program

Create `hello.pln`:

```pleno
fn greet(name: String): String {
    return "Hello, ${name}!";
}

print(greet("Pleno"));
```

## 4. Run and Debug

Click **Run** in the editor title bar, or run `pleno run hello.pln`. Press `F5` to debug with breakpoints.

Errors appear in the **Problems** panel as you type.

## 5. Learn More

Read the [documentation](https://otabekoff.github.io/pleno/) for the language guide and the standard library.
