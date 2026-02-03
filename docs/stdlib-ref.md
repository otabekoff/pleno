# Pleno Standard Library Quick Reference

Complete API reference for all standard library modules.

## Module Index

| Module                             | Description             |
| ---------------------------------- | ----------------------- |
| [std:core](#stdcore)               | Result, Option types    |
| [std:io](#stdio)                   | File and Directory I/O  |
| [std:path](#stdpath)               | Path manipulation       |
| [std:os](#stdos)                   | Operating system info   |
| [std:process](#stdprocess)         | Process management      |
| [std:http](#stdhttp)               | HTTP client/server      |
| [std:net](#stdnet)                 | Low-level TCP           |
| [std:url](#stdurl)                 | URL parsing             |
| [std:json](#stdjson)               | JSON serialization      |
| [std:yaml](#stdyaml)               | YAML serialization      |
| [std:toml](#stdtoml)               | TOML serialization      |
| [std:xml](#stdxml)                 | XML parsing             |
| [std:csv](#stdcsv)                 | CSV parsing             |
| [std:base64](#stdbase64)           | Base64 encoding         |
| [std:math](#stdmath)               | Mathematical functions  |
| [std:random](#stdrandom)           | Random generation       |
| [std:time](#stdtime)               | Date/time utilities     |
| [std:crypto](#stdcrypto)           | Cryptography            |
| [std:uuid](#stduuid)               | UUID generation         |
| [std:regex](#stdregex)             | Regular expressions     |
| [std:test](#stdtest)               | Testing framework       |
| [std:log](#stdlog)                 | Logging                 |
| [std:benchmark](#stdbenchmark)     | Performance measurement |
| [std:promise](#stdpromise)         | Async utilities         |
| [std:concurrency](#stdconcurrency) | Mutex, Channel          |
| [std:reflect](#stdreflect)         | Runtime reflection      |
| [std:dotenv](#stddotenv)           | Environment files       |

---

## std:core

```pleno
import { Result, Option, Ok, Err, Some, None } from "std:core";
```

| Type           | Description      |
| -------------- | ---------------- |
| `Result<T, E>` | Success or error |
| `Option<T>`    | Value or nothing |

---

## std:io

```pleno
import { File, Directory } from "std:io";
```

| Function                 | Description        |
| ------------------------ | ------------------ |
| `File.read(path)`        | Read file contents |
| `File.write(path, data)` | Write to file      |
| `File.exists(path)`      | Check existence    |
| `Directory.list(path)`   | List directory     |
| `Directory.create(path)` | Create directory   |

---

## std:path

```pleno
import { Path } from "std:path";
```

| Function              | Description        |
| --------------------- | ------------------ |
| `Path.join(...parts)` | Join path segments |
| `Path.dirname(path)`  | Get directory      |
| `Path.basename(path)` | Get filename       |
| `Path.extname(path)`  | Get extension      |
| `Path.resolve(path)`  | Get absolute path  |

---

## std:os

```pleno
import { OS } from "std:os";
```

| Function        | Description      |
| --------------- | ---------------- |
| `OS.platform()` | OS name          |
| `OS.arch()`     | CPU architecture |
| `OS.hostname()` | System hostname  |
| `OS.homedir()`  | Home directory   |

---

## std:process

```pleno
import { Process } from "std:process";
```

| Function             | Description       |
| -------------------- | ----------------- |
| `Process.cwd()`      | Current directory |
| `Process.exit(code)` | Exit process      |
| `Process.args()`     | CLI arguments     |
| `Process.env(name)`  | Get env variable  |
| `Process.pid()`      | Process ID        |

---

## std:http

```pleno
import { HTTP } from "std:http";
```

| Function               | Description    |
| ---------------------- | -------------- |
| `HTTP.get(url)`        | GET request    |
| `HTTP.post(url, body)` | POST request   |
| `HTTP.put(url, body)`  | PUT request    |
| `HTTP.delete(url)`     | DELETE request |
| `HTTP.Server(router)`  | Create server  |

---

## std:json

```pleno
import { JSON } from "std:json";
```

| Function              | Description       |
| --------------------- | ----------------- |
| `JSON.parse(str)`     | Parse JSON string |
| `JSON.stringify(val)` | Convert to JSON   |

---

## std:math

```pleno
import { Math } from "std:math";
```

| Constant  | Value      |
| --------- | ---------- |
| `Math.PI` | 3.14159... |
| `Math.E`  | 2.71828... |

| Function         | Description    |
| ---------------- | -------------- |
| `Math.abs(n)`    | Absolute value |
| `Math.floor(n)`  | Round down     |
| `Math.ceil(n)`   | Round up       |
| `Math.sqrt(n)`   | Square root    |
| `Math.pow(b, e)` | Power          |
| `Math.sin(rad)`  | Sine           |
| `Math.cos(rad)`  | Cosine         |

---

## std:random

```pleno
import { Random } from "std:random";
```

| Function                   | Description      |
| -------------------------- | ---------------- |
| `Random.random()`          | Float 0.0-1.0    |
| `Random.randint(min, max)` | Random integer   |
| `Random.choice(arr)`       | Random element   |
| `Random.shuffle(arr)`      | Shuffle in place |

---

## std:time

```pleno
import { DateTime, Duration } from "std:time";
```

| Function                   | Description       |
| -------------------------- | ----------------- |
| `DateTime.now()`           | Current date/time |
| `DateTime.parse(str, fmt)` | Parse date        |
| `Duration.seconds(n)`      | Create duration   |

---

## std:crypto

```pleno
import { Crypto } from "std:crypto";
```

| Function                              | Description |
| ------------------------------------- | ----------- |
| `Crypto.hash(algo, data)`             | Hash data   |
| `Crypto.hmac(algo, key, data)`        | HMAC        |
| `Crypto.encrypt(algo, key, iv, data)` | Encrypt     |
| `Crypto.decrypt(algo, key, iv, data)` | Decrypt     |
| `Crypto.jwtSign(payload, secret)`     | Sign JWT    |
| `Crypto.jwtVerify(token, secret)`     | Verify JWT  |

---

## std:uuid

```pleno
import { UUID } from "std:uuid";
```

| Function    | Description       |
| ----------- | ----------------- |
| `UUID.v4()` | Random UUID       |
| `UUID.v7()` | Time-ordered UUID |

---

## std:regex

```pleno
import { Regex } from "std:regex";
```

| Method                     | Description      |
| -------------------------- | ---------------- |
| `Regex.test(str)`          | Check if matches |
| `Regex.match(str)`         | Get first match  |
| `Regex.findAll(str)`       | Get all matches  |
| `Regex.replace(str, repl)` | Replace matches  |

---

## std:test

```pleno
import { describe, test, assert } from "std:test";
```

| Function             | Description   |
| -------------------- | ------------- |
| `describe(name, fn)` | Test suite    |
| `test(name, fn)`     | Test case     |
| `assert.equal(a, b)` | Assert equal  |
| `assert.true(val)`   | Assert true   |
| `assert.throws(fn)`  | Assert throws |

---

## std:concurrency

```pleno
import { Channel, Mutex } from "std:concurrency";
```

| Type      | Description           |
| --------- | --------------------- |
| `Channel` | Message passing       |
| `Mutex`   | Mutual exclusion lock |

---

## std:reflect

```pleno
import { Reflect } from "std:reflect";
```

| Function                              | Description      |
| ------------------------------------- | ---------------- |
| `Reflect.getTypeName(val)`            | Get type name    |
| `Reflect.getFields(obj)`              | Get field names  |
| `Reflect.getMethods(obj)`             | Get method names |
| `Reflect.callMethod(obj, name, args)` | Call method      |

---

For detailed documentation, see the [Standard Library Reference](/stdlib/).
