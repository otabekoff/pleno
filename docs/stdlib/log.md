# std:log

> **Stability:** stable in 1.0. The format of the printed lines may change.

## Contents

- [Log](#log)

## Log

```pleno
import { Log } from "std:log";
```

Leveled logging to stderr. The functions can also be imported one by one, e.g. `import { info, warn } from "std:log"`.

### Log Levels

| Level | Value | Description                   |
| ----- | ----- | ----------------------------- |
| TRACE | 0     | Very detailed debugging       |
| DEBUG | 1     | Debug information             |
| INFO  | 2     | General information (default) |
| WARN  | 3     | Warnings                      |
| ERROR | 4     | Errors                        |
| FATAL | 5     | Fatal errors                  |
| OFF   | 6     | Disable all logging           |

The level belongs to the script. It does not depend on `RUST_LOG`.

### Methods

Each logging method takes any number of values and joins them with spaces.

#### `fn trace(...args)`

Log a trace-level message.

```pleno
import { Log } from "std:log";

Log.setLevel("trace");
Log.trace("Entering function", "parseConfig");
```

#### `fn debug(...args)`

Log a debug-level message.

```pleno
import { Log } from "std:log";

let x = 42;
Log.setLevel("debug");
Log.debug("Variable value:", x);
```

#### `fn info(...args)`

Log an info-level message.

```pleno
import { Log } from "std:log";

Log.info("Server started on port", 8080);
```

#### `fn warn(...args)`

Log a warning-level message.

```pleno
import { Log } from "std:log";

Log.warn("Deprecated API used");
```

#### `fn error(...args)`

Log an error-level message.

```pleno
import { Log } from "std:log";

Log.error("Failed to connect:", "connection refused");
```

#### `fn fatal(...args)`

Log a fatal-level message. It only logs; it does not stop the program.

```pleno
import { Log } from "std:log";

Log.fatal("Unrecoverable error, shutting down");
```

#### `fn setLevel(level: String | Int)`

Set the minimum log level. Messages below this level are dropped. Takes a level name (`"trace"` to `"fatal"`, or `"off"`) or a number from 0 to 6.

```pleno
import { Log } from "std:log";

Log.setLevel("debug");  // Show debug and above
Log.setLevel(4);        // Show error and above only
Log.setLevel("off");    // Disable all logging
```

#### `fn getLevel(): String`

Get the current log level as a lowercase string.

```pleno
import { Log } from "std:log";

print(Log.getLevel());  // info
```

#### `fn create(prefix: String): Map`

Create a logger that starts each message with `prefix`. It has the methods `trace`, `debug`, `info`, `warn`, `error`, and `fatal`, and uses the global level.

```pleno
import { Log } from "std:log";

let dbLog = Log.create("[DB]");
dbLog.info("Connected to database");  // ... INFO  - [DB] Connected to database
```

### Example

```pleno
import { Log } from "std:log";

// Set level to debug for development
Log.setLevel("debug");

// Log different types of data
Log.info("Application started");
Log.debug("Config:", { port: 8080, debug: true });
Log.warn("Cache miss for key:", "user:123");
Log.error("Request failed:", { status: 500, message: "Internal error" });

// Create module-specific loggers
let authLog = Log.create("[Auth]");
authLog.info("User logged in:", "john@example.com");

// In production, set to warn or error
Log.setLevel("warn");
```

### Output Format

Each line has the local time, the level, and the message:

```
[2026-02-01T01:49:11] INFO  - Application started
[2026-02-01T01:49:11] DEBUG - Config: {port: 8080, debug: true}
[2026-02-01T01:49:11] WARN  - Cache miss for key: user:123
[2026-02-01T01:49:11] ERROR - Request failed: {status: 500, message: Internal error}
[2026-02-01T01:49:11] INFO  - [Auth] User logged in: john@example.com
```
