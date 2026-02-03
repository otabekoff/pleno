# std:crypto

Cryptographic functions for hashing, encryption, and JWT handling.

## Import

```pleno
import { Crypto } from "std:crypto";
```

## Hashing

### `hash(algorithm: String, data: String): String`

Computes a cryptographic hash of the data.

| Algorithm  | Output        |
| ---------- | ------------- |
| `"sha256"` | 64 hex chars  |
| `"sha512"` | 128 hex chars |

```pleno
let hash = Crypto.hash("sha256", "password123");
// "ef92b778bafe771e89245b89ecbc08a44a4e16...
```

### `hmac(algorithm: String, key: String, data: String): String`

Computes HMAC (Hash-based Message Authentication Code).

```pleno
let signature = Crypto.hmac("sha256", "secret-key", "message");
```

---

## Encryption

### `encrypt(algo: String, key: String, iv: String, data: String): String`

Encrypts data using the specified algorithm. Returns Base64-encoded ciphertext.

| Algorithm       | Key Length        | IV Length         |
| --------------- | ----------------- | ----------------- |
| `"aes-256-cbc"` | 32 bytes (64 hex) | 16 bytes (32 hex) |

```pleno
let key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
let iv = "0123456789abcdef0123456789abcdef";
let encrypted = Crypto.encrypt("aes-256-cbc", key, iv, "secret message");
```

### `decrypt(algo: String, key: String, iv: String, ciphertext: String): String`

Decrypts Base64-encoded ciphertext.

```pleno
let decrypted = Crypto.decrypt("aes-256-cbc", key, iv, encrypted);
// "secret message"
```

---

## JWT (JSON Web Tokens)

### `jwtSign(payload: Map, secret: String): String`

Creates a signed JWT using HS256 algorithm.

```pleno
let token = Crypto.jwtSign({
    "sub": "user123",
    "exp": 1735689600
}, "my-secret");
// "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
```

### `jwtVerify(token: String, secret: String): Map`

Verifies and decodes a JWT. Throws on invalid signature.

```pleno
let payload = Crypto.jwtVerify(token, "my-secret");
print(payload.sub);  // "user123"
```

---
