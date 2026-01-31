# Security Features Technical Debt

## Summary
Audited specs.md section 20 (Security Features) against actual implementation. Pruned items completed in the Networking & Metadata pass.

## Status by Feature

### 20.2 Sandboxing
| Feature | Status | Notes |
|---------|--------|-------|
| Low-level Network (`std:net`) | ✅ IMPLEMENTED | TCP support in `std:net` with `allowNetwork` checks. |

## Recommendations

1. **Consider for next cycle**:
   - Implement `std:net` (TCP/UDP) with support for `allowNetwork` sandbox restriction. ✅ IMPLEMENTED
   - Capability-based security model for granular file system permissions (e.g., read-only on specific directories). ✅ IMPLEMENTED

2. **Defer to future**:
   - Full OS-level process sandboxing (using `seccomp` or similar).
   - Memory usage quotas per-module (advanced isolation).
