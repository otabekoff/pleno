# std:net

> **Stability:** stable in 1.0, covered by the [1.x compatibility promise](../specs/compatibility.md).

TCP sockets: listen for connections, connect to a server, and send and receive text.

## Import

```pleno
import { Net } from "std:net";
```

`listen` and `connect` can also be imported directly.

## How it works

Every operation that waits for the network returns a [promise](promise.md): `connect`, `accept`, `read`, `readLine` and `write`. Use `await` to wait for the result. While one task waits, other tasks keep running, so a server and a client can run in the same program.

```pleno
import { Net } from "std:net";

// Port 0 asks the system for a free port.
let listener = Net.listen("127.0.0.1", 0);

async fn serve() {
    let conn = await listener.accept();
    let line = await conn.readLine();
    await conn.write("echo: " + line + "\n");
    conn.close();
}

let server = serve();
let client = await Net.connect("127.0.0.1", listener.port);
await client.write("hello\n");
print(await client.readLine());   // echo: hello
client.close();
await server;
listener.close();
```

[examples/net_echo.pln](https://github.com/otabekoff/pleno/blob/main/examples/net_echo.pln) is a longer echo server and client.

Things to know:

- **Text only.** Data is sent and received as `String`, encoded as UTF-8. Pleno has no bytes type yet, so binary protocols are not supported. Bytes that are not valid UTF-8 are read as `�` (U+FFFD).
- **No timeout by default.** An operation waits until it can finish. Call `setTimeout(ms)` on a socket or listener, or pass `timeout:` to `connect`, to make it fail with a `TimeoutError` instead.
- **Closing.** Call `close()` when you are done. `close()` can be called more than once. A socket or listener that nothing refers to any more is closed by the garbage collector, and all of them are closed when the program ends, but do not rely on that: an open listener keeps its port taken until then.
- **Host names.** `connect` and `listen` look up a host name such as `"localhost"` before they start, and this lookup blocks the program for its duration. IPv4 addresses are tried first, so `"localhost"` means `127.0.0.1` for both.
- **Permission.** `pleno run --allow-network false` and a [`Sandbox`](../specs/security-features.md) with `allowNetwork: false` deny every call in this module with a `SecurityError`, including calls on a socket that was opened before.

## Functions

### `listen(host: String, port: Int): Listener`

Starts listening for TCP connections on the address. Port `0` picks a free port; `listener.port` tells which one. Throws an error if the address cannot be used, for example because the port is already taken.

```pleno
import { Net } from "std:net";

let listener = Net.listen("127.0.0.1", 0);
print(listener.port > 0);   // true
listener.close();
```

To accept connections from other machines, listen on `"0.0.0.0"` (all IPv4 addresses) instead of `"127.0.0.1"`.

### `connect(host: String, port: Int, timeout: Int = none): Promise<Socket>`

Opens a TCP connection. The promise resolves to a `Socket` once the connection is made. If the host has several addresses, they are tried in turn.

The promise is rejected with an `Error` when the connection fails:

| Situation | `e.name` | `e.message` |
| --- | --- | --- |
| Nothing listens on the port | `Error` | `connection refused: 127.0.0.1:9` |
| The host name is unknown | `Error` | `cannot resolve host "no-such-host.invalid": ...` |
| `timeout:` milliseconds passed | `TimeoutError` | `connection to 10.0.0.1:80 timed out after 2000 ms` |
| Network access is denied | `SecurityError` | `network access denied: connect to 127.0.0.1:9` |

```pleno
import { Net } from "std:net";

try {
    await Net.connect("127.0.0.1", 9, timeout: 5000);
} catch (e) {
    print(e.message);   // connection refused: 127.0.0.1:9
}
```

A denied connection and invalid arguments throw at once instead of rejecting the promise; `await` inside `try` catches both.

## Listener

The value `listen` returns.

| Member | Type | Description |
| --- | --- | --- |
| `address` | String | The address listened on, such as `"127.0.0.1:54321"` (an IPv6 address is in brackets: `"[::1]:54321"`) |
| `host` | String | The IP address part of `address` |
| `port` | Int | The port. With `listen(host, 0)`, the port the system picked |

### `accept(): Promise<Socket>`

Waits for the next incoming connection and resolves to its `Socket`. Several `accept()` calls can wait at once; connections go to them in the order they were made.

### `setTimeout(ms: Int?)`

Makes each later `accept()` fail with a `TimeoutError` (`accept() timed out after 500 ms`) if no connection arrives within `ms` milliseconds. `null` removes the limit.

### `close()`

Stops listening and frees the port. `accept()` calls still waiting fail with `the listener was closed while accept() was waiting`; a later `accept()` throws `cannot accept(): the listener is closed`. Connections accepted earlier stay open. Closing a closed listener does nothing.

## Socket

A connection, from `connect` or `accept`.

| Member | Type | Description |
| --- | --- | --- |
| `localAddress` | String | This end of the connection, such as `"127.0.0.1:54322"` |
| `remoteAddress` | String | The other end, such as `"127.0.0.1:54321"` |

`Socket` and `Listener` are objects with the members listed here. What `typeOf` reports for them is not part of the API.

### `read(maxBytes: Int = 65536): Promise<String?>`

Waits for data and resolves to what has arrived, at most `maxBytes` bytes. It does not wait for `maxBytes` bytes: one `write` on the other side can arrive in several reads, and several writes in one read. A character is never split between two reads. Resolves to `null` once the other side has closed the connection and all data has been read.

```pleno
import { Net } from "std:net";

let listener = Net.listen("127.0.0.1", 0);
let connecting = Net.connect("127.0.0.1", listener.port);
let server = await listener.accept();
let client = await connecting;

await server.write("abcdef");
server.close();
print(await client.read(4));   // abcd
print(await client.read());    // ef
print(await client.read());    // null
client.close();
listener.close();
```

### `readLine(): Promise<String?>`

Waits for a whole line and resolves to it without the line ending (`\n` or `\r\n`). If the other side closes in the middle of a line, that last part is returned as a line. Resolves to `null` once the other side has closed and all data has been read. A line longer than 1 MiB is an error.

```pleno norun
// Reads lines until the other side closes.
let mut line = await socket.readLine();
while (line != null) {
    print(line);
    line = await socket.readLine();
}
```

`read` and `readLine` share one buffer, so they can be mixed. Several reads can wait at once; they are served in the order they were made.

### `write(data: String): Promise<Null>`

Sends `data` as UTF-8. The promise resolves once all of it has been handed to the operating system, which then delivers it. Writes are sent in the order they were made, so it is safe to call `write` again before the previous one has finished. Await the last write before `close()`: data still waiting when the socket closes is not sent, and its promise is rejected.

### `setTimeout(ms: Int?)`

Makes each later `read`, `readLine` and `write` fail with a `TimeoutError` (`read() timed out after 500 ms`) if it cannot finish within `ms` milliseconds. `null` removes the limit. After a timeout the socket stays usable: data that arrives later is returned by the next read. After a `write` times out, part of its data may have been sent.

```pleno
import { Net } from "std:net";

let listener = Net.listen("127.0.0.1", 0);
let connecting = Net.connect("127.0.0.1", listener.port);
let server = await listener.accept();
let client = await connecting;

client.setTimeout(100);
try {
    await client.readLine();
} catch (e) {
    print(e.name);      // TimeoutError
    print(e.message);   // readLine() timed out after 100 ms
}
server.close();
client.close();
listener.close();
```

### `close()`

Closes the connection; the other side's reads then return `null`. Reads and writes still waiting fail. After `close()`, `read`, `readLine` and `write` throw an error such as `cannot read(): the socket is closed`. Closing a closed socket does nothing.

## Errors

Errors are `Error` values with a `name` and a `message`:

- `SecurityError`: network access is denied.
- `TimeoutError`: a timeout set with `setTimeout` or `timeout:` passed.
- `Error`: anything else, such as `connection refused: 127.0.0.1:9`, `connection reset by peer: 127.0.0.1:54321` or `cannot read(): the socket is closed`.

As for the whole standard library, the exact wording of messages is not covered by the compatibility promise; `name` is.

> For HTTP, use [`std:http`](http.md) instead.
