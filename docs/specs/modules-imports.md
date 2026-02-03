# Modules & Imports

Current implementation supports:

```pleno
import "path/to/module.pln";
import ModuleName from "path/to/module.pln";
import { symbol, other as alias } from "path/to/module.pln";
import * as ModuleName from "path/to/module.pln";
```

Exports are limited to `export` on `fn`, `class`, `let`, and `const` declarations plus `export { name as alias };`. Import returns a module namespace map (accessed via `ModuleName.field`). Module declarations and structured imports are implemented for string paths; relative paths and `sources/` lookups are supported. Package/registry resolution is not implemented yet.

### 8.1 Module Definition

```pleno
// File: math_utils.pln
module MathUtils;

// Public export
export fn square(x: Float): Float {
    return x * x;
}

export const PI: Float = 3.14159;

// Private (not exported)
fn helperFunction(): Void {
    // Only available within module
}

export class Calculator {
    public fn add(a: Float, b: Float): Float {
        return a + b;
    }
}
```

### 8.2 Importing

```pleno
// Import entire module
import "math_utils.pln";
// Note: If no alias is provided, the module needs to be accessed
// via its defined module name if present, or by filename.

// Import specific items
import { square, PI } from "math_utils.pln";
square(5.0);

// Import with alias
import { square as sq } from "math_utils.pln";
sq(5.0);

// Import all exported items with a namespace alias
import * as Math from "math_utils.pln";
Math.square(5.0);
Math.PI;

// Relative imports
import { User } from "./models/user.pln";
import { Database } from "../db/connection.pln";
```

### 8.3 Package Management

```toml
# pleno.toml
[package]
name = "my-app"
version = "1.0.0"
entry = "main.pln"

[dependencies]
http-client = "^2.0.0"
json-parser = "^1.5.0"
```

// Using external packages
import { HttpClient } from "http-client";
import { parse } from "json-parser";

````
### 8.4 Standard Library Imports

Pleno's standard library modules are accessed via the `std:` namespace prefix.

```pleno
import { JSON } from "std:json";
import { HTTP } from "std:http";
import { File, Directory } from "std:io";

// Native modules are often prefixed with _ in internal code
import _math from "std:math";
let res = _math.sin(_math.PI / 2);
````

For more details on available modules, see the [Standard Library Reference](../stdlib-ref.md).
