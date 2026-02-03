# Code Signing and Security Guide

This guide explains how Pleno ensures binary integrity and trust across different platforms.

## 🛡️ Why Sign?

To prevent "Unknown Publisher" or "Malicious Software" warnings (Windows SmartScreen, macOS Gatekeeper) and to ensure users are running authentic Pleno binaries that haven't been tampered with.

---

## 🍏 macOS (Notarization)

Apple requires all software distributed outside the App Store to be signed with a **Developer ID Certificate** and notarized by Apple.

### What you need:

1.  **Apple Developer Program Subscription** ($99/year).
2.  **Certificate**: Create a "Developer ID Application" certificate in the [Apple Developer Portal](https://developer.apple.com/account/resources/certificates/list).
3.  **App-Specific Password**: For the GitHub Action to authenticate.

### GitHub Secrets to Add:

- `MACOS_CERTIFICATE`: Base64 encoded `.p12` certificate.
- `MACOS_CERTIFICATE_PASSWORD`: Password for the `.p12` file.
- `MACOS_DEVELOPER_ID`: Your Developer ID (e.g., `Developer ID Application: Your Name (ID)`).
- `MACOS_NOTARIZATION_APPLE_ID`: Your Apple ID email.
- `MACOS_NOTARIZATION_PASSWORD`: The app-specific password.

---

## 🪟 Windows (EV/Standard Certificate)

Windows binaries should be signed with an Authenticode certificate. **EV (Extended Validation)** certificates provide instant SmartScreen reputation.

### What you need:

1.  **Purchase a Certificate**: From a CA like DigiCert, GlobalSign, or Sectigo.
2.  **Export as PFX**: Usually provided as a `.pfx` or `.p12` file.

### GitHub Secrets to Add:

- `WINDOWS_CERT_BASE64`: Base64 encoded `.pfx` file.
- `WINDOWS_CERT_PASSWORD`: Password for the `.pfx` file.

---

## 🐧 Linux (GPG Signing)

On Linux, we provide GPG signatures for our releases and checksum files. This is **free** and uses standard open-source tools.

### What you need:

1.  **GPG Key Pair**:
    ```bash
    gpg --full-generate-key
    ```
2.  **Export Private Key**:
    ```bash
    gpg --export-secret-keys --armor <ID> | base64
    ```

### GitHub Secrets to Add:

- `GPG_PRIVATE_KEY`: The base64 encoded private key.
- `GPG_PASSPHRASE`: The passphrase for your key.

---

## 🛠️ How to Implement (Infrastructure)

The [release.yml](.github/workflows/release.yml) is already configured with placeholders for these processes. Once you add the secrets above, the CI will automatically:

1.  Sign Windows `.exe` files using `signtool`.
2.  Sign macOS binaries and notarize them.
3.  Sign the `SHA256SUMS.txt` file with GPG.

### Verifying a Release

Users can verify the signature of the checksum file:

```bash
gpg --verify SHA256SUMS.txt.asc
sha256sum -c SHA256SUMS.txt
```

---

> [!IMPORTANT]
> As an AI, I cannot purchase these certificates for you (they require legal identity verification and payment). However, once you obtain them and add them to your GitHub Secrets, the automation I've built will take over.
