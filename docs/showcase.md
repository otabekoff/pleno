# Showcase

Programs and tools built with Pleno, and the tools built for it.

## Programs in the repository

These run with `pleno run` and are tested in CI.

| Program | What it shows |
| --- | --- |
| [`examples/web_app/app.pln`](https://github.com/otabekoff/pleno/blob/main/examples/web_app/app.pln) | A web app with `std:http`: middleware, static files, cookies and an error handler. |
| [`examples/net_echo.pln`](https://github.com/otabekoff/pleno/blob/main/examples/net_echo.pln) | A TCP echo server and its client in one program, with `std:net` and `async`/`await`. |
| [`examples/todo_app.pln`](https://github.com/otabekoff/pleno/blob/main/examples/todo_app.pln) | A command-line todo list that saves its tasks as JSON. |
| [`examples/concurrency.pln`](https://github.com/otabekoff/pleno/blob/main/examples/concurrency.pln) | Tasks, channels and mutexes. |

The [examples page](/examples) has shorter programs you can run in the browser.

## Tools for Pleno

| Tool | What it does | Docs |
| --- | --- | --- |
| VS Code extension | Highlighting, the language server, the debugger, a test view, and installing the toolchain. | [Editor setup](/guide/editor) |
| `plm` | Creates projects and installs their dependencies from Git repositories. | [Packages](/guide/packages) |
| `pleno-lsp` | The language server: errors as you type, completion, go to definition, rename. | [CLI and tools](/specs/tooling-cli) |
| `pleno-dap` | The debugger, over the Debug Adapter Protocol. | [CLI and tools](/specs/tooling-cli) |

## Community projects

None are listed yet. If you have built a program or a library with Pleno, add it to this page: edit [`docs/showcase.md`](https://github.com/otabekoff/pleno/edit/main/docs/showcase.md) and open a pull request with a link and one line about what it does. Libraries can also use the GitHub topic [`pleno-package`](https://github.com/topics/pleno-package), which `plm search` points to.
