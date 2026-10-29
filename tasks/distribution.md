# Distribution: what is left

Everything about getting Pleno to people: releases, signing, package managers and the VS Code
extension stores. It is one of the 1.0 must-haves in [`v1.md`](v1.md) ("You can install Pleno
with one command on Windows, macOS and Linux"). As in the other task lists, an item that is
completely done is removed and logged in [`done.md`](done.md).

How-to guides, with exact steps, links and commands:

- [`packaging/README.md`](../packaging/README.md): the package managers and the release
  workflow's secrets (Homebrew, Scoop, WinGet, Chocolatey, AUR).
- [`ide/vscode/PUBLISHING.md`](../ide/vscode/PUBLISHING.md): the VS Code Marketplace and Open VSX.

Status checked on 2026-10-29, after the 0.4.0 release.

## Where each channel stands

| Channel                                                                      | Version there                   | Updated on release                                                     | Left                                       |
| ---------------------------------------------------------------------------- | ------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------ |
| GitHub release (6 targets, `.deb`, `.vsix`, Docker image, cosign signatures) | 0.4.0                           | Yes                                                                    | Windows signing (below)                    |
| Install scripts (`scripts/install.sh`, `install.ps1`)                        | latest release                  | Yes                                                                    | —                                          |
| Homebrew (`otabekoff/homebrew-tap`)                                          | 0.4.0                           | Yes                                                                    | —                                          |
| Scoop (`otabekoff/scoop-bucket`)                                             | 0.4.0                           | Yes                                                                    | —                                          |
| WinGet (`microsoft/winget-pkgs`)                                             | not listed yet                  | After the first version is accepted (the `WINGET_TOKEN` secret is set) | First version accepted                     |
| VS Code Marketplace                                                          | not published                   | Yes, once the `VSCE_PAT` secret is set                                 | Publisher and token                        |
| Open VSX                                                                     | not published                   | Yes, once trusted publishing is set up (skipped for 0.4.0)             | Trusted publisher, variable, first publish |
| Chocolatey                                                                   | not published                   | No                                                                     | Account, first push, maybe automation      |
| AUR (`pleno-bin`)                                                            | not published                   | No                                                                     | Account, first push                        |
| apt (Debian, Ubuntu)                                                         | `.deb` on the release page only | No                                                                     | An apt repository                          |
| Snap, Flatpak, asdf, RPM                                                     | —                               | —                                                                      | Not planned for 1.0 (see "After 1.0")      |

## 0.5.0: more binaries, platforms and devices

0.5.0 is the delivery release. It covers the store and account steps below ("Must have for 1.0"
and "Should have") and this build work. Each new target also needs a line in
`ide/vscode/src/toolchain.ts` (the extension's "Install Toolchain"), the install scripts and
`docs/guide/installation.md`.

Released in 0.4.0: Linux glibc 2.17+ (x86_64, aarch64, i686), macOS (x86_64, aarch64), Windows
x86_64 (ARM64 runs it under emulation), the `.deb` (amd64), the `.vsix`, and the Docker image
(amd64, arm64). Each comes with a `.sha256`, `SHA256SUMS.txt` and a cosign bundle.

- [ ] **Windows on ARM64** (`aarch64-pc-windows-msvc`), built natively on the free `windows-11-arm`
      runner. Snapdragon laptops run it without emulation. Also a second installer in the WinGet
      manifest, and `install.ps1` choosing it on ARM64.
- [ ] **Static Linux builds with musl** (`x86_64-` and `aarch64-unknown-linux-musl`, with
      cargo-zigbuild as now). They run on Alpine, in `scratch` or distroless containers, and in
      Termux on Android. `install.sh` picks musl when `ldd --version` says musl.
- [ ] **32-bit ARM Linux** (`armv7-unknown-linux-gnueabihf`): Raspberry Pi OS 32-bit and older
      boards.
- [ ] **More Linux packages**: the `.deb` for arm64 (and i386), and `.rpm` for x86_64 and aarch64
      (cargo-generate-rpm), so Fedora, RHEL and openSUSE install with `dnf`/`zypper`.
- [ ] **Package repositories**: the signed apt repository (below) and an rpm repository, both on
      GitHub Pages, built by the release workflow with the GPG key already stored as a secret.
- [ ] **Archives with LICENSE and README**, as Homebrew, Debian and Chocolatey reviewers expect.
- [ ] **`pleno upgrade`**: updates an install in `~/.pleno/bin` to the latest release, as
      `scripts/release-install.*` does. For an install made by a package manager, it names the
      command to use instead (`brew upgrade pleno`, `scoop update pleno`, `winget upgrade Pleno.Pleno`).
- [ ] **Build provenance**: `actions/attest-build-provenance` in `release.yml`, so
      `gh attestation verify <file> -R otabekoff/pleno` works next to cosign.
- [ ] **`packaging/` by pull request**: a release job that runs `scripts/update-packaging.sh` and
      opens the pull request. It needs "Allow GitHub Actions to create pull requests" in the
      repository settings.
- [ ] **The Docker image on a smaller base**: the binaries need only glibc 2.17 now, so
      `Dockerfile.release`'s note about glibc 2.39 and its `debian:trixie-slim` base are out of
      date. With the musl build, the image can be distroless.
- [ ] **WebAssembly**: an npm package of `crates/pleno-wasm`, for browsers, Node and Deno (see
      [`later.md`](later.md)), and, if `pleno` builds for `wasm32-wasip1`, a WASI binary for
      wasmtime and other WASI hosts.
- Not planned: macOS notarization and the Apple Developer Program (paid; `curl` downloads and
  Homebrew installs are not quarantined), iOS, MSI installers (WinGet, Scoop and the zip cover
  Windows), and Homebrew core (it requires a notable, widely used project).

## Must have for 1.0

- [ ] **WinGet: first version accepted.** Status: microsoft/winget-pkgs#460928 adds `Pleno.Pleno`
      0.4.0 (moved from 0.3.0 on 2026-10-29; `winget validate` passes). The CLA is signed, and it
      is waiting for a volunteer moderator (since 2026-10-24). Missing: a moderator's approval.
      Answer any review comment there. After that, every release opens its WinGet pull request by
      itself.
- [ ] **VS Code Marketplace.** Status: the extension ID `pleno.pleno-language` and the publisher
      name `pleno` are free, and the publishing workflow (`publish-extension.yml`, #163) is
      merged. Missing:
  - an Azure DevOps token with **Marketplace → Manage**, saved as the `VSCE_PAT` secret;
  - the `pleno` publisher at marketplace.visualstudio.com/manage;
  - the first upload: run the Publish Extension workflow for v0.4.0.
    Steps: `ide/vscode/PUBLISHING.md` → "VS Code Marketplace".
- [ ] **Open VSX.** Status: the `pleno` namespace exists (created 2026-10-27) and the publishing
      workflow is merged. Missing:
  - the Eclipse Contributor Agreement signed, with GitHub Username `otabekoff` in the Eclipse
    account;
  - the trusted publisher registered for `otabekoff/pleno`, workflow `publish-extension.yml`,
    no environment;
  - the repository variable `OPEN_VSX_PUBLISH` set to `true`;
  - the first upload: run the Publish Extension workflow for v0.4.0.
    Steps: `ide/vscode/PUBLISHING.md` → "Open VSX".
- [ ] **Docs once the extension is in the stores.** Status: `docs/guide/editor.md`,
      `docs/guide/installation.md`, `docs/specs/release.md` and `ide/vscode/README.md` say to install
      the `.vsix` from a release, and `docs/community.md` says the extension is not published on
      purpose. Missing: store links and `code --install-extension pleno.pleno-language` in those pages.
- [ ] **Signed Windows binaries.** Status: Windows downloads are not code-signed, so SmartScreen
      warns on first run. The SignPath Foundation application is written in
      [`notes/SIGNPATH_APPLICATION.md`](../notes/SIGNPATH_APPLICATION.md) and waits for approval.
      Missing: approval, then a signing step in `release.yml` before the Windows zip is packed.
- [ ] **Releases 1.0.0-rc.1 and 1.0.0.** Status: 0.4.0 is the latest release (2026-10-29). The
      release workflow builds, signs (cosign) and publishes everything above. Missing: tag
      `v1.0.0-rc.1` once the other must-haves in [`v1.md`](v1.md) are done. A prerelease tag (with a
      `-`) skips the package managers and stores, so it is safe to test with. Then tag `v1.0.0`.

## Should have (1.0 or soon after)

- [ ] **Chocolatey.** Status: `packaging/chocolatey/` holds the 0.3.0 package, and the ID `pleno`
      was free on 2026-10-27. Missing: an account at community.chocolatey.org and the first
      `choco push`, which goes through moderation (days for the first versions). Then optionally a
      release job with a `CHOCO_API_KEY` secret. Steps: `packaging/README.md` → "Chocolatey".
- [ ] **AUR.** Status: `packaging/aur/pleno-bin/` holds the 0.3.0 `PKGBUILD` and `.SRCINFO`, and
      the name `pleno-bin` was free on 2026-10-27. Missing: an AUR account with an SSH key, and the
      first push. Steps: `packaging/README.md` → "AUR".
- [ ] **apt repository.** Status: each release has a `.deb` (`pleno_vX.Y.Z_amd64.deb`), installed
      with `sudo apt install ./pleno_*.deb`, but there is no apt repository, so `apt upgrade` does not
      update Pleno. Missing: a signed repository (for example on GitHub Pages, built by the release
      workflow with the existing GPG key, or a hosted service), and an install line in
      `docs/guide/installation.md`.
- [ ] **Open VSX verified namespace.** Status: optional; it only adds a "verified" badge. Missing:
      once the extension is on the Marketplace, file a claim at
      [EclipseFdn/open-vsx.org](https://github.com/EclipseFdn/open-vsx.org/issues/new?template=claim-namespace-ownership.yml&namespace=pleno&title=Claiming%20namespace%20%60pleno%60)
      with **Option 1**, using the extension's repository `github.com/otabekoff/pleno` as evidence.

## After 1.0

- Snap, Flatpak, an RPM repository, asdf and mise plugins (`packaging/snap/`, `flatpak/` and
  `asdf/` are placeholders).
- A JetBrains plugin (the `docs/specs/release.md` plan mentions one).
