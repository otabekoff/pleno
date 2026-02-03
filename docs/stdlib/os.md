# std:os

System-level operations and environment access.

## Import

```pleno
import { OS } from "std:os";
```

## Functions

### `exit(code: Int): Void`

Terminates the program with the specified exit code.

```pleno
if (error) {
    print("Fatal error!");
    OS.exit(1);
}
```

### `getEnv(name: String): String?`

Returns the value of an environment variable, or `null` if not set.

```pleno
let home = OS.getEnv("HOME");
let apiKey = OS.getEnv("API_KEY") ?? "default-key";
```

### `setEnv(name: String, value: String): Void`

Sets an environment variable.

```pleno
OS.setEnv("MY_VAR", "my_value");
```

### `args(): String[]`

Returns command-line arguments passed to the program.

```pleno
let args = OS.args();
if (args.length > 0) {
    print("First argument: " + args[0]);
}
```

### `cwd(): String`

Returns the current working directory.

```pleno
print("Working directory: " + OS.cwd());
```

### `platform(): String`

Returns the operating system platform (`"linux"`, `"macos"`, `"windows"`).

```pleno
let plat = OS.platform();
if (plat == "windows") {
    print("Running on Windows");
}
```

### `arch(): String`

Returns the CPU architecture (`"x86_64"`, `"aarch64"`, etc.).

```pleno
print("Architecture: " + OS.arch());
```

---
