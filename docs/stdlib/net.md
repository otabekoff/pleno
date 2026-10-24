# std:net

Low-level TCP networking primitives.

## Import

```pleno
import { Net } from "std:net";
```

`listen` and `connect` can also be imported directly.

::: warning Limited
`listen` binds the address and `connect` opens a connection, but both close the socket right away: there is no `accept`, `read` or `write` yet. Use them to check that an address can be bound or reached.
:::

## Functions

### `listen(address: String): Map`

Binds a TCP listener to the address. Throws an error if the address cannot be bound, for example because the port is in use.

```pleno
import { Net } from "std:net";

let server = Net.listen("127.0.0.1:0");  // port 0: any free port
print("Bound " + server.address);        // Bound 127.0.0.1:0
```

**Returns:**

| Property  | Type   | Description                        |
| --------- | ------ | ---------------------------------- |
| `address` | String | The address string that was passed |

### `connect(address: String): Map`

Opens a TCP connection to the address. Throws an error if the connection fails.

```pleno norun
import { Net } from "std:net";

let conn = Net.connect("example.com:80");
print("Connected to " + conn.remoteAddress);
```

```pleno
import { Net } from "std:net";

try {
    Net.connect("127.0.0.1:1");
} catch (e) {
    print("Cannot connect: " + e.message);
}
```

**Returns:**

| Property        | Type   | Description                        |
| --------------- | ------ | ---------------------------------- |
| `remoteAddress` | String | The address string that was passed |

> **Note**: For HTTP requests, use `std:http` instead.

---
