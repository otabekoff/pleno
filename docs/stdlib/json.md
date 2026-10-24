# std:json

## Contents

- [JSON](#json)

## JSON

```pleno
import { JSON } from "std:json";
```

The `JSON` module provides functions for parsing and stringifying JSON data.

### Methods

#### `static` `fn parse(json: String) -> Any`

Parses a JSON string and returns the corresponding Pleno value (Map, Array, String, Number, Boolean, or Null).

```pleno
let data = JSON.parse(`{"name": "Alice", "age": 30}`);
print(data.name); // "Alice"
```

#### `static` `fn stringify(value: Any) -> String`

Converts a Pleno value into a JSON string.

```pleno
let obj = { "name": "Bob", "age": 25 };
let jsonStr = JSON.stringify(obj);
// '{"name":"Bob","age":25}'
```

#### `static` `fn stringifyPretty(value: Any, indent: Int = 2) -> String`

Converts a Pleno value into a formatted JSON string with indentation.

```pleno
let jsonStr = JSON.stringifyPretty(obj, indent: 4);
```

---
