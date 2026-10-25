# std:os

> **Stability:** stable in 1.0.

Information about the operating system and machine.

## Import

```pleno
import { OS } from "std:os";
```

## Functions

Environment variables, command-line arguments, the working directory and `exit` are in [std:process](./process.md).

### `platform(): String`

Returns the operating system: `"linux"`, `"macos"`, `"windows"`, `"freebsd"`, `"android"` and so on, or `"unknown"`.

```pleno
import { OS } from "std:os";

let plat = OS.platform();
if (plat == "windows") {
    print("Running on Windows");
} else {
    print(`Running on ${plat}`);
}
```

### `arch(): String`

Returns the CPU architecture: `"x86"`, `"x86_64"`, `"arm"`, `"aarch64"`, or `"unknown"`.

```pleno
import { OS } from "std:os";

print("Architecture: " + OS.arch());
```

### `homedir(): String?`

The current user's home directory, or `null` if it is unknown.

```pleno
import { OS } from "std:os";

print(OS.homedir());
```

### `tmpdir(): String`

The directory for temporary files.

```pleno
import { OS } from "std:os";

let scratch = OS.tmpdir();
print(scratch);
```

### `hostname(): String?`

The machine's host name, or `null` if it cannot be found.

```pleno
import { OS } from "std:os";

print("Host: " + (OS.hostname() ?? "unknown"));
```

### `uptime(): Float?`

Seconds since the system started, or `null` on platforms where it is unknown.

```pleno
import { OS } from "std:os";
import { floor } from "std:math";

let hours = (OS.uptime() ?? 0.0) / 3600.0;
print(`Up for ${floor(hours)} hours`);
```

---
