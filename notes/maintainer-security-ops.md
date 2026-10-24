# Pleno Maintainer: Security Operations Guide

This guide provides step-by-step instructions for the daily and one-time security activities required to keep Pleno trusted and secure.

---

## 1. 🛡️ GPG Key Management

I have already generated a release key for Pleno. You should back it up.

### Activity: Backup Your Key

1. **Export Private Key** (Keep this offline!):
   ```bash
   gpg --armor --export-secret-keys releases@pleno-lang.org > pleno_private_backup.asc
   ```
2. **Move to Secure Storage**: Save this file on a physical encrypted USB drive or a secure password manager.
3. **Delete Local Copy**: `rm pleno_private_backup.asc`.

---

## 2. 🪟 Windows Signing (SignPath.io)

Since we want to avoid viruses and SmartScreen warnings for free.

### Activity: Setup SignPath (One-time)

1. **Apply for Open Source Foundation**: Go to [SignPath.io](https://signpath.org/foundation) and apply for the free "Open Source Foundation" plan.
2. **Link GitHub**: Once approved, link the `otabekoff/pleno` repository.
3. **Configure the Project**:
   - Set the build system to "GitHub Actions".
   - Define a signing policy (e.g., "Only sign releases from the main branch").
4. **Update GitHub Secrets**: SignPath will provide a `SIGNPATH_API_TOKEN`. Add this to your GitHub repository secrets.

---

## 3. 🍏 macOS "Pseudo-Notarization"

We use Homebrew to avoid the $99 Apple tax.

### Activity: Regular Maintenance

1. **Update Brew Formula**: Every time you release a new version, update your [homebrew-tap](https://github.com/otabekoff/homebrew-tap).
2. **Verify on macOS**: After releasing, run:
   ```bash
   brew update && brew install pleno
   ```
   If it installs without a "Maliicous Software" warning, our distribution strategy is working.

---

## 4. 🚀 Verification of Trust (Anti-Virus)

To ensure Pleno isn't flagged as a virus.

### Activity: Pre-Release Check

1. **VirusTotal**: Before announcing a release, upload the Windows `.exe` to [VirusTotal](https://www.virustotal.com/).
2. **False Positive Reporting**: If a vendor (like Avast or Windows Defender) flags it, use their "Report False Positive" forms immediately.
3. **Reputation Building**: The more users run the signed SignPath binary, the higher our reputation becomes.

---

## 🤝 Community Transparency

### Sigstore (The Modern Standard)

Sigstore allows you to sign releases without managing long-lived keys.

1. **Action**: In your GitHub Release workflow, add the `sigstore/gh-action-sigstore-python` (or similar generic action).
2. **Benefit**: It provides a publicly auditable log that you signed the release using your GitHub identity.

### Security Advisories

- **Action**: Use [GitHub Security Advisories](https://github.com/otabekoff/pleno/security/advisories) to report bugs gracefully.
- **Dependency Review**: Run `cargo audit` monthly to check for vulnerabilities in our dependencies.
