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
| `DOCKERHUB_TOKEN`    | Secret    | Access token for pushing images.                                  |
| `HOMEBREW_TAP_TOKEN` | Secret    | GitHub PAT with `repo` scope to update your Formula tap.          |

---

## 4. Why "No Signing"?

Following your decision to **build but not sign**:

- The workflow no longer requires `GPG_PRIVATE_KEY` or `GPG_PASSPHRASE`.
- Windows `.exe` and macOS binaries will be built but will be "unsigned". Users might see "Unknown Developer" warnings when running them for the first time.
- The `.deb` package and `.vsix` extension will follow standard packaging rules.

---

## 🚀 How to Trigger a Release

1.  Make sure your code is pushed to `main`.
2.  Create a new tag: `git tag -a v0.1.1 -m "Release v0.1.1"`
3.  Push the tag: `git push origin v0.1.1`
4.  GitHub Actions will start the "Release" workflow automatically!
