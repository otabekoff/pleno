# Package Manager (PLM)

PLM is Pleno's package manager. `plm <command>` and `pleno plm <command>` are the same.

Pleno has no central registry. A package is a Git repository, a version is a Git tag, and `plm` finds and fetches packages with the user's own `git`. This page is the reference; [Packages](/guide/packages) is the introduction. The [protocol](#protocol) section says exactly what plm does, so that another tool can read and write the same files.

## Quick start

```bash
plm new my-app && cd my-app
plm add github:ada/greet@^1.2   # add, install and lock a dependency
plm install                     # install what pleno.lock records
plm update                      # move to the newest allowed versions
plm tree                        # show the dependency tree
plm remove greet
plm publish                     # check the package and print the tag commands
```

## pleno.toml

```toml
[package]
name = "my-app"
version = "1.0.0"
description = "My Pleno application"
main = "src/main.pln"

[scripts]
start = "pleno run src/main.pln"
test = "pleno test tests/"

[dependencies]
greet = { git = "https://github.com/ada/greet", version = "^1.2" }
json = { git = "https://gitlab.com/team/json-utils", tag = "v0.4.1" }
nightly = { git = "https://github.com/ada/nightly", branch = "main" }
fixed = { git = "https://github.com/ada/fixed", rev = "3f2c9e1d7a" }
local = { path = "../local" }

[dev-dependencies]
test-kit = { git = "https://github.com/ada/test-kit", version = "^0.3" }

[features]
default = []
extra = ["fancy"]
```

`[package]` takes `name`, `version` (a semantic version such as `1.2.0`), `description` and `main` (default `src/main.pln`); other keys, such as `authors` or `license`, are accepted and ignored.

A dependency is a table with exactly one source:

| Key | Meaning |
| --- | --- |
| `git` | A Git URL: `https://`, `ssh://`, `git@host:owner/repo.git` or `file://`. |
| `path` | A folder, relative to this `pleno.toml`. Only for the project itself and for other path packages: a package fetched from Git cannot have path dependencies. |

and, for `git`, at most one of:

| Key | Meaning |
| --- | --- |
| `version` | A version range over the repository's tags (below). The default, when no key is given, is `"*"`: the highest release. |
| `tag` | Exactly this tag. |
| `branch` | The newest commit of this branch, at the time `pleno.lock` is written. |
| `rev` | Exactly this commit: a full SHA, or at least 7 hex digits of one. |

`optional = true` leaves a dependency out unless a feature in `[features]` names it and that feature is on (`plm install --features extra`, or the `default` feature). A package that depends on yours turns your features on with `features = ["extra"]` in its entry for your package. `[dev-dependencies]` are installed for the project itself, never for projects that depend on it.

The dependency's name (the key) is the folder in `packages/` and the name used in `import ... from "name"`. It may contain letters, digits, `-`, `_` and `.`.

A plain string such as `greet = "^1.2"` is an error: without a registry, a name alone does not say where the package is.

## Version ranges

Ranges follow Cargo's rules:

| Range | Matches |
| --- | --- |
| `^1.5.0` or `1.5.0` | At least 1.5.0, below 2.0.0 |
| `^0.5.2` | At least 0.5.2, below 0.6.0 |
| `~1.5.0` | At least 1.5.0, below 1.6.0 |
| `=1.5.0` | Exactly 1.5.0 |
| `>=1.2, <1.5` | Both bounds |
| `*` | Any release |

A leading `v` is accepted (`v1.2` is `1.2`). A pre-release tag (`v2.0.0-beta.1`) matches only a range that names a pre-release of the same version.

## Commands

| Command | Behavior |
| --- | --- |
| `plm add <source>[@<range>]` | Adds a dependency to `pleno.toml` (keeping its comments), installs it and updates `pleno.lock`. `<source>` is a Git URL, a shorthand `github:owner/repo`, `gitlab:owner/repo` or `codeberg:owner/repo`, a host path `github.com/owner/repo`, or a folder (`./x`, `../x`, an absolute path). Without `@<range>`, `--tag`, `--rev` or `--branch`, plm writes `^X.Y.Z` for the newest release tag. Options: `--name <name>`, `--version <range>`, `--tag <tag>`, `--rev <sha>`, `--branch <branch>`, `--dev`, `--offline`. `pleno.toml` is only changed if the install succeeds. `plm install <source>` does the same. |
| `plm install` | Resolves (keeping every locked commit that still satisfies `pleno.toml`), fetches what is missing from the cache, verifies checksums, fills `packages/` and writes `pleno.lock` if it changed. Options: `--offline`, `--features a,b`. |
| `plm update [name...]` | Resolves again without the lockfile for the named packages, or all of them, so each gets the highest tag its requirements allow. Prints each change (`~ greet v1.2.0 -> v1.3.1`). |
| `plm remove <name>` | Removes the dependency from `pleno.toml`, `packages/` and `pleno.lock`. Packages that only it needed are removed too. |
| `plm tree` (or `plm list`) | Prints the tree from `pleno.toml` and `pleno.lock`, without network. `(*)` marks a package whose dependencies are printed above. |
| `plm publish` | Checks the package and prints the commands that publish it; `--push` runs them, `--no-test` skips the tests. See [Publishing](#publishing). |
| `plm search [words]` | Prints a link to GitHub repositories with the `pleno-package` topic. |
| `plm run <script>` | Runs a script from `[scripts]`. |
| `plm build`, `plm test`, `plm info` | Build, test or describe the package. |

Errors say what to do. The common ones:

| Situation | Message (shortened) |
| --- | --- |
| The URL is wrong | `repository <url> was not found. Check the URL; a private repository also needs your git credentials` |
| A private repository without credentials | `git has no credentials for it ... set up a credential helper (gh auth setup-git) or an SSH key` |
| No tag matches | `no version of 'greet' matches ^3 (from pleno.toml): <url> has v2.1.0, v2.0.0, v1.4.2` |
| A repository has no release tags | `<url> has no release tags (vX.Y.Z) ... plm add <url> --rev <sha> or --branch main` |
| Two packages need incompatible versions | `no version of 'common' matches ^1 (from a v1.0.0) and ^2 (from b v1.0.0)` |
| `--offline` and something is missing | `'greet' needs the network: it is not in pleno.lock` |
| Files do not match the lockfile | `checksum mismatch for 'greet' at commit 3f2c9e1...` |
| A locked tag was moved | `tag v1.2.0 of <url> now points to commit ..., but pleno.lock has ...` |

## Resolution

Each package has one version in a project, because `packages/` is flat and `import ... from "greet"` must mean the same package in every file. plm gathers every requirement on a package (from your `pleno.toml` and from the `pleno.toml` of each package that depends on it, at the version chosen for that package) and picks:

- for ranges: the **highest tag that satisfies every range**;
- for a `tag`, `rev` or `branch` pin: that commit. A tag pin whose version a range does not match, or two different pins, are a conflict.

A requirement in your own `pleno.toml` that pins a package (`tag`, `rev`, `branch` or `path`) wins over every other requirement on it. That is how you settle a conflict between two dependencies: pin the shared package yourself. Every package must come from one place: the same name with two different URLs is a conflict, and so is the same name as a Git and as a path dependency (unless your own `pleno.toml` pins it).

plm repeats these steps until nothing changes (a newly chosen version can bring new requirements). It does not backtrack: it never picks an older version of one package to make room for another; when no version fits it reports who asked for what.

With a lockfile, a locked commit is kept while it still satisfies the requirements, so `plm install` changes nothing. `plm update` drops the lock for the packages it updates.

Why highest-compatible and not Go's minimal version selection: Pleno's ranges can have upper bounds (`<1.5`, `~1.2`), and a lockfile already makes builds repeatable, which is what minimal version selection is for in Go. Picking the highest compatible tag gives you bug fixes when you run `plm update` and nothing changes until then. This is what Cargo and npm do, without their ability to install two versions of one package.

## Lockfile

`pleno.lock` is TOML, written by plm in a fixed order. Commit it.

```toml
# Written by plm. Do not edit it by hand: run `plm install` or `plm update`.

version = 1

[[package]]
name = "greet"
source = "git+https://github.com/ada/greet"
version = "1.2.3"
tag = "v1.2.3"
commit = "3f2c9e1d7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c2d"
checksum = "sha256:9a1f0c...e04c"
dependencies = ["text-utils"]

[[package]]
name = "nightly"
source = "git+https://github.com/ada/nightly"
branch = "main"
commit = "77aa01c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8"
checksum = "sha256:41d2bb...7a10"

[[package]]
name = "local"
source = "path+../local"
```

| Field | Meaning |
| --- | --- |
| `version` (top) | The lockfile format, `1`. A lockfile without it (from an older plm) is ignored and written again. |
| `name` | The dependency name. |
| `source` | `git+<url>` as written in `pleno.toml`, or `path+<folder>` relative to the project. |
| `version`, `tag` | The tag that was chosen and its version (no `version` for a tag that is not a version). |
| `branch` | The branch followed, for a `branch` dependency. |
| `commit` | The full commit SHA. |
| `checksum` | The checksum of the package's files at that commit (below). |
| `dependencies` | The names of the packages this one depends on. |

Path packages have no commit and no checksum: they are copied as they are.

## Cache

The cache is `$PLENO_CACHE_DIR`, or `$PLENO_HOME/cache`, or `~/.pleno/cache`. The files of a commit are kept in `git/<repository>-<hash>/<commit>/`, where `<hash>` is the first 16 hex digits of the SHA-256 of the normalized URL. A commit folder is either complete or absent. `plm install` copies from the cache into `packages/<name>/`, and checks the checksum of the cached files each time.

## Protocol

This is what plm does, so that another tool can do the same.

**Normalized URL.** To compare two URLs, remove a trailing `/` and `.git`, and lower-case the scheme and host: `https://GitHub.com/Ada/Greet.git` and `https://github.com/Ada/Greet` are one repository.

**Versions.** The versions of a package are the tags of its repository that are semantic versions, with or without a leading `v` (`v1.2.0`, `1.2.0`). When both spellings exist for one version, `v1.2.0` wins. An annotated tag stands for the commit it points to. Tags are listed with `git ls-remote --tags --heads <url>`.

**Fetching a commit.** plm creates a scratch bare repository and runs `git fetch --depth 1 --no-tags <url> <commit>`. If the server refuses to send a commit by SHA, plm fetches the tag or branch it came from, then all branches and tags; if the commit is still missing, it is an error. The files are taken with `git -c core.autocrlf=false -c core.eol=lf archive --format=tar <commit>`: the files as stored in Git, with no line-ending conversion, so they are identical on every OS. Only regular files are kept; symbolic links, folders and file modes are dropped. Attributes such as `export-ignore` in the package's `.gitattributes` apply.

**Checksum.** For each regular file, sorted by its path relative to the package root (with `/` separators, compared as bytes), take the line

```
<lowercase hex SHA-256 of the file's bytes><two spaces><path>\n
```

The checksum is `sha256:` followed by the lowercase hex SHA-256 of all lines joined. This is Go's `h1:` directory hash with hex instead of base64. An empty package hashes the empty string.

**Credentials.** plm runs `git` with the user's configuration and environment, so credential helpers, SSH keys and `url.<base>.insteadOf` rules apply. When standard input is not a terminal it sets `GIT_TERMINAL_PROMPT=0`, so a missing password fails instead of waiting. plm never stores credentials.

**A package.** The repository's top folder is the package. Its `pleno.toml` is optional for a dependency: without one, the package has no dependencies and is imported through `lib.pln`.

## Publishing

Publishing a version is pushing the tag `v<version>`:

```bash
plm publish          # checks, then prints the two commands
git tag -a v1.2.0 -m "greet v1.2.0"
git push origin v1.2.0
```

`plm publish` checks that:

- `pleno.toml` is valid and its `version` is a semantic version;
- no `[dependencies]` entry is a `path` (others could not install it);
- `pleno.toml` is at the top of a Git repository with at least one commit;
- tracked files have no uncommitted changes;
- the tag `v<version>` exists neither locally nor on the remote;
- the tests in `tests/` pass (`--no-test` skips them).

It warns when the version is lower than an existing tag, when there is no remote, and when the repository's name differs from the package name. With `--push` it runs `git tag -a` and `git push` itself. A published tag must never move: users' lockfiles record its commit, and `plm update` refuses a tag that moved.

## Finding packages

Add the GitHub topic `pleno-package` to a package's repository so people can find it at [github.com/topics/pleno-package](https://github.com/topics/pleno-package). `plm search <words>` prints a search link.

A future index could be a plain Git repository (say `pleno-lang/packages`) with one file per package giving its name, URL and description, updated by pull request, like Julia's General registry. Tools would read it to search and to turn a short name into a URL; versions would still be the tags of each package's repository, and nothing would need a server.
