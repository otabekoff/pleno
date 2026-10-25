# std:crypto

> **Stability:** stable in 1.0.

Cryptographic functions for hashing, encryption, and JWT handling.

## Import

```pleno
import { Crypto } from "std:crypto";
```

The functions can also be imported one by one, e.g. `import { hash, hmac } from "std:crypto"`.

## Hashing

### `hash(algorithm: String, data: String): String`

Computes a cryptographic hash of the data and returns it as lowercase hex.

| Algorithm  | Output        |
| ---------- | ------------- |
| `"sha256"` | 64 hex chars  |
| `"sha512"` | 128 hex chars |

Any other algorithm name throws an error (`Unsupported hash algorithm: md5`).

```pleno
import { Crypto } from "std:crypto";

let hash = Crypto.hash("sha256", "hello");
print(hash);  // 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
```

### `hmac(algorithm: String, key: String, data: String): String`

Computes an HMAC (Hash-based Message Authentication Code) and returns it as hex. Only `"sha256"` is supported.

```pleno
import { Crypto } from "std:crypto";

let signature = Crypto.hmac("sha256", "secret-key", "message");
print(signature.length);  // 64
```

---

## Encryption

### `encrypt(algo: String, key: String, iv: String, data: String): String`

Encrypts data using the specified algorithm. The key and IV are hex strings. Returns Base64-encoded ciphertext.

| Algorithm       | Key Length        | IV Length         |
| --------------- | ----------------- | ----------------- |
| `"aes-256-cbc"` | 32 bytes (64 hex) | 16 bytes (32 hex) |

```pleno
import { Crypto } from "std:crypto";

let key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
let iv = "0123456789abcdef0123456789abcdef";
let encrypted = Crypto.encrypt("aes-256-cbc", key, iv, "secret message");
print(encrypted);  // IFL3l4O5UHCPSKn2OAPp2g==
```

Use a random key and a new random IV for real data. The fixed values here are only for the example.

### `decrypt(algo: String, key: String, iv: String, ciphertext: String): String`

Decrypts Base64-encoded ciphertext. Throws if the key, IV, or data is wrong.

```pleno
import { Crypto } from "std:crypto";

let key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
let iv = "0123456789abcdef0123456789abcdef";
let encrypted = Crypto.encrypt("aes-256-cbc", key, iv, "secret message");
let decrypted = Crypto.decrypt("aes-256-cbc", key, iv, encrypted);
print(decrypted);  // secret message
```

---

## JWT (JSON Web Tokens)

Tokens are signed and verified with HS256 (HMAC-SHA256) only.

### `jwtSign(payload: Map, secret: String): String`

Creates a JWT signed with HS256. The payload is written as JSON, like `JSON.stringify`. Claims such as `exp` (expires at) and `nbf` (not before) are numbers of seconds since the Unix epoch, for example `Time.now() + 3600` (from `std:time`) for one hour from now.

```pleno
import { Crypto } from "std:crypto";

let token = Crypto.jwtSign({
    "sub": "user123",
    "exp": 4102444800
}, "my-secret");
print(token);  // eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
```

### `jwtVerify(token: String, secret: String): Map`

Verifies a JWT and returns its payload. It throws an error when:

| Problem                                                   | Error message                                |
| --------------------------------------------------------- | -------------------------------------------- |
| Not three dot-separated parts, or a part is not Base64url | `Invalid JWT format`, `Invalid ... encoding` |
| The signature does not match the secret                   | `Invalid JWT signature`                      |
| The header's `alg` is not `HS256`                         | `Unsupported JWT algorithm: none`            |
| `exp` is present and the current time is at or after it   | `JWT expired`                                |
| `nbf` is present and the current time is before it        | `JWT not valid yet`                          |
| `exp` or `nbf` is present but not a number                | `Invalid JWT exp claim: expected a number`   |

The signature is checked first, in constant time, so a forged token fails with `Invalid JWT signature` whatever its claims say. `exp` and `nbf` are compared with the system clock, without leeway. Other claims, such as `iss`, `aud` and `iat`, are returned in the payload but not checked; check them yourself.

```pleno
import { Crypto } from "std:crypto";

let token = Crypto.jwtSign({ "sub": "user123" }, "my-secret");
let payload = Crypto.jwtVerify(token, "my-secret");
print(payload.sub);  // user123

try {
    Crypto.jwtVerify(token, "wrong-secret");
} catch (e) {
    print(e.message);  // Invalid JWT signature
}

let expired = Crypto.jwtSign({ "sub": "user123", "exp": 1 }, "my-secret");
try {
    Crypto.jwtVerify(expired, "my-secret");
} catch (e) {
    print(e.message);  // JWT expired
}
```
