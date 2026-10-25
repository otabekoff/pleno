# Create a project

**Pleno: New Project...** runs `plm init` in a folder you pick. The same in a terminal:

```sh
mkdir my-app
cd my-app
plm init my-app
```

It creates:

- `pleno.toml`: the package name, version, entry file, scripts and dependencies.
- `src/main.pln`: the entry file.
- `tests/integration_test.pln`: a first test.

When the folder has a `pleno.toml`, the **Pleno Project** view in the Explorer lists its scripts
and dependencies. Click the play button next to a script to run it with `plm run`. The scripts,
`plm build`, `plm test`, `plm install` and `plm update` are also in **Tasks: Run Task**.
