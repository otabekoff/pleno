---
title: "Pleno 0.4.0: resources that close themselves, sockets, and a web framework"
date: 2026-10-29T12:00:00+05:00
author: Otabek
description: The with statement, TCP sockets, HTTP middleware, static files and cookies, Git packages without a registry, and Linux builds that run on distributions back to 2014.
---

Pleno 0.4.0 is out. It should be the last release before 1.0, and the language reference is now frozen for 1.x.

```sh
brew install otabekoff/tap/pleno        # macOS, Linux
```

```powershell
scoop bucket add pleno https://github.com/otabekoff/scoop-bucket
scoop install pleno                      # Windows
```

Or download it from [the release page](https://github.com/otabekoff/pleno/releases/tag/v0.4.0).
[Installation](/guide/installation) lists the other ways.

## Upgrading from 0.3

- `with` is now a keyword, so it can no longer name a variable. Methods named `with` still work.
- `exit(code)` throws a `SystemExit` error, as Python's `sys.exit` does. `finally` blocks and `with`
  cleanup now run before the program ends.
- `Server.listen(port)` listens on `127.0.0.1` (this machine only). Pass `host: "0.0.0.0"` to accept
  connections from other machines.
- Some output changes:
  - `print(10 / 2)` prints `5.0`;
  - strings inside arrays and maps print quoted;
  - `sort()` sorts numbers by value, so `[10, 9, 2].sort()` gives `[2, 9, 10]` (it gave
    `[10, 2, 9]`), and it uses the comparator you pass;
  - an unknown escape such as `"\d"` is a compile error.

  The [changelog](https://github.com/otabekoff/pleno/blob/main/CHANGELOG.md) lists every change,
  and marks the ones that alter what programs print or accept in bold.
- Compile `.plc` files again: the bytecode format is now 7.

## The `with` statement

```pleno norun
with (input = File.open("in.txt"), out = File.open("out.txt", "w")) {
    out.write(input.read());
}
```

Resources close in reverse order when the block ends, including on `return`, `break` or an error.
A resource is anything with a `close()` method, including your own classes.

## Networking and the web

- **`std:net`:** real TCP sockets. `Net.listen("127.0.0.1", 0)` and `Net.connect(host, port)`
  return promises, so a server and a client can run in the same program.
- **`std:http`:** `Router` and `Server` are stable, and now have:
  - middleware (`router.use`) and an error handler (`router.onError`);
  - static files with `ETag` and `304` responses, and path checks that a request can't escape;
  - cookies and redirects.

  [`examples/web_app`](https://github.com/otabekoff/pleno/tree/main/examples/web_app) puts them
  together.

## Packages without a registry

A package is a Git repository, and a version is a tag `vX.Y.Z`, as with Go modules. Nobody has to
run a server:

```sh
plm add github:ada/greet@^1.2
```

`pleno.lock` records the exact commit and a checksum of every package.

## Safer and more correct

- The Linux programs need only glibc 2.17, so they run on Ubuntu 22.04, Debian 12, RHEL 9, and
  anything from 2014 on. 0.3.0 needed glibc 2.39.
- Long fuzzing runs found bugs, now fixed:
  - a loop that could slip past the sandbox's instruction limit;
  - a crash in `super()`;
  - wrong calls in files with more than 256 functions;
  - unbounded loop unrolling in the optimizer.
- `pleno check` and the editor catch more mistakes before you run anything: methods of a known
  class, return types, and compile errors such as assigning to an immutable binding.
- `pleno format`, and so format-on-save, no longer changes what a program computes. It wrote
  `n *= 3 + 1` as `n = n * 3 + 1`; compound assignments now stay as they are.
- Errors in an imported module point at the module's file and line, in the message and in the
  backtrace.
- New methods: `array.sortBy(key)`, `array.concat(other)`, `s.padStart(width)` and
  `s.padEnd(width)`.

## Next: 1.0

What is left is in [the roadmap](https://github.com/otabekoff/pleno/blob/main/ROADMAP.md). The main
item is a full day of fuzzing that finds nothing.
