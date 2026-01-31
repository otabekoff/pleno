# log

## Contents

- [Log](#log)

## Log

```pleno
import Log from "std:log";
```

Structured logging module with configurable log levels and colorized output.

### Log Levels

| Level | Value | Color | Description |
|-------|-------|-------|-------------|
| TRACE | 0 | Gray | Very detailed debugging |
| DEBUG | 1 | Cyan | Debug information |
| INFO | 2 | Green | General information (default) |
| WARN | 3 | Yellow | Warnings |
| ERROR | 4 | Red | Errors |
| FATAL | 5 | Magenta | Fatal errors |
| OFF | 6 | - | Disable all logging |

### Methods

#### `fn trace(...args)`
Log a trace-level message.

```pleno
Log.trace("Entering function", functionName);
```

#### `fn debug(...args)`
Log a debug-level message.

```pleno
Log.debug("Variable value:", x);
```

#### `fn info(...args)`
Log an info-level message.

```pleno
Log.info("Server started on port", 8080);
```

#### `fn warn(...args)`
Log a warning-level message.

```pleno
Log.warn("Deprecated API used");
```

#### `fn error(...args)`
Log an error-level message.

```pleno
Log.error("Failed to connect:", error);
```

#### `fn fatal(...args)`
Log a fatal-level message.

```pleno
Log.fatal("Unrecoverable error, shutting down");
```

#### `fn setLevel(level: string | int)`
Set the minimum log level. Messages below this level are suppressed.

```pleno
Log.setLevel("debug");  // Show debug and above
Log.setLevel(4);        // Show error and above only
Log.setLevel("off");    // Disable all logging
```

#### `fn getLevel(): string`
Get the current log level as a lowercase string.

```pleno
let level = Log.getLevel();  // "info"
```

#### `fn create(prefix: string): object`
Create a child logger with a prefix.

```pleno
let dbLog = Log.create("[DB]");
dbLog.info("Connected to database");  // [DB] Connected to database
```

### Example

```pleno
import Log from "std:log";

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

```
2026-02-01 01:49:11.875 [INFO] Application started
2026-02-01 01:49:11.876 [DEBUG] Config: {port: 8080, debug: true}
2026-02-01 01:49:11.876 [WARN] Cache miss for key: user:123
2026-02-01 01:49:11.876 [ERROR] Request failed: {status: 500, message: Internal error}
```

---
