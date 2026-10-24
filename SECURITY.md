# Security Policy

We take the security of the Pleno programming language seriously. This document describes how to report vulnerabilities and what we consider to be in scope.

## 🛡️ Supported Versions

We currently provide security updates for the following versions:

| Version | Supported       |
| ------- | --------------- |
| 0.1.x   | ✅ Yes (Active) |
| < 0.1.0 | ❌ No           |

## 📝 Reporting a Vulnerability

**DO NOT open a public GitHub issue for security vulnerabilities.**

If you discover a security vulnerability, report it privately through GitHub:

**[Report a vulnerability](https://github.com/otabekoff/pleno/security/advisories/new)**

Only the maintainers can see the report.

Please include the following in your report:

- **Type of issue** (e.g., buffer overflow, logic error, sandbox escape).
- **Location** of the affected code.
- **Detailed steps** to reproduce the issue.
- **Proof-of-Concept** (if possible).
- **Impact assessment** (how could this be exploited?).

### 📅 Response Timeline

1. **Acknowledgment**: You will receive an acknowledgment within **48 hours**.
2. **Investigation**: We will investigate the issue and provide updates every **7 days**.
3. **Resolution**: We aim to resolve critical vulnerabilities within **30 days**.

## 🎯 Scope

### In Scope

- **VM Escapes**: Code running in the VM that can execute arbitrary code on the host.
- **Parser/Compiler Crashes**: Maliciously crafted source code that can crash the compiler (DoS).
- **Standard Library**: Vulnerabilities in `std` (e.g., directory traversal in file IO).
- **Sandbox**: Bypassing configured security constraints in the runtime sandbox.

### Out of Scope

- Issues in the documentation website (`otabekoff.github.io/pleno`), which is static.
- Vulnerabilities in third-party dependencies (unless they impact Pleno-specific logic).
- Social engineering attacks.

## 📜 Public Disclosure

We ask that you do not disclose the vulnerability publicly until we have had a chance to fix it and release an update. Once fixed, we will provide an advisory and credit you for the discovery (unless you prefer to remain anonymous).

---

Thank you for helping keep Pleno secure!
