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

Importing a name that the module does not export gives `null`; for a `std:` module, `pleno check` warns about it and names the closest export. The `.pln` extension can be left out.

An import makes visible only the names it binds: the names listed in `{ ... }`, or the one namespace name of the default and `* as` forms. Other exports, and everything the module does not export, stay out of the importing file; using them there throws `undefined variable '<name>'`:

```pleno norun
import { PI } from "math_utils.pln";
print(PI);           // 3.14159
print(square(2.0));  // Error: undefined variable 'square'
```

A module's own code sees all of its top-level names, exported or not, plus the built-in globals. The names a module imports stay inside that module too.

> **Note**: A module can still read a top-level name of the program that runs it when the module has no name of its own by that name. Do not rely on this; import what a module needs.

## Module Scope and `internal`

A class member marked `internal` can be used anywhere in the file that declares the class: in its constructor and methods and in other functions of that file. Another file can import the class, create instances and call its public methods, but reading or assigning the internal member there throws, for example `cannot read internal field 'secret' of Box outside its module`.

```pleno norun
// box.pln
export class Box {
    internal secret: Int;
    constructor() { this.secret = 1; }   // allowed: same module
    fn reveal(): Int { return this.secret; }
}

// main.pln
import { Box } from "box.pln";
let b = Box();       // the constructor runs in box.pln
print(b.reveal());   // 1
print(b.secret);     // Error: cannot read internal field 'secret' of Box outside its module
```

In a program of a single file, `internal` behaves like `public`.

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
