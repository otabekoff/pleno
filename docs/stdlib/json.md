# std:json

> **Stability:** stable in 1.0.

## Contents

- [JSON](#json)

## JSON

```pleno
import { JSON } from "std:json";
```

The `JSON` namespace parses and stringifies JSON data. The same functions can also be imported directly: `import { parse, stringify, stringifyPretty } from "std:json";`.

### Methods

#### `static` `fn parse(json: String) -> Any`

Parses a JSON string and returns the matching Pleno value: Map, Array, String, Int, Float, Bool or `null`. Invalid JSON throws an error.

```pleno
import { JSON } from "std:json";

let data = JSON.parse(`{"name": "Alice", "age": 30}`);
print(data.name); // Alice
print(data.age);  // 30

try {
    JSON.parse("{bad");
} catch (e) {
    print(e.message); // JSON parse error: key must be a string at line 1 column 2
}
```

#### `static` `fn stringify(value: Any) -> String`

Converts a Pleno value into a compact JSON string:

| Pleno value          | JSON                                                        |
| -------------------- | ----------------------------------------------------------- |
| `null`, Bool         | `null`, `true`, `false`                                     |
| Int, Float           | a number; a NaN or infinite Float becomes `null`            |
| String, Char         | a string                                                    |
| Array                | an array                                                    |
| Set                  | an array of its elements, in insertion order                |
| Map                  | an object; keys keep their insertion order                  |
| Class instance       | an object of its public fields, in the order they were set  |
| Function, class, ... | `null`                                                      |

For a class instance, fields declared `private`, `protected` or `internal` are left out, and so are getters and methods. `JSON.parse` gives back a map, not an instance of the class. A value nested more than 512 levels deep, such as a map that contains itself, throws an error.

```pleno
import { JSON } from "std:json";

let obj = { "name": "Bob", "age": 25 };
print(JSON.stringify(obj)); // {"name":"Bob","age":25}

class User {
    name: String;
    private password: String;
    constructor(name: String, password: String) {
        this.name = name;
        this.password = password;
    }
}
print(JSON.stringify(User("ann", "secret")));     // {"name":"ann"}
print(JSON.stringify({ "tags": Set(["a", "b"]) })); // {"tags":["a","b"]}
```

#### `static` `fn stringifyPretty(value: Any, indent: Int = 2) -> String`

Converts a Pleno value into JSON on several lines, indenting nested values by `indent` spaces (0 to 16). The indent can be passed by position or by name.

```pleno
import { JSON } from "std:json";

let obj = { "name": "Bob" };
print(JSON.stringifyPretty(obj, indent: 4));
// {
//     "name": "Bob"
// }
```

---
