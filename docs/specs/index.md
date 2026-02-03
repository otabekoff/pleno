# Pleno Programming Language

## Complete Language Specification v2.0.0

**File Extension:** `.pln`  
**Paradigms:** Object-Oriented, Functional, Procedural, Mixed  
**Typing:** Dynamic with optional static typing and gradual typing support  
**Design Philosophy:** Clarity, Safety, Expressiveness, Modern Best Practices

## Overview

### String Encoding Decision

Pleno uses UTF-8 for all string literals and runtime strings.

Key points:

- UTF-8 is the 2026+ industry standard across tools, platforms, and APIs.
- ASCII remains 1 byte per char, keeping source code compact.
- Full Unicode support is preserved for international text and emoji.

String indexing defaults to Unicode scalar values for safety. Byte-level access
is provided via explicit APIs.
