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

Path rules compare normalized absolute paths, one folder at a time: the path a program uses is resolved against the working directory, `.` and `..` are removed and symbolic links are followed, so `data/../secret.txt` is outside a rule for `data`, and `data2/x.txt` is outside it too. Checking whether a path exists (`Path.exists`, `isFile`, `isDir`, `resolve`) counts as reading it. A sandbox option with a value that does not parse (`--allow-io maybe`, `--max-memory 10MB`) is an error, never "no limit".

::: warning Not an operating system sandbox
The limits stop runaway or careless code and keep a script to the files it was given. Environment variables are not restricted. To run code you do not trust, use an operating system sandbox or container as well.
:::

#### CLI Flags

The flags go before or after the file name: `pleno run --max-instructions 1000000 script.pln`.

- `--max-instructions <N>`: stop after N VM instructions with `SandboxError: instruction limit exceeded`.
- `--max-memory <BYTES>`: stop when the heap grows past the limit.
- `--allow-io false`: deny console and file access. `print` and `File.read` throw `SecurityError: IO access denied`.
- `--allow-read <path>`, `--allow-write <path>`: allow reading or writing only paths that start with `<path>` (see the warning above). Can be repeated. `File.exists` is not restricted.
- `--allow-network false`: deny `std:http` and `std:net`.

Denied operations throw a `SecurityError` that a `catch` can handle. When a limit is exceeded the program stops with `SandboxError: instruction limit exceeded` (or `memory limit exceeded`) and exit status 1: no `catch` or `finally` block runs, so a program cannot keep itself alive past its limit.

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
    print(e.name, e.message);   // SandboxError instruction limit exceeded
}
```

| Option | Type | Default | Meaning |
| --- | --- | --- | --- |
| `allowIO` | `Bool` | `true` | Allow console and file I/O |
| `allowNetwork` | `Bool` | `true` | Allow network access |
| `maxMemory` | `Int` | none | Heap limit in bytes |
| `maxInstructions` | `Int` | none | Instruction limit |
| `allowedReadPaths` | `String[]` | all | Folders (or files) the function may read |
| `allowedWritePaths` | `String[]` | all | Folders (or files) the function may write |

An error inside `run` reaches the caller with its original `name` (`SandboxError` for a limit, `SecurityError` for a denied operation) and message. A limit cannot be caught inside the sandboxed function itself: reaching it ends that function, and only the caller of `run` sees the error.

### 20.3 Encapsulation

Pleno enforces `private`, `protected` and `internal` access modifiers at runtime. Reading a private field from outside its class throws an `Error` such as `cannot read private field 'name' of Person outside its class`; calling a private method throws `method access denied`. See [Access Modifiers](/specs/OOP#access-modifiers).
