# Standard Library Reference

Pleno's standard library provides essential utilities for building applications. Import a module with `import { Name } from "std:module";`. Every module offers a namespace (`import { Path } from "std:path";`) and its functions as named imports (`import { join } from "std:path";`).

## Stability in 1.0

A **stable** API keeps its name, parameters and documented behavior in every 1.x release, as described in the [1.x compatibility promise](../specs/compatibility.md). An **experimental** API may change or disappear in any release. Each module page starts with a stability line, and experimental functions inside a stable module are marked **Experimental.** in their description.

Behavior a page calls unspecified (for example the order of `Directory.list`) is not covered, and neither is the exact wording of error messages or of lines printed for people (`Log`, `Benchmark.run`).

| Module                                  | Stability    | Notes                                                                                                                                        |
| --------------------------------------- | ------------ | -------------------------------------------------------------------------------------------------------------------------------------------- |
| [`std:core`](core.md)                   | stable       | `Ok`, `Err`, `Some`, `None`; also globals.                                                                                                   |
| [`std:math`](math.md)                   | stable       |                                                                                                                                              |
| [`std:random`](random.md)               | stable       |                                                                                                                                              |
| [`std:time`](time.md)                   | stable       | All dates are UTC.                                                                                                                           |
| [`std:io`](io.md)                       | stable       | `Directory.list_files` and `Directory.list_directories` are older spellings, not covered; use `listFiles` and `listDirectories`.            |
| [`std:path`](path.md)                   | stable       |                                                                                                                                              |
| [`std:os`](os.md)                       | stable       |                                                                                                                                              |
| [`std:process`](process.md)             | stable       |                                                                                                                                              |
| [`std:dotenv`](dotenv.md)               | stable       |                                                                                                                                              |
| [`std:http`](http.md)                   | stable       | The client (`get`, `post`, `request`, ...) is stable. `Router` and `Server` are experimental: no automated tests serve requests yet, and middleware, static files and cookies are still to come. |
| [`std:net`](net.md)                     | experimental | `listen` and `connect` close the socket at once; there is no `accept`, `read` or `write` yet.                                               |
| [`std:url`](url.md)                     | stable       |                                                                                                                                              |
| [`std:json`](json.md)                   | stable       |                                                                                                                                             |
| [`std:yaml`](yaml.md)                   | stable       |                                                                                                                                              |
| [`std:toml`](toml.md)                   | stable       |                                                                                                                                              |
| [`std:xml`](xml.md)                     | stable       | Input without a complete root element returns `null` instead of throwing; that case is unspecified.                                         |
| [`std:csv`](csv.md)                     | stable       |                                                                                                                                              |
| [`std:base64`](base64.md)               | stable       |                                                                                                                                              |
| [`std:promise`](promise.md)             | stable       | `Promise` and `spawn` are also globals.                                                                                                      |
| [`std:concurrency`](concurrency.md)     | stable       | `MutexProto`, `ChannelProto` and a channel's `iterator`/`next` methods are internal.                                                         |
| [`std:crypto`](crypto.md)               | stable       |                                                                                                                                             |
| [`std:uuid`](uuid.md)                   | stable       |                                                                                                                                              |
| [`std:test`](test.md)                   | stable       |                                                                                                                                              |
| [`std:log`](log.md)                     | stable       | The format of the printed lines may change.                                                                                                  |
| [`std:benchmark`](benchmark.md)         | stable       | The line `run` prints may change; the returned numbers are measurements, not guarantees.                                                     |
| [`std:reflect`](reflect.md)             | stable       | `vmStats` is experimental: its numbers depend on the VM's internals.                                                                         |
| [`std:regex`](regex.md)                 | stable       |                                                                                                                                              |
| [`std:security`](../specs/security-features.md) | experimental | `Sandbox`, described under [Security features](../specs/security-features.md).                                                     |
| `std:gc`                                | experimental | `collect`, `heapSize`, `isEnabled`, `report`, `stats`: views of the VM's memory manager. Not documented further.                            |
| `std:db`                                | experimental | `Database.connect("sqlite:path")` with `execute`, `query` and `close`. Not documented further.                                               |

## Categories

### Core

| Module              | Description                          |
| ------------------- | ------------------------------------ |
| [core](core.md)     | Result and Option types              |
| [math](math.md)     | Mathematical constants and functions |
| [random](random.md) | Random number generation             |
| [time](time.md)     | Date, time, and duration             |

### I/O & System

| Module                | Description                   |
| --------------------- | ----------------------------- |
| [io](io.md)           | File and directory operations |
| [path](path.md)       | Path manipulation             |
| [os](os.md)           | Operating system utilities    |
| [process](process.md) | Process management            |
| [dotenv](dotenv.md)   | Environment file loading      |

### Network

| Module          | Description                             |
| --------------- | --------------------------------------- |
| [http](http.md) | HTTP client and server                  |
| [net](net.md)   | Low-level TCP networking (experimental) |
| [url](url.md)   | URL parsing                             |

### Data Formats

| Module              | Description                    |
| ------------------- | ------------------------------ |
| [json](json.md)     | JSON parsing and serialization |
| [yaml](yaml.md)     | YAML parsing and serialization |
| [toml](toml.md)     | TOML parsing and serialization |
| [xml](xml.md)       | XML parsing                    |
| [csv](csv.md)       | CSV parsing                    |
| [base64](base64.md) | Base64 encoding/decoding       |

### Async & Concurrency

| Module                        | Description                  |
| ----------------------------- | ---------------------------- |
| [promise](promise.md)         | Promise and async utilities  |
| [concurrency](concurrency.md) | Mutex and Channel primitives |

### Security & Crypto

| Module              | Description              |
| ------------------- | ------------------------ |
| [crypto](crypto.md) | Hashing, encryption, JWT |
| [uuid](uuid.md)     | UUID generation          |

### Testing & Debugging

| Module                    | Description             |
| ------------------------- | ----------------------- |
| [test](test.md)           | Testing framework       |
| [log](log.md)             | Logging utilities       |
| [benchmark](benchmark.md) | Performance measurement |

### Advanced

| Module                | Description         |
| --------------------- | ------------------- |
| [reflect](reflect.md) | Runtime reflection  |
| [regex](regex.md)     | Regular expressions |
