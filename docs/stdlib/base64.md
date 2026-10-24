# std:base64

Base64 encoding and decoding (standard alphabet, with padding).

## Import

```pleno
import { Base64 } from "std:base64";
```

`encode` and `decode` can also be imported directly.

## Functions

### `encode(data: String): String`

Encodes the UTF-8 bytes of a string to Base64.

```pleno
import { Base64 } from "std:base64";

print(Base64.encode("Hello, World!")); // SGVsbG8sIFdvcmxkIQ==
```

### `decode(data: String): String`

Decodes a Base64 string back to the original string.

```pleno
import { Base64 } from "std:base64";

print(Base64.decode("SGVsbG8sIFdvcmxkIQ==")); // Hello, World!
```

Throws an error if the input is not valid Base64 or the result is not valid UTF-8.

---
