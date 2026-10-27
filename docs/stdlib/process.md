# std:process

> **Stability:** stable in 1.0.

Process-level operations and environment access.

## Import

```pleno
import { Process } from "std:process";
```

## Functions

### `cwd(): String`

Returns the current working directory.

```pleno
import { Process } from "std:process";

print(Process.cwd());  // e.g. /home/user/project
```

### `exit(code: Int = 0): Void`

Ends the program with the given exit code, the way Python's `sys.exit` does: it throws a `SystemExit` error (an `Error` with an Int `code`). `finally` blocks and `with` closes run on the way out, so files are closed, but a plain `catch (e)` lets it through, so the program still ends; code after the call does not run. Only `catch (e: SystemExit)` stops it. When nothing does, `pleno run` exits with the code. In a `Sandbox`, `exit` ends only the sandboxed function: the program that called `sandbox.run` gets a catchable `program exited with code N`. In `pleno test` and the playground, `exit(0)` is a normal end.

```pleno
import { Process } from "std:process";

let mode = Process.env("APP_MODE") ?? "dev";
if (mode != "dev" && mode != "prod") {
    print(`Unknown mode: ${mode}`);
    Process.exit(1);
}
print(`Mode: ${mode}`);
Process.exit();
print("never printed");
```

### `args(): String[]`

Returns the command-line arguments. The first entry is the path of the `pleno` executable, followed by its arguments.

```pleno
import { Process } from "std:process";

let args = Process.args();
// pleno run script.pln --verbose
// gives [/path/to/pleno, run, script.pln, --verbose]

if (args.length > 2) {
    print("Script: " + args[2]);
}
```

### `env(name: String): String?`

Returns an environment variable value, or `null` if it is not set.

```pleno
import { Process } from "std:process";

let path = Process.env("PATH");
let apiKey = Process.env("API_KEY") ?? "default";
print(apiKey);
```

### `pid(): Int`

Returns the current process ID.

```pleno
import { Process } from "std:process";

print(`PID: ${Process.pid()}`);
```

---
