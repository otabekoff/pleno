# Security Features Technical Debt

## Summary
Audited specs.md section 20 (Security Features) against actual implementation. Pruwhat items.

## Status by Feature

### 20.2 Sandboxing
| Feature | Status | Notes |
|---------|--------|-------|
| Network access control | ✅ IMPLEMENTED | Blocked in `std:http` via `allowNetwork: false` |

## Recommendations

1. **Consider for next cycle**:
   - Implement `std:http` and `std:net` with support for `allowNetwork` sandbox restriction.
   - Refine `SandboxError` objects to include more metadata (e.g., peak memory used). ✅ IMPLEMENTED

2. **Defer to future**:
   - Full OS-level process sandboxing (using `seccomp` or similar).
   - Capability-based security model for granular file system permissions (e.g., read-only on specific directories).
