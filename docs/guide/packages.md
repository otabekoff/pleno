# Packages

`plm` creates projects, installs dependencies and runs scripts. Every command is also available as `pleno plm <command>`.

## pleno.toml

`plm init` writes a manifest like this:

```toml
[package]
name = "my-app"
version = "0.1.0"
main = "src/main.pln"

[scripts]
dev = "pleno run src/main.pln"
test = "plm test"

[dependencies]
```

`plm run <script>` runs a command from `[scripts]`.

## Dependencies

Add dependencies under `[dependencies]` and run `plm install`:

```toml
[dependencies]
# Another package on disk
greet = { path = "../greet" }
# A Git repository, at a tag, branch or commit
http-utils = { git = "https://github.com/user/http-utils", tag = "v1.2.0" }
```

`plm install <source>` adds and installs one in a step: `plm install github.com/user/http-utils@v1.2.0`. `plm remove <name>` takes it out, and `plm update` fetches newer versions.

Installed packages go into `packages/`. Import them by name:

```pleno
import { hello } from "greet";
print(hello("Ada"));
```

A bare name resolves to `packages/<name>/`, using the `main` file named in that package's `pleno.toml` (or `lib.pln`), and Pleno looks in the importing file's folder and each parent folder.

## Registry

There is no public package registry yet. `plm publish` and `plm search` work with a registry you run yourself (`crates/pleno-registry`): set `PLENO_REGISTRY` to its URL. Until a public one exists, share packages through Git.
