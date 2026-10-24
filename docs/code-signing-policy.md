# Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org).

## What is signed

Windows release binaries are signed with an Authenticode certificate:

- `pleno.exe`, `plm.exe`, `pleno-lsp.exe`, `pleno-repl.exe` and `pleno-dap.exe`
- shipped in `pleno-<version>-x86_64-pc-windows-msvc.zip` on [GitHub Releases](https://github.com/otabekoff/pleno/releases)

Only binaries built from this repository by the [release workflow](https://github.com/otabekoff/pleno/blob/main/.github/workflows/release.yml) are signed. That workflow runs on GitHub Actions for tags pushed from the `main` branch, and every signing request is approved manually by an approver listed below. Third-party libraries are compiled into the binaries from source; no upstream binaries are signed under this policy.

All release archives, on every platform, are also signed with [Sigstore](https://www.sigstore.dev/) keyless signing. Each archive has a `.bundle` file next to it, and `SHA256SUMS.txt` lists the checksums of every file. See [Verifying downloads](#verifying-downloads).

**Signing status:** Windows code signing is being set up with SignPath Foundation. Until a release notes otherwise, Windows binaries are not Authenticode-signed; the Sigstore signatures and checksums apply to all releases.

## Team roles

| Role | Members |
| --- | --- |
| Committers and reviewers | [Otabek (@otabekoff)](https://github.com/otabekoff) |
| Approvers | [Otabek (@otabekoff)](https://github.com/otabekoff) |

Changes from contributors without commit access are reviewed by a committer before they are merged. All team members use multi-factor authentication for GitHub and SignPath.

## Privacy policy

This program will not transfer any information to other networked systems unless specifically requested by the user or the person installing or operating it.

Pleno connects to the network only when you ask it to:

- `plm install`, `plm update` and `plm publish` contact the package registry and the Git hosts of the packages you depend on.
- Pleno programs you write or run can make network requests through `std:http` and `std:net`. These can be turned off with `--allow-network false`.

The [VS Code extension](https://github.com/otabekoff/pleno/tree/main/ide/vscode) is distributed separately and is not covered by this policy.

## Verifying downloads

Check an archive's checksum:

```bash
sha256sum -c pleno-<version>-x86_64-unknown-linux-gnu.tar.gz.sha256
```

Check its Sigstore signature with [cosign](https://docs.sigstore.dev/cosign/system_config/installation/):

```bash
cosign verify-blob \
  --bundle pleno-<version>-x86_64-unknown-linux-gnu.tar.gz.bundle \
  --certificate-identity-regexp "https://github.com/otabekoff/pleno/.github/workflows/release.yml@refs/tags/.*" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  pleno-<version>-x86_64-unknown-linux-gnu.tar.gz
```

On Windows, the Authenticode signature of a signed `.exe` is shown under **Properties → Digital Signatures**, or with `Get-AuthenticodeSignature .\pleno.exe` in PowerShell.

## Installing and uninstalling

The install scripts copy the binaries to `~/.pleno/bin` and add that directory to your `PATH`. They print each change as they make it.

To uninstall, run the matching script, which removes `~/.pleno` and the `PATH` entry:

```bash
curl -fsSL https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/uninstall.sh | sh
```

```powershell
irm https://raw.githubusercontent.com/otabekoff/pleno/main/scripts/uninstall.ps1 | iex
```

To uninstall by hand, delete `~/.pleno` and remove the `PATH` line the installer added to your shell profile (Linux and macOS) or the `~\.pleno\bin` entry from your user `PATH` (Windows).
