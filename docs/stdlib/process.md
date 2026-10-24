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
import { Process } from "std:process";

print(Process.cwd());  // e.g. /home/user/project
```

### `exit(code: Int = 0): Void`

Terminates the process with the given exit code. Code after the call does not run.

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
