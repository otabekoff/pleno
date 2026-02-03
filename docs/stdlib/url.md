# std:url

URL parsing and manipulation utilities.

## Import

```pleno
import { Url } from "std:url";
```

## Functions

### `parse(url: String): Map`

Parses a URL string and returns its components.

```pleno
let url = Url.parse("https://user:pass@example.com:8080/path?q=1#section");
```

**Returns:**

| Property   | Type    | Example                                                 |
| ---------- | ------- | ------------------------------------------------------- |
| `href`     | String  | `"https://user:pass@example.com:8080/path?q=1#section"` |
| `protocol` | String  | `"https"`                                               |
| `username` | String  | `"user"`                                                |
| `password` | String? | `"pass"`                                                |
| `host`     | String? | `"example.com"`                                         |
| `port`     | Int?    | `8080`                                                  |
| `path`     | String  | `"/path"`                                               |
| `query`    | String? | `"q=1"`                                                 |
| `hash`     | String? | `"section"`                                             |

**Example:**

```pleno
let url = Url.parse("https://api.example.com/v1/users");
print(url.protocol);  // "https"
print(url.host);      // "api.example.com"
print(url.path);      // "/v1/users"
```

---
