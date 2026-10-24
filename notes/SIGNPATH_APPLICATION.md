# SignPath Foundation application: Pleno

Everything you need to apply for free Windows code signing at [signpath.org/apply](https://signpath.org/apply), and to set it up once approved. Researched on 2026-10-24 from the [SignPath Foundation terms](https://signpath.org/terms) and the [SignPath GitHub integration docs](https://docs.signpath.io/trusted-build-systems/github).

The application form loads dynamically, so its exact field names could not be read in advance. The answers below cover every item the terms require and that applications are known to ask for; copy each answer into the matching field.

## 1. Before you apply

| Requirement (SignPath terms) | Status |
| --- | --- |
| OSI-approved license, no commercial dual-licensing | ✅ MIT (`LICENSE`) |
| No proprietary or closed-source components | ✅ All dependencies are open source, built from source |
| Actively maintained | ✅ Recent commits, issues and PRs |
| Already released in the form to be signed | ✅ `pleno-v0.1.0-x86_64-pc-windows-msvc.zip` on GitHub Releases |
| Functionality described on the download page | ⚠️ After this branch merges, run the command in step 1b |
| "Code signing policy" on the home page and release pages | ✅ Docs nav, README, and the release notes template (after merge) |
| Credit line, team roles and privacy policy on the policy page | ✅ `docs/code-signing-policy.md` |
| Uninstall instructions for the installers | ✅ `scripts/uninstall.sh`, `scripts/uninstall.ps1`, documented on the policy page |
| No unannounced system changes | ✅ Installers print every change (PATH update) |
| Product name and identical product version in every binary | ✅ Embedded by `build/windows_metadata.rs` (`ProductName=Pleno`) |
| Binaries built verifiably from source | ✅ GitHub Actions `release.yml` from tagged commits |
| Multi-factor authentication for every team member | ⚠️ **Your action:** confirm at <https://github.com/settings/security> |
| A contact email you read reliably | ⚠️ **Your action:** choose one (SignPath follows up by email) |

### 1a. Turn on GitHub two-factor authentication

Open <https://github.com/settings/security>. If "Two-factor authentication" is not enabled, enable it. SignPath requires MFA for everyone with access to the repository and to SignPath.

### 1b. Describe Pleno on the existing release page

After the docs with the policy page are deployed (merging to `main` deploys them), add the description and policy link to the v0.1.0 release:

```bash
gh release edit v0.1.0 --notes "Pleno is an open-source (MIT) programming language: a compiler and bytecode VM with a package manager (plm), language server, REPL and debug adapter. Each archive contains pleno, plm, pleno-lsp, pleno-repl and pleno-dap for one platform.

Downloads are verified by checksums and Sigstore signatures. See the [Code signing policy](https://pleno.mohirlab.uz/code-signing-policy).

**Full Changelog**: https://github.com/otabekoff/pleno/commits/v0.1.0"
```

Future releases get this text automatically from `release.yml`.

## 2. Answers for the application form

**Project name**
```
Pleno
```

**Repository URL** (the primary development repository)
```
https://github.com/otabekoff/pleno
```

**Project home page**
```
https://pleno.mohirlab.uz/
```

**Download / release page**
```
https://github.com/otabekoff/pleno/releases
```

**Code signing policy URL**
```
https://pleno.mohirlab.uz/code-signing-policy
```

**Privacy policy**
```
https://pleno.mohirlab.uz/code-signing-policy#privacy-policy
```

**License**
```
MIT License (OSI-approved). The full text is in the LICENSE file at the repository root. No component is commercially dual-licensed or proprietary.
```

**Project description**
```
Pleno is an open-source programming language with a compiler, a register-based bytecode virtual machine and a standard library, written in Rust. It ships as command-line tools: pleno (run, format, lint, test and document Pleno programs), plm (package manager), pleno-lsp (language server for editors), pleno-repl (interactive shell) and pleno-dap (debug adapter for editors such as VS Code). It is used by developers writing and running Pleno programs.

All downloads are free. Releases are published on GitHub Releases as archives for Linux, macOS and Windows.
```

**What will be signed** (artifact types)
```
Windows x64 executables: pleno.exe, plm.exe, pleno-lsp.exe, pleno-repl.exe and pleno-dap.exe. They are distributed inside the release archive pleno-<version>-x86_64-pc-windows-msvc.zip.
```

**Build system / how the binaries are built**
```
GitHub Actions. The release workflow (.github/workflows/release.yml) runs when a version tag is pushed from main. It builds every binary from source with cargo build --release on GitHub-hosted windows-latest runners; no binaries are added manually and no upstream binaries are signed. Signing requests will be submitted from that workflow with signpath/github-action-submit-signing-request and approved manually for each release.
```

**Team and roles**
```
Committers and reviewers: Otabek (@otabekoff, https://github.com/otabekoff)
Approvers: Otabek (@otabekoff)
Contributions from people without commit access are reviewed by a committer before merging. All team members use multi-factor authentication.
```

**Does the software transfer data to other systems?**
```
No, unless the user requests it. The plm package manager contacts the package registry and Git hosts only when the user runs plm install, update or publish, and Pleno programs can make network requests through std:http and std:net only when the program's author writes them (they can be disabled with --allow-network false). There is no telemetry.
```

**Does the software change system configuration?**
```
The optional install scripts copy the binaries to ~/.pleno/bin and add that directory to the user's PATH, printing each change. Matching uninstall scripts (scripts/uninstall.sh, scripts/uninstall.ps1) undo both. The binaries themselves change nothing on the system.
```

**Contact email**
```
<the email address you read reliably>
```

## 3. After approval: set up SignPath

SignPath will send onboarding instructions. Based on their documentation, the setup is:

1. In SignPath, add the predefined **GitHub.com** trusted build system to your organization and link it to the Pleno project.
2. Optionally install the [SignPath GitHub App](https://github.com/apps/signpath) on `otabekoff/pleno`. It is needed only for audit-log checks; the repository is public, so the default `GITHUB_TOKEN` can already read build details.
3. Create a signing policy (for example `release-signing`) with **Verify origin** enabled and **Allowed branch names** set to `main`, and with manual approval for each request.
4. Create an artifact configuration that signs the five `.exe` files inside a ZIP. Its root element must be `<zip-file>` because `actions/upload-artifact` uploads a ZIP.
5. Create an API token for a user with submitter permission and store it as the repository secret `SIGNPATH_API_TOKEN` (Settings → Secrets and variables → Actions).
6. Note your **organization ID**, **project slug** and **signing policy slug** for the workflow.

### Workflow change

Add these steps to the Windows build in `.github/workflows/release.yml`, after the `Build` step and before `Packaging`, and fill in your IDs. They upload the unsigned binaries, wait for the approved signature, and replace the binaries with the signed ones before the archive is made:

```yaml
      - name: Upload unsigned Windows binaries
        if: runner.os == 'Windows'
        id: upload-unsigned
        uses: actions/upload-artifact@v4
        with:
          name: unsigned-windows-binaries
          path: |
            target/${{ matrix.job.target }}/release/pleno.exe
            target/${{ matrix.job.target }}/release/plm.exe
            target/${{ matrix.job.target }}/release/pleno-lsp.exe
            target/${{ matrix.job.target }}/release/pleno-repl.exe
            target/${{ matrix.job.target }}/release/pleno-dap.exe

      - name: Sign Windows binaries with SignPath
        if: runner.os == 'Windows'
        uses: signpath/github-action-submit-signing-request@v3
        with:
          api-token: ${{ secrets.SIGNPATH_API_TOKEN }}
          organization-id: "<SignPath organization id>"
          project-slug: "pleno"
          signing-policy-slug: "release-signing"
          github-artifact-id: ${{ steps.upload-unsigned.outputs.artifact-id }}
          wait-for-completion: true
          output-artifact-directory: target/${{ matrix.job.target }}/release
```

Then update the **Signing status** paragraph in `docs/code-signing-policy.md` to say from which release Windows binaries are signed.

## 4. What SignPath does not cover

- **macOS:** Gatekeeper notarization needs a paid Apple Developer Program membership. Downloads made with `curl` (as `install.sh` does) are not quarantined, so they run without it.
- **Linux:** no OS-level signing is needed; checksums and Sigstore signatures already cover all platforms.
