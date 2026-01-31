# Pleno Programming Language
## Complete Language Specification v1.0

**File Extension:** `.pln`  
**Paradigms:** Object-Oriented, Functional, Procedural, Mixed  
**Typing:** Dynamic with optional static typing and gradual typing support  
**Design Philosophy:** Clarity, Safety, Expressiveness, Modern Best Practices

## 20. Security Features

### 20.1 Safe Defaults
- No implicit type coercion
- Strict null checking
- Bounds checking on arrays
- Integer overflow detection

### 20.2 Sandboxing

```pleno
import { Sandbox } from "std:security";

// Run untrusted code in sandbox
let sandbox = Sandbox({
    allowFileAccess: false,
    allowNetworkAccess: false,
    memoryLimit: 10 * 1024 * 1024  // 10 MB
});

let result = sandbox.run(fn() {
    // Untrusted code here
    return calculate(data);
});
```

---

## Conclusion

Pleno is designed to be:
- **Easy to learn**: Familiar syntax, clear semantics
- **Safe by default**: Strong typing, real encapsulation, memory safety
- **Powerful**: OOP, functional, and procedural paradigms
- **Modern**: Async/await, pattern matching, generics
- **Practical**: Great standard library, package manager, tooling

This specification provides a complete foundation for a modern programming language suitable for 2026 and beyond.