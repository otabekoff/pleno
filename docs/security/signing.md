# Security, Signing & Trust Guide

As an Open Source and non-profit project, Pleno prioritizes security without requiring expensive commercial certificates. This guide explains how we build trust and ensure binary integrity for free.

## 🛡️ Trust Architecture

Since we don't use high-cost "Enterprise" certificates, we use a **Community Trust Model**:

1.  **Transparency**: Every binary is built from public source code on GitHub Actions.
2.  **Verification**: Every release is GPG signed and SHA256 hashed.
3.  **Managed Distribution**: We prioritize Package Managers (Homebrew, AUR, Scoop) which "vouch" for the software.

---

## 🪟 Windows: Free Code Signing (SignPath)

Windows "SmartScreen" warnings can be avoided for free using **SignPath Foundation**.

- **Service**: [SignPath.io](https://signpath.io/)
- **How it works**: They provide a free code signing service for Open Source projects. You link your GitHub repository, and their CI signs your binaries with a certificate trusted by Windows.
- **Cost**: $0 for Open Source.

### Alternative: Reputation Building

If not signing, Windows will warn users initially. As more users click "Run anyway", the reputation of the binary grows, and the warning eventually disappears.

---

## 🍏 macOS: Homebrew Over Manual Install

Apple's $99/year fee is the only way to get a "Notarized" app for direct download. **However, for developers, Homebrew is the better, free alternative.**

- **The Solution**: Use our [Homebrew Tap](https://github.com/otabekoff/homebrew-tap).
- **Why it works**: When users run `brew install pleno`, Homebrew downloads or builds the binary. Since it's handled by a trusted local tool, macOS Gatekeeper does not block it like a random download from the web.
- **Cost**: $0.

---

## 🐧 Linux: GPG & Checksums (The Standard)

Linux users trust GPG signatures and repository managers.

### 1. GPG Signing (Free)

We sign our `SHA256SUMS.txt` with a GPG key. This allows you to verify that the binary was indeed packaged by the Pleno team.

### 2. Official Repositories

Installing via **AUR** (Arch) or **Snap** provides an extra layer of trust as these packages are defined by public build scripts.

---

## 🛠️ Verification for Users

Users can verify any Pleno download for free:

1.  **Verify Checksum**:
    ```bash
    sha256sum -c SHA256SUMS.txt
    ```
2.  **Verify Signature**:
    ```bash
    # Download our public key once
    gpg --recv-keys <OUR_KEY_ID>
    # Verify the sums file
    gpg --verify SHA256SUMS.txt.asc
    ```

---

## 💡 Policy for Contributors

- **Never** add your own private signing keys to the repo.
- **Always** ensure the CI generates checksums for new artifacts.
- **Prefer** updating package manager manifests over direct binary links in documentation.

---

> [!TIP]
> **Aesthestics of Trust**: Even without a "Verified Publisher" badge, having a professional website, clear SHA256 hashes, and GPG signatures sends a strong signal to the developer community that Pleno is high-quality and secure.
