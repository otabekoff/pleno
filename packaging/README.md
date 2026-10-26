# Packaging

Package definitions for each package manager. Every one installs the prebuilt release archives
(`pleno`, `plm`, `pleno-lsp`, `pleno-dap` and `pleno-repl`) and pins their SHA-256 checksums,
taken from the release's `SHA256SUMS.txt`.

| Directory        | Platform      | Package manager | How it is published                                    |
| ---------------- | ------------- | --------------- | ------------------------------------------------------ |
| `homebrew/`      | macOS, Linux  | Homebrew        | The release workflow pushes it to `otabekoff/homebrew-tap` |
| `scoop/`         | Windows       | Scoop           | Copied to `bucket/pleno.json` in `otabekoff/scoop-bucket`  |
| `winget/`        | Windows       | WinGet          | A pull request to `microsoft/winget-pkgs`               |
| `chocolatey/`    | Windows       | Chocolatey      | `choco push` with the maintainer's API key              |
| `aur/pleno-bin/` | Arch Linux    | AUR             | Pushed to the `pleno-bin` AUR repository                |
| `snap/`, `flatpak/`, `asdf/` | Linux | —        | Not maintained yet                                     |

## For each release

The Homebrew formula is written and published by the release workflow
(`scripts/homebrew-formula.sh`). The others are updated by hand: change the version, the URLs and the
checksums (from `SHA256SUMS.txt` in the release), then publish.

### Homebrew

```sh
brew install otabekoff/tap/pleno
```

### Scoop

```powershell
scoop bucket add pleno https://github.com/otabekoff/scoop-bucket
scoop install pleno
```

To publish, copy `scoop/pleno.json` to `bucket/pleno.json` in the bucket repository. Its
`autoupdate` section also lets Scoop's tools update it from a new release.

### WinGet

```powershell
winget validate --manifest packaging/winget
```

Then submit the three files as `manifests/p/Pleno/Pleno/<version>/` in a pull request to
[microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs), or use
`wingetcreate submit packaging/winget`. Once it is accepted: `winget install Pleno.Pleno`.

### Chocolatey

```powershell
choco pack packaging/chocolatey/pleno.nuspec --outputdirectory packaging/chocolatey
choco install pleno --source packaging/chocolatey   # test locally
choco push packaging/chocolatey/pleno.0.3.0.nupkg --source https://push.chocolatey.org/ --api-key <key>
```

### AUR

```sh
git clone ssh://aur@aur.archlinux.org/pleno-bin.git
cp packaging/aur/pleno-bin/PKGBUILD packaging/aur/pleno-bin/.SRCINFO pleno-bin/
cd pleno-bin && makepkg -si   # test, then:
git add PKGBUILD .SRCINFO && git commit -m "pleno-bin 0.3.0" && git push
```

`.SRCINFO` must match the PKGBUILD; regenerate it with `makepkg --printsrcinfo > .SRCINFO`.
