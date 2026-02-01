# Pleno Standard Library Reference

This document provides a reference for the Pleno Standard Library modules.

## Modules

- [std:io](#stdio) - File System and I/O
- [std:net](#stdnet) - Networking (TCP)
- [std:http](#stdhttp) - HTTP Client/Server
- [std:math](#stdmath) - Mathematical functions
- [std:time](#stdtime) - Time and Duration
- [std:json](#stdjson) - JSON parsing and stringification
- [std:os](#stdos) - Operating System interaction
- [std:path](#stdpath) - File path manipulation
- [std:process](#stdprocess) - Child process management
- [std:test](#stdtest) - Testing framework
- [std:concurrency](#stdconcurrency) - Concurrency primitives

---

## std:io
See [Specs](../specs.md#135-file-io-stdio) for details.

## std:net
See [Specs](../specs.md#139-low-level-networking) for details.

## std:http
See [Specs](../specs.md#136-http-client) for details.

## std:math

Provides basic mathematical functions and constants.

```pleno
import { abs, PI, max } from "std:math";
```

### Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `abs` | `abs(n: Number) -> Number` | Absolute value |
| `min` | `min(a, b, ...) -> Number` | Smallest of arguments |
| `max` | `max(a, b, ...) -> Number` | Largest of arguments |
| `floor` | `floor(n: Number) -> Number` | Round down |
| `ceil` | `ceil(n: Number) -> Number` | Round up |
| `round` | `round(n: Number) -> Number` | Round to nearest integer |
| `trunc` | `trunc(n: Number) -> Number` | Truncate decimal part |
| `sqrt` | `sqrt(n: Number) -> Number` | Square root |
| `pow` | `pow(base, exp) -> Number` | Power |
| `exp` | `exp(n) -> Number` | e^n |
| `log` | `log(n) -> Number` | Natural logarithm |
| `log10` | `log10(n) -> Number` | Base-10 logarithm |
| `sin` | `sin(rad) -> Number` | Sine |
| `cos` | `cos(rad) -> Number` | Cosine |
| `tan` | `tan(rad) -> Number` | Tangent |
| `atan2` | `atan2(y, x) -> Number` | Arc tangent of y/x |
| `sign` | `sign(n) -> Number` | Sign of number (-1, 0, 1) |
| `random` | `random() -> Float` | Random float between 0.0 and 1.0 |
| `randomInt` | `randomInt(min, max) -> Int` | Random integer in [min, max) |

### Constants

| Constant | Description |
|----------|-------------|
| `PI` | 3.14159... |
| `E` | 2.71828... |
| `LN2` | Natural log of 2 |
| `LN10` | Natural log of 10 |
| `SQRT2` | Square root of 2 |

## std:time

Time manipulation utilities.

```pleno
import { now, DateTime, setTimeout, sleep } from "std:time";
```

### Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `now` | `now() -> Int` | Current timestamp (ms since epoch) |
| `sleep` | `sleep(ms: Int)` | Sleep for `ms` milliseconds (sync) |
| `setTimeout`| `setTimeout(callback, ms, args...) -> Int` | Schedule callback (returns timer ID) |

### DateTime Class

| Method | Signature | Description |
|--------|-----------|-------------|
| `now` | `DateTime.now() -> DateTime` | Current date/time object |

## std:json

JSON processing.

```pleno
import { JSON } from "std:json";
```

### JSON Object

| Method | Signature | Description |
|--------|-----------|-------------|
| `parse` | `JSON.parse(str: String) -> Value` | Parse JSON string |
| `stringify`| `JSON.stringify(val: Value) -> String` | Convert value to JSON string |

## std:os

Operating System information.

```pleno
import { platform, arch, homedir } from "std:os";
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `platform` | `platform() -> String` | OS name (linux, macos, windows) |
| `arch` | `arch() -> String` | CPU architecture (x86_64, aarch64) |
| `hostname` | `hostname() -> String` | System hostname |
| `homedir` | `homedir() -> String` | Current user's home directory |
| `tmpdir` | `tmpdir() -> String` | System temporary directory |
| `uptime` | `uptime() -> Int` | System uptime in seconds |

## std:path

File path manipulation.

```pleno
import { join, resolve, basename } from "std:path";
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `join` | `join(paths...) -> String` | Join path segments |
| `resolve` | `resolve(paths...) -> String` | Resolve absolute path |
| `dirname` | `dirname(path) -> String` | Directory name of path |
| `basename` | `basename(path) -> String` | File name of path |
| `extname` | `extname(path) -> String` | Extension of path |
| `exists` | `exists(path) -> Bool` | Check if path exists |
| `separator`| `String` | Path separator ('/' or '\') |

## std:process

Process interaction.

```pleno
import { env, exit, args } from "std:process";
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `exit` | `exit(code: Int)` | Exit process with code |
| `cwd` | `cwd() -> String` | Get current working directory |
| `pid` | `pid() -> Int` | Process ID |
| `args` | `args() -> [String]` | Command line arguments |
| `env` | `env() -> Map<String, String>` | Environment variables |

## std:test

Testing framework. See [Specs](../specs.md#1310-testing-framework-stdtest).

## std:concurrency

Concurrency primitives.

```pleno
import { Channel, Mutex } from "std:concurrency";
```

| Class | Description |
|-------|-------------|
| `Channel` | Message passing channel (`send`, `recv`, `close`) |
| `Mutex` | Mutual exclusion lock (`lock`, `unlock`) |
