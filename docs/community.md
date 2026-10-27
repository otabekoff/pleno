# Community

Pleno is an open-source language, built in the open on [GitHub](https://github.com/otabekoff/pleno). It is maintained by one person, [Otabek Ismoilov](https://github.com/otabekoff), in spare time, so the project is kept small on purpose: it avoids anything that costs money or needs constant care, and spends the time on the language.

Help is welcome if you are interested in:

- programming language design;
- virtual machines, compilers and runtimes;
- tools: the language server, debugger, formatter and linter;
- documentation, examples and lessons;
- writing programs and libraries in Pleno.

## Where to talk

- **Questions, bugs and ideas:** [GitHub issues](https://github.com/otabekoff/pleno/issues). Search first, then open a new one. For a bug, include `pleno --version`, your operating system and the smallest program that shows the problem.
- **Security problems:** do not open a public issue; follow [SECURITY.md](https://github.com/otabekoff/pleno/blob/main/SECURITY.md).
- **News:** the [blog](/blog/) and the [releases](https://github.com/otabekoff/pleno/releases).

Pleno has no chat server, forum or social media accounts yet. Answers can take a few days.

## How the project is run

The maintainer reviews and merges every change and makes the releases. Language changes start as an issue, so they can be discussed before code is written. [GOVERNANCE.md](https://github.com/otabekoff/pleno/blob/main/GOVERNANCE.md) has the details, and everyone follows the [Code of Conduct](https://github.com/otabekoff/pleno/blob/main/CODE_OF_CONDUCT.md).

Things Pleno does not have, by design:

- **A package registry.** Packages are Git repositories, and a version is a Git tag. Anyone can publish, host or fork a package without an account or a server. See [Packages](/guide/packages).
- **Paid infrastructure.** The site, the releases and the package taps run on free services (GitHub Pages, GitHub Releases, GitHub Actions).

Pleno installs with an install script, Homebrew, Scoop or from the release page (see [Installation](/guide/installation)). WinGet is waiting for its first version to be accepted.

## The VS Code extension

The extension is ready to use: download the `.vsix` file from the [latest release](https://github.com/otabekoff/pleno/releases) and install it as described in [Editor setup](/guide/editor). It is not in the VS Code Marketplace or on Open VSX yet. Publishing it there is part of the 1.0 release.

## Get involved

- Fix a bug or add a test: the [contributing guide](/guide/contributing) explains how to build and check Pleno.
- Improve these docs: every page has an **Edit this page on GitHub** link.
- Build something with Pleno and add it to the [showcase](/showcase).

Everyone with a merged change is listed on the [team and contributors](/team) page.
