# GitHub Release Configuration Guide

This guide provides step-by-step, click-by-click instructions for configuring your GitHub repository to support the Pleno release workflow.

## 1. Configure Workflow Permissions

This allows the default `GITHUB_TOKEN` to create releases and upload binary assets.

1.  **Open your repository** on GitHub.
2.  Click the **Settings** tab (the gear icon @ top right).
3.  On the left sidebar, find the **Actions** section and click **General**.
4.  Scroll down to the **Workflow permissions** section.
5.  Select the **Read and write permissions** radio button.
6.  Check the box that says **Allow GitHub Actions to create and approve pull requests**.
7.  Click the **Save** button.

---

## 2. Configure Repository Secrets

If you want to add free, keyless signing to your workflow using Sigstore (Cosign), follow this complete guide.

### Prerequisites

You do **NOT** need to generate keys. Sigstore uses your GitHub identity (OIDC) to sign artifacts.

### 1. Update Workflow Permissions

In `.github/workflows/release.yml`, add `id-token: write` to the `publish-github` job. This allows GitHub to assert its identity.

```yaml
publish-github:
  # ...
  permissions:
    contents: write
    id-token: write # REQUIRED for Sigstore
```

### 2. Add Signing Step

Modify the `publish-github` job to install Cosign and sign the artifacts before uploading.

```yaml
steps:
  - uses: actions/checkout@v4

  # ... (download artifacts step) ...

  - name: Install Cosign
    uses: sigstore/cosign-installer@v3.3.0

  - name: Sign Release Assets
    run: |
      cd all-artifacts
      # Sign all binaries and archives
      cosign sign-blob --yes --bundle signature.bundle dist/pleno*
      cd ..

  # ... (upload step) ...
```

### 3. Verification for Users

Users can verify that the binary was built by your GitHub Actions workflow:

1.  Download the binary (`pleno`) and the signature bundle (`signature.bundle`).
2.  Run verify:

```bash
cosign verify-blob \
  --certificate-identity "https://github.com/otabekoff/pleno/.github/workflows/release.yml@refs/tags/v0.1.0" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  --bundle signature.bundle \
  pleno
```

Result:

> Verified OK

### Why do this?

It proves to users that the binary they downloaded was actually built by the code in your repository at that specific tag, and hasn't been swapped out by a malicious middleman.
These secrets are required for optional distribution channels like Docker Hub and Homebrew.

1.  Stay in the **Settings** tab.
2.  On the left sidebar, click **Secrets and variables** > **Actions**.
3.  Click the **New repository secret** button (green button @ top right).

### Add Docker Hub Credentials

- **DOCKERHUB_USERNAME**:
  1.  Name: `DOCKERHUB_USERNAME`
  2.  Value: Your Docker Hub username.
  3.  Click **Add secret**.
- **DOCKERHUB_TOKEN**:
  1.  Go to [Docker Hub Settings](https://hub.docker.com/settings/security).
  2.  Click **New Access Token**.
  3.  Give it a name (e.g., "pleno-ci") and click **Generate**.
  4.  Copy the token.
  5.  Back on GitHub, create a new secret named `DOCKERHUB_TOKEN` with this value.

### Add Homebrew Tap Token

This allows the workflow to update your Homebrew formula in `otabekoff/homebrew-tap`.

1.  Go to your [GitHub Personal Access Tokens (Classic)](https://github.com/settings/tokens).
2.  Click **Generate new token** > **Generate new token (classic)**.
3.  Note: "Homebrew Formula Bot"
4.  Expiration: 90 days (or as desired).
5.  **Scopes**: Check the **repo** box (this allows pushing to your tap repo).
6.  Scroll to the bottom and click **Generate token**.
7.  Copy the token immediately.
8.  Back on GitHub Secrets, create a new secret named `HOMEBREW_TAP_TOKEN` with this value.

---

## 3. Understanding Environment Variables

| Variable             | Context   | Purpose                                                           |
| :------------------- | :-------- | :---------------------------------------------------------------- |
| `GITHUB_TOKEN`       | Automatic | Built-in token. No setup needed (just the permissions in step 1). |
| `DOCKERHUB_USERNAME` | Secret    | Your Docker Hub ID.                                               |
| `DOCKERHUB_TOKEN`    | Secret    | Access token (the one you just generated).                        |
| `HOMEBREW_TAP_TOKEN` | Secret    | GitHub PAT with `repo` scope to update your Formula tap.          |

> [!IMPORTANT]
> **What do you do in the workflow file?**
> You don't need to change anything in `.github/workflows/release.yml`.
> The workflow is already programmed to automatically look for these exact secret names: `DOCKERHUB_USERNAME`, `DOCKERHUB_TOKEN`, and `HOMEBREW_TAP_TOKEN`.
> As long as you followed the naming in **Step 2** of this guide, it will work perfectly!

---

## 4. What Is (and Isn't) Signed

Releases are signed for free with **Sigstore keyless signing** (already enabled in `release.yml`):

- Every `.tar.gz`, `.zip`, `.deb` and `.vsix` gets a `<file>.bundle` signature, and `SHA256SUMS.txt` is signed too.
- `scripts/install.sh` and `scripts/install.ps1` verify the SHA256 checksum before installing.
- No keys or secrets are needed (`GPG_PRIVATE_KEY` / `GPG_PASSPHRASE` are not used).

Not covered, because there is no free option or it needs setup outside this repo:

- **Windows Authenticode**: unsigned `.exe` files trigger SmartScreen "Unknown publisher" warnings. [SignPath Foundation](https://signpath.org) signs open-source projects for free, but requires a public repository and an application.
- **macOS notarization**: Gatekeeper blocks unsigned binaries downloaded in a browser (`curl` downloads are not quarantined). This needs a paid Apple Developer account.
- **GitHub artifact attestations** (`actions/attest-build-provenance`): free for public repositories only.

---

## 🚀 How to Trigger a Release

1.  Make sure your code is pushed to `main`.
2.  Create a new tag: `git tag -a v0.1.1 -m "Release v0.1.1"`
3.  Push the tag: `git push origin v0.1.1`
4.  GitHub Actions will start the "Release" workflow automatically!
