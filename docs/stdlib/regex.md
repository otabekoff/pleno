# std:regex

The Regex module provides regular expression matching and manipulation.

## Import

```pleno
import { Regex } from "std:regex";
```

## Creating a Regex

```pleno
let pattern = Regex("\\d+");        // Matches one or more digits
let email = Regex("[a-z]+@[a-z]+\\.com", "i");  // Case-insensitive
```

### Flags

- `"i"` - Case-insensitive matching
- `"g"` - Global matching (find all)
- `"m"` - Multiline mode

## Methods

### `test(input: String): Bool`

Returns `true` if the pattern matches anywhere in the input string.

```pleno
let pattern = Regex("\\d+");
pattern.test("abc123");  // true
pattern.test("abc");     // false
```

### `match(input: String): Match?`

Returns the first match or `null` if no match found.

```pleno
let pattern = Regex("(\\w+)@(\\w+)");
let m = pattern.match("contact@pleno.dev");
if (m != null) {
    print(m.value);     // "contact@pleno"
    print(m.groups[0]); // "contact"
    print(m.groups[1]); // "pleno"
}
```

### `findAll(input: String): Match[]`

Returns an array of all matches.

```pleno
let pattern = Regex("\\d+");
let matches = pattern.findAll("a1b2c3");
// matches[0].value == "1"
// matches[1].value == "2"
// matches[2].value == "3"
```

### `replace(input: String, replacement: String): String`

Replaces all matches with the replacement string.

```pleno
let pattern = Regex("\\s+");
pattern.replace("hello   world", " ");  // "hello world"
```

### `split(input: String): String[]`

Splits the input string by the pattern.

```pleno
let pattern = Regex(",\\s*");
pattern.split("a, b,  c");  // ["a", "b", "c"]
```

## Match Class

The `Match` object returned by `match()` and `findAll()` contains:

| Property | Type       | Description             |
| -------- | ---------- | ----------------------- |
| `value`  | `String`   | The matched text        |
| `index`  | `Int`      | Start position in input |
| `groups` | `String[]` | Captured groups         |

```pleno
let m = Regex("(\\d{4})-(\\d{2})-(\\d{2})").match("2026-02-03");
print(m.value);     // "2026-02-03"
print(m.groups[0]); // "2026"
print(m.groups[1]); // "02"
print(m.groups[2]); // "03"
```

---
