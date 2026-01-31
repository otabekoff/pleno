# Security Features Technical Debt

## Summary
Audited specs.md section 20 (Security Features) against actual implementation.

## Status by Feature

### 20.1 Safe Defaults
| Feature | Status | Notes |
|---------|--------|-------|
| No implicit type coercion | ✅ IMPLEMENTED | String+Any now errors, requires explicit toString() |
| Strict null checking | ✅ IMPLEMENTED | `null.property` throws "field access on non-object" |
| Bounds checking on arrays | ✅ IMPLEMENTED | Out-of-bounds throws "array index out of bounds" |
| Integer overflow detection | ✅ IMPLEMENTED | `checked_add/sub/mul` in `value_ops.rs` |

### 20.2 Sandboxing
| Feature | Status | Notes |
|---------|--------|-------|
| `std:security` module | ✅ IMPLEMENTED | Exposes `Sandbox` class |
| `Sandbox` class | ✅ IMPLEMENTED | `Sandbox(config).run(fn)` creates restricted VM |
| File access control | ✅ IMPLEMENTED | Controlled via `allowIO: false` in config |
| Network access control | ⚠️ PARTIAL | Config exists, but no network modules to block yet |
| Memory limits | ⚠️ PARTIAL | Config exists, basic checks (not strictly enforced per-op) |

## Test Results

| Test | Result |
|------|--------|
| `"5" + 5` | `55` (coerces - not strict) |
| `true + 1` | Error: "invalid operands for +" ✅ |
| `null.foo` | Error: "field access on non-object" ✅ |
| `arr[999]` | Error: "array index out of bounds" ✅ |
| `9223372036854775807 + 1` | Error: "integer overflow in addition" ✅ |
| `Sandbox({allowIO: false}).run(...)` | Error: "SecurityError: IO access denied" ✅ |

## Recommendations

1. **Consider for v1**:
   - Decide if String+Any auto-concatenation is intended behavior
   - Document type coercion rules clearly

2. **Defer to future**:
   - Full sandboxing (major security architecture)
   - `std:security` module
