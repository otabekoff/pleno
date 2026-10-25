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

Converts a Pleno value into a compact JSON string. Map keys keep their insertion order. Null, Bool, Int, Float, String, Array and Map values are converted; functions become `null`.

Class instances and sets also become `null` today (`JSON.stringify(Point(1, 2))` is `null`). This is unspecified behavior, not covered by the [1.x compatibility promise](../specs/compatibility.md): a later release may write their fields instead. Convert them to a map or an array first, for example with a `toMap()` method of your own or `set.toArray()`.

```pleno
import { JSON } from "std:json";

let obj = { "name": "Bob", "age": 25 };
print(JSON.stringify(obj)); // {"name":"Bob","age":25}
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
