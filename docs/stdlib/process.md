# std:process

Process-level operations and environment access.

## Import

```pleno
import { Process } from "std:process";
```

## Functions

### `cwd(): String`

Returns the current working directory.

```pleno
print(Process.cwd());  // "/home/user/project"
```

### `exit(code: Int = 0): Void`

Terminates the process with the specified exit code.

```pleno
if (error) {
    print("Fatal error");
    Process.exit(1);
}
```

### `args(): String[]`

Returns command-line arguments (including the program name).

```pleno
let args = Process.args();
// ["pleno", "run", "script.pln", "--verbose"]

if (args.length > 2) {
    print("Script: " + args[2]);
}
```

### `env(name: String): String?`

Returns an environment variable value, or `null` if not set.

```pleno
let home = Process.env("HOME");
let apiKey = Process.env("API_KEY") ?? "default";
```

### `pid(): Int`

Returns the current process ID.

```pleno
print("PID: " + Process.pid().toString());
```

---
