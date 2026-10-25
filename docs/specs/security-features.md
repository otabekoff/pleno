# Security Features

Pleno checks common mistakes at runtime, enforces access modifiers, and can limit what a program or a function may do.

### 20.1 Safe Defaults

- **Few implicit conversions.** `+` with a String on either side converts the other operand to a string (`"a" + 1` is `"a1"`). Other mixes are errors: `true + 1` and `"3" * 2` throw `invalid operands`, and `1 == "1"` is `false`. An `Int` becomes a `Float` only where a `Float` is annotated.
- **Null access throws.** Reading a property of `null` throws (`cannot read property 'length' of Null`); use `?.` and `??` for values that may be missing.
- **Bounds checking.** Array and string indexes are checked: `[1][5]` throws `array index out of bounds`.
- **Integer overflow throws.** `Int` arithmetic that leaves the 64-bit range throws `integer overflow in addition` (or `multiplication`, ...) instead of wrapping.

```pleno
print("total: " + 3);          // total: 3

try {
    let big = 9223372036854775807;
    print(big + 1);
} catch (e) {
    print(e.message);          // integer overflow in addition
}

try {
    print([1, 2][5]);
} catch (e) {
    print(e.message);          // array index out of bounds
}
```

### 20.2 Sandboxing

Limits can be set for the whole program with flags of `pleno run`, or for one function with `std:security`.

::: warning Not a security boundary
The limits are meant to stop runaway or careless code, not a determined attacker. In particular the path rules below compare strings: an allowed path is a prefix of the path exactly as the program writes it, with no normalization, so `data/../secret.txt` passes a rule for `data/`. Run untrusted code in an operating system sandbox as well.
:::

#### CLI Flags

The flags go before or after the file name: `pleno run --max-instructions 1000000 script.pln`.

- `--max-instructions <N>`: stop after N VM instructions with `SandboxError: instruction limit exceeded`.
- `--max-memory <BYTES>`: stop when the heap grows past the limit.
- `--allow-io false`: deny console and file access. `print` and `File.read` throw `SecurityError: IO access denied`.
- `--allow-read <path>`, `--allow-write <path>`: allow reading or writing only paths that start with `<path>` (see the warning above). Can be repeated. `File.exists` is not restricted.
- `--allow-network false`: deny `std:http` and `std:net`.

Denied operations throw a `SecurityError` that a `catch` can handle. When a limit is exceeded the program stops.

#### Programmatic Sandbox

**Experimental.** `std:security` is not covered by the [compatibility promise](./compatibility.md).

`Sandbox(options)` creates a sandbox, and `sandbox.run(fn)` calls `fn` in a separate VM with those limits and returns its result. The function can read variables it captures.

```pleno
import { Sandbox } from "std:security";

let sandbox = Sandbox({
    allowIO: false,           // no printing or file access
    allowNetwork: false,
    maxMemory: 10 * 1024 * 1024,
    maxInstructions: 100000
});

print(sandbox.run(fn() { return 6 * 7; }));   // 42

try {
    sandbox.run(fn() {
        while (true) {}
    });
} catch (e) {
    print(e.message);   // uncaught exception: SandboxError: instruction limit exceeded
}
```

| Option | Type | Default | Meaning |
| --- | --- | --- | --- |
| `allowIO` | `Bool` | `true` | Allow console and file I/O |
| `allowNetwork` | `Bool` | `true` | Allow network access |
| `maxMemory` | `Int` | none | Heap limit in bytes |
| `maxInstructions` | `Int` | none | Instruction limit |
| `allowedReadPaths` | `String[]` | all | Path prefixes the function may read |
| `allowedWritePaths` | `String[]` | all | Path prefixes the function may write |

An error inside `run` reaches the caller as an `Error` whose `name` is `Error` and whose `message` starts with `uncaught exception:` followed by the original error, as above. This is expected to change so that the original `name` (`SandboxError`, `SecurityError`) is kept.

### 20.3 Encapsulation

Pleno enforces `private`, `protected` and `internal` access modifiers at runtime. Reading a private field from outside its class throws an `Error` such as `cannot read private field 'name' of Person outside its class`; calling a private method throws `method access denied`. See [Access Modifiers](/specs/OOP#access-modifiers).
