# std:net

Low-level TCP networking primitives.

## Import

```pleno
import { Net } from "std:net";
```

## Functions

### `listen(address: String): Map`

Creates a TCP listener bound to the specified address.

```pleno
let server = Net.listen("127.0.0.1:8080");
print("Listening on " + server.address);
```

**Returns:**

| Property  | Type   | Description       |
| --------- | ------ | ----------------- |
| `address` | String | The bound address |

### `connect(address: String): Map`

Creates a TCP connection to the specified address.

```pleno
let conn = Net.connect("example.com:80");
print("Connected to " + conn.remoteAddress);
```

**Returns:**

| Property        | Type   | Description        |
| --------------- | ------ | ------------------ |
| `remoteAddress` | String | The remote address |

> **Note**: These are low-level functions. For HTTP requests, use `std:http` instead.

---
