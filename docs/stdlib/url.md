# std:url

URL parsing.

## Import

```pleno
import { Url } from "std:url";
```

`URL` is the same namespace under another name: `import { URL } from "std:url";`.

## Functions

### `parse(url: String): Map`

Parses an absolute URL string and returns its components. Throws an error for an invalid or relative URL.

```pleno
import { Url } from "std:url";

let url = Url.parse("https://user:pass@example.com:8080/path?q=1#section");
print(url.port);  // 8080
print(url.query); // q=1
```

**Returns:**

| Property   | Type    | Example                                                 |
| ---------- | ------- | ------------------------------------------------------- |
| `href`     | String  | `"https://user:pass@example.com:8080/path?q=1#section"` |
| `protocol` | String  | `"https"`                                               |
| `username` | String  | `"user"` (empty string if none)                         |
| `password` | String? | `"pass"`                                                |
| `host`     | String? | `"example.com"`                                         |
| `port`     | Int?    | `8080` (`null` when it is the default for the scheme)   |
| `path`     | String  | `"/path"`                                               |
| `query`    | String? | `"q=1"`                                                 |
| `hash`     | String? | `"section"`                                             |

Missing optional parts are `null`.

**Example:**

```pleno
import { Url } from "std:url";

let url = Url.parse("https://api.example.com/v1/users");
print(url.protocol);  // https
print(url.host);      // api.example.com
print(url.path);      // /v1/users
print(url.port);      // null

try {
    Url.parse("not a url");
} catch (e) {
    print(e.message); // Invalid URL: relative URL without a base
}
```

---
