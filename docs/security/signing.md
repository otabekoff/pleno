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

**Pleno Public GPG Key (Fingerprint: `E99E76E3B2DE...A00F9D08`):**

```text
-----BEGIN PGP PUBLIC KEY BLOCK-----

mQINBGmBpD4BEADG0FtI3jhELE8P+G7PJmT+pDmzQj6fzUfKmhLQjCQeoC/Ce3aB
F6DimhzOG/h+evGffYeoZP9Ur6Ugt8gXzTMKgDsQX9HMQiAvO6DXWtRNdxIV3DAD
ciHEm9zs9q+CyQqGSyoXmdVG5qWfaQ4p2DOmNcf5XAoCUFn1eM5kg9L5U9fY6xgt
e9w0iiCAR/5oaK5dspGovs/AtpQ57zgUtKKjGcIEJiOdlIsHW5Xy6T4GmZtG+Y2N
DEQMyh0rAL8c70ZQEQixEVHLv27t6EZVk7ew9o694sLUo7/wpA/9M9fgdRKVyDaN
Ww23aYnpR6Th3t7IjReBq6UGSfG2cINd5OdTIqrwM+Wzyg6Y8NT/10JY7aRi7C0n
yyJXJuj5KxWtS4wArofhZWlw9kYoiCFjX7qOSfm+oVbxP5XoWAcdwuvA6knYoxyn
hMK2jXhTGrRmn0znVWJvWCMs/jhiT6PK8RZZdwqeZSAmP0ebuHNy/PkSqowRiPa5
RPWx3lpU0N7BR6Q05vvOBeHdmfpiy/7PEuQJ//d8T17evN9+3uf8dBNyDPdDxSVf
tSIYuhk4mrhr+MJu1NmFQFnPwCsR6PtqfHMbXlh6R7KqqpBR3foutxuwXslAWQJl
6qEGgrs6qTYCqc/K+cunB2SvhgWM5lyT0IYpBVrbMsMuOQIk0DYJ/Cy6twARAQAB
tChQbGVubyBSZWxlYXNlcyA8cmVsZWFzZXNAcGxlbm8tbGFuZy5vcmc+iQJPBBMB
CgA5FiEE6Z5247LedeYHLk2+UmWVpaAPnQgFAmmBpD4DGy8EBQsJCAcCBhUKCQgL
AgQWAgMBAh4BAheAAAoJEFJllaWgD50IJlQP/2DfOPbwbef83IyvckcUVgpLXrvI
cGUwOkX6SQ2Xg1fgrDKSTSy3JXYERauou2htji8/XQMh3ZKNBJSiG2Fa5+vFjlZn
JVaWErR/K21GBO/sGdErxLl5ST0FAQqO1OC7yQGax6lsicir+lAjCvuxkSvXsPpk
4Mh+xwGYinX2xbgeO+yvf9f3rFrfn0Fx+4NduI1ztaqg1mYTEkz1abm4mz3jEc5h
VQ28e/AfX1b6F0pkrSzDdsWRL1K8CkyMh+qusxzFGK9u7t2KrSzDdsWRL1K8CkyM
h+qusxzFGK9u7t2KrNnK0YlqTdv2w06FtdUt9KDC2bKBXwmZv4bnVnTsqEnH0NaD
T8Dn/uz9Xa2P89WqrRn6joXDg6wPxw7e7VLFGtq7uYGHEVvpNYcQn8ItccYWxzCu
uOeiRdzX2PJXFwvfVB3mwAMrpIqWqn0e70EBvfv8oHAlLjB4fO1FyAjVAGQvwqtC
W4hUJmC6cFpKtGdkGWVtPjG6cXU/hoPkfQvovUW2+JK5vm80QYWPFQOY+ysX9WQB
2BmgXSP8vmNpu9IO6ypm4RM912WxW4QygeOg7//2o7ZF8mFfRVv3PBVbU0OB+ShE
rBUK/rSOK5N3t59dbF1oUg3YCCnG8d3JOShBz4pTBAbnFN+H4vy8FTQFVw+l/o2N
8mKh6jyh09nZLYgSuBLIF6MJljFNPbv1P1QDjjlS7FFHNBK5
=+daw
-----END PGP PUBLIC KEY BLOCK-----
```

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
    # Import the Pleno public key (from block above)
    # Or from keyserver:
    gpg --keyserver keys.openpgp.org --recv-keys E99E76E3B2DE75E6072E4DBE526595A5A00F9D08

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
