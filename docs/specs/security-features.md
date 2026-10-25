# Security Features

Pleno provides a multi-layered security model including memory safety, strict encapsulation, and resource-bounded sandboxing.

### 20.1 Safe Defaults

- **No implicit type coercion**: Forces explicit conversion to prevent subtle bugs.
- **Strict null checking**: Reduces null-pointer style exceptions.
- **Bounds checking**: All array and string accesses are checked.
- **Integer overflow detection**: Prevents wrapping by throwing errors.

### 20.2 Sandboxing

Pleno allows running untrusted code with restricted capabilities. Restrictions can be applied via CLI flags for the entire process, or programmatically using the `std:security` module.

#### CLI Flags

- `--max-instructions <N>`: Stop execution after N virtual instructions.
- `--max-memory <BYTES>`: Stop execution if memory exceeds limit.
- `--allow-io <bool>`: Enable/disable file system access.
- `--allow-read <path>`: Whitelist specific path for reading.
- `--allow-write <path>`: Whitelist specific path for writing.
- `--allow-network <bool>`: Enable/disable network access.

#### Programmatic Sandbox

The `std:security` module allows creating isolated execution environments.

```pleno
import { Sandbox } from "std:security";
import { vmStats } from "std:reflect";

// Define a sandbox with restricted permissions
let sandbox = Sandbox({
    allowIO: true,
    allowedReadPaths: ["/tmp/data"],
    allowedWritePaths: ["/tmp/output"],
    allowNetwork: false,
    maxMemory: 10 * 1024 * 1024,  // 10 MB
    maxInstructions: 1000000      // 1M instructions
});

try {
    let result = sandbox.run(fn() {
        // Untrusted code here
        let data = foreignSource.fetch();
        return process(data);
    });
} catch (e) {
    print("Sandbox violation: " + e);   // e.g. SandboxError: memory limit exceeded
    print("Peak Memory: " + vmStats().peakMemory.toString());
}
```

### 20.3 Encapsulation

Pleno enforces `private` and `protected` access modifiers at runtime. Reading a private field from outside its class throws an `Error` such as `cannot read private field 'name' of Person outside its class`; calling a private method throws `method access denied`. See [Access Modifiers](/specs/OOP#access-modifiers).
