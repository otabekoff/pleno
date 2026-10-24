# std:os

System-level operations and environment access.

## Import

```pleno
import { OS } from "std:os";
```

## Functions

Environment variables, command-line arguments, the working directory and `exit` are in [std:process](./process.md).

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

### `homedir(): String?`

The current user's home directory, or `null` if it is unknown.

```pleno
print(OS.homedir());
```

### `tmpdir(): String`

The directory for temporary files.

```pleno
let scratch = OS.tmpdir();
```

### `hostname(): String?`

The machine's host name, or `null` if it cannot be found.

```pleno
print("Host: " + (OS.hostname() ?? "unknown"));
```

### `uptime(): Float?`

Seconds since the system started, or `null` on platforms where it is unknown.

```pleno
let hours = (OS.uptime() ?? 0.0) / 3600.0;
```

---
