# std:base64

Base64 encoding and decoding.

## Import

```pleno
import { Base64 } from "std:base64";
```

## Functions

### `encode(data: String): String`

Encodes a string to Base64.

```pleno
let encoded = Base64.encode("Hello, World!");
// "SGVsbG8sIFdvcmxkIQ=="
```

### `decode(data: String): String`

Decodes a Base64 string back to the original string.

```pleno
let decoded = Base64.decode("SGVsbG8sIFdvcmxkIQ==");
// "Hello, World!"
```

Throws an error if the input is not valid Base64 or the result is not valid UTF-8.

---
