# Packages

A Pleno package is a Git repository with a `pleno.toml` at its top. A version of a package is a Git tag such as `v1.2.0`. There is no central registry and no account to create: you depend on a package by its Git URL, and you publish one by pushing a tag. Go modules and the Swift Package Manager work the same way.

`plm` creates projects, installs dependencies and runs scripts. Every command is also available as `pleno plm <command>`. `plm` runs your own `git`, so Git must be installed.

## pleno.toml

`plm init` (or `plm new <folder>`) writes a manifest like this:

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

## Adding a dependency

```bash
plm add github:ada/greet          # the newest release, written as ^X.Y.Z
plm add github:ada/greet@^1.2     # any 1.x from 1.2.0 on
plm add https://gitlab.com/team/json-utils
plm add git@github.com:ada/private-lib.git --tag v2.0.0
plm add ../greet                  # a folder on your machine
plm add github:ada/test-kit --dev # a dev-dependency
```

`plm add` writes the dependency into `pleno.toml`, installs it into `packages/` and records it in `pleno.lock`. If something fails, `pleno.toml` is left as it was. The dependency is named after the repository (`greet`); `--name` chooses another name.

The entries `plm add` writes look like this, and you can also write them by hand and run `plm install`:

```toml
[dependencies]
greet = { git = "https://github.com/ada/greet", version = "^1.2" }
json = { git = "https://gitlab.com/team/json-utils", tag = "v0.4.1" }
nightly = { git = "https://github.com/ada/nightly", branch = "main" }
fixed = { git = "https://github.com/ada/fixed", rev = "3f2c9e1d7a" }
local = { path = "../local" }

[dev-dependencies]
test-kit = { git = "https://github.com/ada/test-kit", version = "^0.3" }
```

- `version` is a range over the repository's tags: plm picks the highest tag `vX.Y.Z` that matches. `^1.2` means at least 1.2.0 and below 2.0.0; `~1.2.3` means at least 1.2.3 and below 1.3.0; `=1.4.0` means exactly 1.4.0; `>=1.0, <1.5` combines two bounds. Pre-releases such as `v2.0.0-beta.1` are only picked when the range names one.
- `tag`, `branch` and `rev` pin one tag, the newest commit of a branch, or one commit. Give at most one of `version`, `tag`, `branch` and `rev`; with none, plm takes the highest release tag.
- `path` uses a folder as it is now.

## Using a dependency

Installed packages are in `packages/`. Import them by name:

```pleno norun
import { hello } from "greet";
print(hello("Ada"));
```

`"greet"` resolves to `packages/greet/`, using the `main` file named in that package's `pleno.toml` (or `lib.pln`). Pleno looks in the importing file's folder and each parent folder, so a package's own imports find the other packages in the same `packages/` folder.

`packages/` is rebuilt by `plm install`; keep it out of Git (`plm init` adds it to `.gitignore`).

## Installing, updating and removing

| Command | What it does |
| --- | --- |
| `plm install` | Installs what `pleno.toml` asks for. Commits already in `pleno.lock` are kept, so a teammate gets exactly your versions. |
| `plm install --offline` | The same, without network: everything must be in `pleno.lock` and in the cache. |
| `plm update` | Chooses again, for every package, the highest tag that `pleno.toml` allows, and updates `pleno.lock`. |
| `plm update greet` | The same for `greet` only (and for what its new version needs). |
| `plm remove greet` | Takes `greet` out of `pleno.toml`, `packages/` and `pleno.lock`. |
| `plm tree` | Prints the dependency tree from `pleno.toml` and `pleno.lock`. |

## pleno.lock

`pleno.lock` records, for every package (dependencies of dependencies included), the exact commit and a checksum of its files. Commit it to your repository: `plm install` then installs the same code on every machine.

```toml
version = 1

[[package]]
name = "greet"
source = "git+https://github.com/ada/greet"
version = "1.2.3"
tag = "v1.2.3"
commit = "3f2c9e1d7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c2d"
checksum = "sha256:9a1f...e04c"
dependencies = ["text-utils"]
```

`plm install` checks the checksum every time. It fails, and installs nothing for that package, when:

- the files of the locked commit do not match the checksum (a corrupted cache, or a changed lockfile);
- the locked commit is no longer in the repository (its tag or branch was force-pushed or deleted);
- `plm update` finds that a tag in the lockfile now points to another commit. A published tag should never move; if you trust the new commit, delete that package's entry from `pleno.lock` and run `plm install`.

## The cache

Each commit is downloaded once per machine, into `~/.pleno/cache/git/` (`$PLENO_HOME/cache` if `PLENO_HOME` is set, or `$PLENO_CACHE_DIR`), and copied from there into each project's `packages/`. plm fetches only the commit it needs (a shallow fetch), and nothing at all when the lockfile and the cache already have it. Deleting the cache is safe; plm downloads again what it needs.

## Private repositories and other hosts

Any Git URL works: GitHub, GitLab, Codeberg, Bitbucket, a server of your own, or `file://` for a repository on disk. For a private repository, plm uses your own Git login, the same way `git clone` does: a credential helper for HTTPS (for GitHub, `gh auth setup-git`), or an SSH key with a `git@host:owner/repo.git` URL. plm never asks for, stores or sends a token itself. If `git ls-remote <url>` works in your terminal, plm can read the repository.

## Publishing a package

1. Put `pleno.toml` at the top of a Git repository and push it (to GitHub, for example).
2. Set `version` in `pleno.toml` to the new version, commit, and run:

```bash
plm publish
```

`plm publish` checks that `pleno.toml` is valid and has no `path` dependencies, that all changes are committed, that tag `vX.Y.Z` does not exist yet, and that the tests in `tests/` pass (skip them with `--no-test`). It then prints the two commands that publish the version:

```bash
git tag -a v1.2.0 -m "greet v1.2.0"
git push origin v1.2.0
```

Run them yourself, or run `plm publish --push` to have plm run them. That is all: from then on, `plm add github:you/greet@^1.2` finds the new version. Never move or delete a published tag; publish a new version instead.

Version numbers follow [semantic versioning](https://semver.org): raise the major number (2.0.0) when you break code that uses your package, the minor number (1.3.0) when you add something, and the patch number (1.2.1) for fixes.

## Finding packages

There is no central index yet. Packages that want to be found add the topic `pleno-package` to their GitHub repository; browse them at [github.com/topics/pleno-package](https://github.com/topics/pleno-package), or run `plm search <words>` for a search link. If you publish a package, add that topic in the repository's settings ("About", then "Topics").

A shared index may come later as a plain Git repository that lists packages and their URLs, updated by pull request, the way Julia's General registry works. It would only help you find packages: installing would still fetch from each package's own repository.
