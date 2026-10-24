# Modules & Imports

Each `.pln` file can be a module. A module marks the names it shares with `export`. Another file loads it with `import`.

The import forms are:

```pleno norun
import "path/to/module.pln";                          // run the module, bind no name
import ModuleName from "path/to/module.pln";          // the module's exports as one map
import { symbol, other as alias } from "path/to/module.pln";
import * as Namespace from "path/to/module.pln";      // same as the default form
```

A module runs once, the first time it is imported. Later imports reuse its exports.

## Module Definition

`export` works on `fn`, `class`, `let` and `const` declarations. `export { name as alias };` exports an existing name under another name. Everything else stays private to the file.

A module file may start with `module Name;`. The declaration names the module; it does not change how you import it. It is only allowed in a file that is imported, not in the file you run.

`math_utils.pln`

```pleno norun
module MathUtils;

export fn square(x: Float): Float {
    return x * x;
}

export const PI: Float = 3.14159;

// Private: not exported
fn helperFunction(): Void {
}

export class Calculator {
    public fn add(a: Float, b: Float): Float {
        return a + b;
    }
}
```

## Importing

`main.pln`, in the same folder:

```pleno norun
// The whole module as a map
import MathUtils from "math_utils.pln";
print(MathUtils.square(5.0));   // 25

// Specific items
import { square, PI } from "math_utils.pln";
print(square(5.0));             // 25

// With an alias
import { square as sq } from "math_utils.pln";
print(sq(3.0));                 // 9

// All exports under a namespace
import * as MU from "math_utils.pln";
print(MU.PI);                   // 3.14159
print(MU.Calculator().add(1.0, 2.0));   // 3

// Relative paths start from the importing file's folder
import { User } from "./models/user.pln";
```

Importing a name that the module does not export gives `null`. The `.pln` extension can be left out.

> **Note**: Currently, exported functions and classes also become visible as globals in the importing file, even when you import only other names. Do not rely on this.

## Path Resolution

A path is resolved in this order:

1. A `std:` name is a standard library module.
2. A path that does not start with `./`, `../` or `/`, such as `"mylib"`, is first looked up as a package: `packages/mylib/` (the `main` file named in its `pleno.toml`, or `lib.pln`) or `packages/mylib.pln`. Pleno searches the importing file's folder and then each parent folder.
3. Otherwise the path is relative to the importing file's folder, with `.pln` added if missing. A name without a folder is also looked up in `sources/<name>.pln` under the current working directory.

If nothing matches, the import throws `module not found: <path>`.

## Package Management

`plm init` writes a `pleno.toml`:

```toml
[package]
name = "my-app"
version = "0.1.0"
main = "src/main.pln"

[dependencies]
greet = { path = "../greet" }
http-utils = { git = "https://github.com/user/http-utils", tag = "v1.2.0" }
```

`plm install` puts dependencies into `packages/`. Import them by name:

```pleno norun
import { hello } from "greet";
print(hello("Ada"));
```

There is no public package registry yet. See [Packages](/guide/packages).

## Standard Library Imports

Standard library modules use the `std:` prefix.

```pleno
import { JSON } from "std:json";
import { HTTP } from "std:http";
import { File, Directory } from "std:io";

// The default form binds the whole module
import math from "std:math";
print(math.sin(math.PI / 2));   // 1
```

For the available modules, see the [Standard Library reference](/stdlib/).
