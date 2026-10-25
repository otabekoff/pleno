# std:regex

> **Stability:** stable in 1.0.

The Regex module provides regular expression matching and manipulation.

## Import

```pleno
import { Regex } from "std:regex";
```

## Creating a Regex

`Regex(pattern)` creates a regex object. Write patterns as raw strings, `r"..."`, so backslashes reach the regex engine unchanged.

```pleno
import { Regex } from "std:regex";

let digits = Regex(r"\d+");                     // One or more digits
let email = Regex(r"(?i)[a-z]+@[a-z]+\.com");   // Case-insensitive
print(digits.pattern);  // \d+
```

### Flags

`Regex(pattern, flags)` takes an optional string of flags, in any order:

| Flag | Meaning                                                              |
| ---- | -------------------------------------------------------------------- |
| `i`  | Case-insensitive matching                                            |
| `m`  | Multi-line mode: `^` and `$` match at line breaks                    |
| `s`  | `.` also matches newlines                                            |
| `x`  | Verbose mode: whitespace and `#` comments in the pattern are ignored |

Any other letter throws an error (`invalid regex flag 'g': expected i, m, s or x`), and so does an invalid pattern, when the regex is created. The flags are available as `regex.flags` (`""` when none are given). The same flags can also be set inside the pattern: `(?i)`, `(?m)`, `(?s)` and `(?x)`.

```pleno
import { Regex } from "std:regex";

let word = Regex("hello", "i");
print(word.test("Say HELLO"));               // true
print(Regex("^b$", "m").test("a\nb\nc"));   // true
print(Regex(r"(?i)hello").test("HELLO"));    // true
print(word.flags);                           // i
```

`match` returns the first match. `findAll` and `replace` work on all matches, so no global flag is needed.

## Methods

### `test(input: String): Bool`

Returns `true` if the pattern matches anywhere in the input string.

```pleno
import { Regex } from "std:regex";

let pattern = Regex(r"\d+");
print(pattern.test("abc123"));  // true
print(pattern.test("abc"));     // false
```

### `match(input: String): Match?`

Returns the first match or `null` if no match found.

```pleno
import { Regex } from "std:regex";

let pattern = Regex(r"(\w+)@(\w+)");
let m = pattern.match("contact@pleno.dev");
if (m != null) {
    print(m.value);     // contact@pleno
    print(m.groups[0]); // contact
    print(m.groups[1]); // pleno
}
```

### `findAll(input: String): Match[]`

Returns an array of all matches.

```pleno
import { Regex } from "std:regex";

let pattern = Regex(r"\d+");
let matches = pattern.findAll("a1b2c3");
print(matches.length);    // 3
print(matches[0].value);  // 1
print(matches[2].value);  // 3
```

### `replace(input: String, replacement: String): String`

Replaces all matches with the replacement string. `$1`, `$2`, ... in the replacement insert capture groups.

```pleno
import { Regex } from "std:regex";

print(Regex(r"\s+").replace("hello   world", " "));         // hello world
print(Regex(r"(\w+)@(\w+)").replace("ann@site", "$2 at $1"));  // site at ann
```

### `split(input: String): String[]`

Splits the input string by the pattern.

```pleno
import { Regex } from "std:regex";

let pattern = Regex(r",\s*");
print(pattern.split("a, b,  c"));  // [a, b, c]
```

## Static Functions

### `Regex.isMatch(pattern: String, input: String): Bool`

Checks a pattern without creating a regex object.

```pleno
import { Regex } from "std:regex";

print(Regex.isMatch(r"^\d+$", "123"));  // true
```

### `Regex.replace(pattern: String, input: String, replacement: String): String`

Replaces all matches without creating a regex object.

```pleno
import { Regex } from "std:regex";

print(Regex.replace("o", "foo", "0"));  // f00
```

## Match Object

The `Match` object returned by `match()` and `findAll()` contains:

| Property   | Type       | Description                                |
| ---------- | ---------- | ------------------------------------------ |
| `value`    | `String`   | The matched text                           |
| `index`    | `Int`      | Start position in input, in characters     |
| `groups`   | `String[]` | Captured groups                            |
| `group(i)` | `String?`  | Group `i`; `group(0)` is the whole match   |

A match can also be read like an array: `m[0]` is the whole match, `m[1]` the first group, and `m.length` the number of entries.

```pleno
import { Regex } from "std:regex";

let m = Regex(r"(\d{4})-(\d{2})-(\d{2})").match("2026-02-03");
print(m.value);     // 2026-02-03
print(m.groups[0]); // 2026
print(m.group(2));  // 02
print(m[3]);        // 03
```
