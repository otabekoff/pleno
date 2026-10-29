# Packaging

Package definitions for each package manager. Every one installs the prebuilt release archives
(`pleno`, `plm`, `pleno-lsp`, `pleno-dap` and `pleno-repl`) and pins their SHA-256 checksums,
taken from the release's `SHA256SUMS.txt`. The VS Code extension is published separately; see
[`ide/vscode/PUBLISHING.md`](../ide/vscode/PUBLISHING.md).

| Directory        | Platform      | Package manager | How it is published                                                     |
| ---------------- | ------------- | --------------- | ----------------------------------------------------------------------- |
| `homebrew/`      | macOS, Linux  | Homebrew        | Automatically: the release workflow pushes it to `otabekoff/homebrew-tap` |
| `scoop/`         | Windows       | Scoop           | Automatically: the release workflow pushes it to `otabekoff/scoop-bucket` |
| `winget/`        | Windows       | WinGet          | Automatically after the first version: the release workflow opens a pull request to `microsoft/winget-pkgs` |
| `chocolatey/`    | Windows       | Chocolatey      | By hand: `choco push` with the maintainer's API key                    |
| `aur/pleno-bin/` | Arch Linux    | AUR             | By hand: pushed to the `pleno-bin` AUR repository                      |
| `snap/`, `flatpak/`, `asdf/` | Linux | —        | Not maintained yet                                                      |

The files in this directory are the source of truth. After a release, point them all at it in one
pull request, so the copies and the published packages agree:

```sh
gh release download v1.2.3 --pattern SHA256SUMS.txt --dir release
scripts/update-packaging.sh 1.2.3 release/SHA256SUMS.txt
git diff -- packaging
```

It sets the version, URLs and checksums in every manifest here (the LICENSE checksum for the AUR
comes from the tag, so fetch the tags first) and WinGet's release date, which is today unless you
pass a date as the third argument. If a checksum is missing it changes nothing.
`scripts/test_update_packaging.sh`, run in CI, checks it.

## Release workflow setup

`.github/workflows/release.yml` publishes to Homebrew, Scoop and WinGet, and the VS Code
extension to its two stores, when a `vX.Y.Z` tag is pushed. It skips each one whose secret is missing, and it skips prereleases (`v1.0.0-rc.1`). Add
the secrets under **Settings → Secrets and variables → Actions** in `otabekoff/pleno`.

| Secret               | Used for | Token |
| -------------------- | -------- | ----- |
| `HOMEBREW_TAP_TOKEN` | Pushing `Formula/pleno.rb` to `otabekoff/homebrew-tap` | Fine-grained token, repository `homebrew-tap`, **Contents: Read and write** |
| `SCOOP_BUCKET_TOKEN` | Pushing `bucket/pleno.json` to `otabekoff/scoop-bucket` | Fine-grained token, repository `scoop-bucket`, **Contents: Read and write** |
| `WINGET_TOKEN`       | `wingetcreate update --submit`: pushes a branch to your `winget-pkgs` fork and opens the pull request | Classic token with the **public_repo** scope (fine-grained tokens cannot open pull requests in another owner's repository) |
| `VSCE_PAT`           | Publishing the `.vsix` to the VS Code Marketplace (`publish-extension.yml`) | Azure DevOps token, **Marketplace: Manage** ([`ide/vscode/PUBLISHING.md`](../ide/vscode/PUBLISHING.md)) |
| `OPEN_VSX_PUBLISH` (a variable, not a secret) | Publishing the `.vsix` to Open VSX by trusted publishing, without a token | `true` once the trusted publisher is registered ([`ide/vscode/PUBLISHING.md`](../ide/vscode/PUBLISHING.md)) |

GitHub has two kinds of personal access token. A **fine-grained** token is limited to the
repositories and permissions you pick; it can last at most a year. A **classic** token applies to
every repository you can access, with broad scopes; it can be set to never expire. Use
fine-grained wherever it works. WinGet needs a classic token, because `wingetcreate` pushes to your
fork and opens a pull request in `microsoft/winget-pkgs`, which a fine-grained token cannot do for
another owner's repository.

### `SCOOP_BUCKET_TOKEN` (fine-grained)

1. Open [github.com/settings/personal-access-tokens/new](https://github.com/settings/personal-access-tokens/new).
2. **Token name:** `pleno release: scoop-bucket`. **Expiration:** the longest offered (custom, up
   to a year). Set a calendar reminder for that date.
3. **Resource owner:** `otabekoff`.
4. **Repository access:** **Only select repositories** → `otabekoff/scoop-bucket`.
5. **Permissions → Repository permissions → Contents: Read and write.** Metadata: Read-only is
   added by itself. Leave everything else at No access.
6. **Generate token** and copy it. GitHub shows it only once.
7. Open [github.com/otabekoff/pleno/settings/secrets/actions/new](https://github.com/otabekoff/pleno/settings/secrets/actions/new),
   **Name:** `SCOOP_BUCKET_TOKEN`, **Secret:** the token, then **Add secret**.

`HOMEBREW_TAP_TOKEN` is the same kind of token for `otabekoff/homebrew-tap`. To use one token for
both, edit it at [github.com/settings/personal-access-tokens](https://github.com/settings/personal-access-tokens),
add `scoop-bucket` under **Repository access**, regenerate it, and save the new value under both
secret names.

### `WINGET_TOKEN` (classic)

1. Fork [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) to your account, if
   `otabekoff/winget-pkgs` does not exist yet. It exists already.
2. Open [github.com/settings/tokens/new?scopes=public_repo&description=pleno%20release%3A%20winget](https://github.com/settings/tokens/new?scopes=public_repo&description=pleno%20release%3A%20winget).
   The link fills in the note and ticks **public_repo** (under **repo**). Tick nothing else.
3. **Expiration:** a year, or **No expiration** if you prefer not to renew it. Then
   **Generate token** and copy it.
4. Open [github.com/otabekoff/pleno/settings/secrets/actions/new](https://github.com/otabekoff/pleno/settings/secrets/actions/new),
   **Name:** `WINGET_TOKEN`, **Secret:** the token, then **Add secret**.

### From the command line instead

With the [GitHub CLI](https://cli.github.com/), `gh secret set` asks for the value without echoing
it, so the token never lands in your shell history:

```sh
gh secret set SCOOP_BUCKET_TOKEN --repo otabekoff/pleno
gh secret set WINGET_TOKEN --repo otabekoff/pleno
gh secret list --repo otabekoff/pleno     # shows the names, never the values
```

When a token expires, its job fails with an authentication error on the next release. Generate a
new token the same way and replace the secret.

## Homebrew

```sh
brew install otabekoff/tap/pleno
```

`scripts/homebrew-formula.sh <version> SHA256SUMS.txt` writes the formula. The release workflow
runs it and pushes the result.

## Scoop

```powershell
scoop bucket add pleno https://github.com/otabekoff/scoop-bucket
scoop install pleno
```

`scoop install pleno` without the `bucket add` line fails with `Couldn't find manifest for
'pleno'`, because Pleno is in its own bucket, not in Scoop's main bucket. `scoop update pleno`
picks up new versions.

`scripts/scoop-manifest.sh <version> SHA256SUMS.txt` writes the manifest from `scoop/pleno.json`.
The release workflow runs it and pushes the result. Its `autoupdate` section also lets Scoop's own
tools update it.

## WinGet

```powershell
winget install Pleno.Pleno
```

**The first version** has to be submitted by hand. Validate the three files, then submit them as
`manifests/p/Pleno/Pleno/<version>/` in a pull request to
[microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs):

```powershell
winget validate --manifest packaging/winget
wingetcreate submit packaging/winget --token <token>
```

A volunteer moderator reviews new packages, which can take days or weeks. Pleno's first pull
request is [#460928](https://github.com/microsoft/winget-pkgs/pull/460928) (0.3.0). Until it is
merged, the release workflow's WinGet job skips with a notice.

**Later versions:** the workflow runs `wingetcreate update Pleno.Pleno --version <v> --urls <zip>
--submit`. That command copies the metadata from the version winget already has and opens the
pull request. Updates to an existing package are usually merged automatically once validation
passes.

## Chocolatey

```powershell
choco install pleno
```

### Once

1. Create an account at [community.chocolatey.org](https://community.chocolatey.org/account/Register)
   and confirm the email address.
2. Copy your API key from **Account → API Keys**
   ([community.chocolatey.org/account](https://community.chocolatey.org/account)).
3. Save it for `choco`, so later pushes need no `--api-key`:

   ```powershell
   choco apikey add --source https://push.chocolatey.org/ --key <key>
   ```

The package ID `pleno` was free on 2026-10-27. The first push claims it.

### Each release

`scripts/update-packaging.sh` updates `<version>` and `<releaseNotes>` in `chocolatey/pleno.nuspec`,
and `url64bit` and `checksum64` in `chocolatey/tools/chocolateyinstall.ps1` (see the top of this
page). Then, in an administrator PowerShell:

```powershell
choco pack packaging/chocolatey/pleno.nuspec --outputdirectory packaging/chocolatey
choco install pleno --source packaging/chocolatey --version 0.4.0 -y   # test locally
pleno --version
choco uninstall pleno -y
choco push packaging/chocolatey/pleno.0.4.0.nupkg --source https://push.chocolatey.org/
```

Every version goes through moderation: an automatic validator, an install test in a clean VM, and
for the first versions a human moderator. The package page shows each step's result. Messages from
moderators arrive by email, and a package that does not get an answer within a few weeks is
rejected. The `.nupkg` is generated, so don't commit it.

## AUR

```sh
yay -S pleno-bin      # or paru, or any AUR helper
```

### Once

1. Create an account at [aur.archlinux.org](https://aur.archlinux.org/register).
2. Make an SSH key for it and add the public key under **My Account → SSH Public Key**:

   ```sh
   ssh-keygen -t ed25519 -f ~/.ssh/aur -C "aur"
   ```

   Then tell SSH to use it, in `~/.ssh/config`:

   ```
   Host aur.archlinux.org
     IdentityFile ~/.ssh/aur
     User aur
   ```

3. Clone the package repository. A name nobody has used is created by the first push:

   ```sh
   git clone ssh://aur@aur.archlinux.org/pleno-bin.git
   ```

   The name `pleno-bin` (the `-bin` suffix means it installs prebuilt binaries) was free on
   2026-10-27.

### Each release

`scripts/update-packaging.sh` sets `pkgver`, resets `pkgrel=1` and writes the checksums in both
`aur/pleno-bin/PKGBUILD` and `.SRCINFO` (see the top of this page). If you change anything else in
`PKGBUILD`, regenerate `.SRCINFO` with `makepkg --printsrcinfo > .SRCINFO`. Test and push. This
needs Arch Linux, because `makepkg` refuses to run as root. On Windows, use Docker:

```sh
docker run --rm -it -v "$PWD/packaging/aur/pleno-bin:/pkg" archlinux bash
# inside the container:
pacman -Syu --noconfirm base-devel namcap
useradd -m builder && cp -r /pkg /home/builder/pkg && chown -R builder /home/builder/pkg
su builder -c 'cd ~/pkg && makepkg --printsrcinfo > .SRCINFO && makepkg -f && namcap PKGBUILD *.pkg.tar.zst'
cp /home/builder/pkg/.SRCINFO /pkg/   # keep the regenerated .SRCINFO
```

Then copy `PKGBUILD` and `.SRCINFO` into the `pleno-bin` clone and push:

```sh
cp packaging/aur/pleno-bin/PKGBUILD packaging/aur/pleno-bin/.SRCINFO pleno-bin/
cd pleno-bin
git add PKGBUILD .SRCINFO && git commit -m "pleno-bin 0.4.0" && git push
```

The AUR accepts only `PKGBUILD`, `.SRCINFO` and small helper files, on the `master` branch. A
push whose `.SRCINFO` does not match the `PKGBUILD` is rejected, so regenerate it every time.
