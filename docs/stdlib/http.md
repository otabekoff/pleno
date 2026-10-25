# std:http

> **Stability:** stable in 1.0. The client functions are stable. `Router` and `Server` are experimental: they are not covered by the [1.x compatibility promise](../specs/compatibility.md).

## Contents

- [HTTP](#http)
- [Server](#server)

## HTTP

```pleno
import { HTTP } from "std:http";
```

HTTP client for making requests. `Http` is the same namespace, and each function can also be imported directly, e.g. `import { get } from "std:http";`.

Requests are blocking: each call returns once the response has arrived. A request that cannot be sent (bad URL, connection refused, DNS failure) throws an error. HTTP error statuses such as 404 do not throw.

```pleno
import { HTTP } from "std:http";

try {
    HTTP.get("http://127.0.0.1:1/");
} catch (e) {
    print(e.message); // Request failed: error sending request for url (http://127.0.0.1:1/)
}
```

### Methods

#### `static` `fn get(url: String): String`

Makes a GET request and returns the response body.

```pleno norun
import { HTTP } from "std:http";
import { JSON } from "std:json";

let body = HTTP.get("https://api.example.com/users");
let users = JSON.parse(body);
```

#### `static` `fn post(url: String, body?: String): String`

Makes a POST request with an optional body and returns the response body.

```pleno norun
import { HTTP } from "std:http";

let result = HTTP.post("https://api.example.com/users", `{"name": "John"}`);
```

#### `static` `fn put(url: String, body?: String): String`

Makes a PUT request with an optional body and returns the response body.

#### `static` `fn patch(url: String, body?: String): String`

Makes a PATCH request with an optional body and returns the response body.

#### `static` `fn delete(url: String): String`

Makes a DELETE request and returns the response body.

#### `static` `fn head(url: String): Map`

Makes a HEAD request. Returns a map of the response headers (lowercase names) plus a `status` entry.

#### `static` `fn options(url: String): Map`

Makes an OPTIONS request. Returns the same kind of map as `head`, including the `allow` header if the server sends one.

```pleno norun
import { HTTP } from "std:http";

let info = HTTP.head("https://example.com");
print(info.status);            // 200
print(info["content-type"]);   // text/html

let options = HTTP.options("https://api.example.com/users");
print(options["allow"]);       // e.g. GET, POST, OPTIONS
```

#### `static` `fn request(method: String, url: String, body?: String, headers?: Map): Map`

Generic request for any HTTP method, including WebDAV methods. The method name is case-insensitive. Use it when you need custom headers or the response status.

Returns a map with:

- `status`: HTTP status code
- `ok`: `true` for a 2xx status
- `headers`: map of response headers (lowercase names)
- `body`: response body as a string

```pleno norun
import { HTTP } from "std:http";

let response = HTTP.request("GET", "https://api.example.com/users");
print(response.status, response.ok);

// With a body and custom headers
let created = HTTP.request("POST", "https://api.example.com/data",
    `{"key": "value"}`,
    { "Content-Type": "application/json", "Authorization": "Bearer token123" }
);

// WebDAV methods
let propfind = HTTP.request("PROPFIND", "https://dav.example.com/files/");
let mkcol = HTTP.request("MKCOL", "https://dav.example.com/newfolder/");
```

### Supported Methods

| Method    | Function                 | Description                 |
| --------- | ------------------------ | --------------------------- |
| GET       | `get()`, `request()`     | Retrieve a resource         |
| POST      | `post()`, `request()`    | Create a resource           |
| PUT       | `put()`, `request()`     | Replace a resource          |
| PATCH     | `patch()`, `request()`   | Partially update a resource |
| DELETE    | `delete()`, `request()`  | Delete a resource           |
| HEAD      | `head()`, `request()`    | Get headers only            |
| OPTIONS   | `options()`, `request()` | Get allowed methods         |
| TRACE     | `request()`              | Echo request back           |
| CONNECT   | `request()`              | Establish tunnel            |
| PROPFIND  | `request()`              | WebDAV: Get properties      |
| PROPPATCH | `request()`              | WebDAV: Set properties      |
| MKCOL     | `request()`              | WebDAV: Create collection   |
| COPY      | `request()`              | WebDAV: Copy resource       |
| MOVE      | `request()`              | WebDAV: Move resource       |
| LOCK      | `request()`              | WebDAV: Lock resource       |
| UNLOCK    | `request()`              | WebDAV: Unlock resource     |

---

## Server

```pleno
import { Router, Server } from "std:http";
```

::: warning Experimental
`Router`, `Server` and `listen` are not covered by the [1.x compatibility promise](../specs/compatibility.md). They work and are tested by serving requests, but middleware, static files and cookies are still to come, which may change the shape of `req` and `res`.
:::

A small HTTP/1.1 server. Build a `Router`, pass it to `Server`, then call `listen`.

### `Router(): Router`

Creates an empty router. Register handlers with `get`, `post`, `put`, `delete` and `patch`, each taking a path pattern and a handler. They return the router, so calls can be chained. A segment written `:name` matches any value and is available as `req.params.name`.

```pleno
import { Router } from "std:http";

let router = Router();

router
    .get("/", fn(req, res) => "Hello, Pleno!")
    .get("/users/:id", fn(req, res) {
        res.json({ "id": req.params.id });
    })
    .post("/users", fn(req, res) {
        res.status(201).send(req.body);
    });
```

The handler gets two arguments:

- `req`: a map with `method`, `path`, `params`, `query`, `headers` (lowercase names) and `body` (String).
- `res`: the response. `res.status(code)` and `res.header(name, value)` set the status and a header and return `res`. `res.send(body)` sets the body. `res.json(value)` sets the body to `value` as JSON and the `Content-Type` to `application/json`.

A handler can also return a string instead of calling `res.send`. An `async fn` handler can `await`; the response is sent when it finishes. If a handler throws, the client gets a 500 response. A request that matches no route gets a 404.

### `Server(router: Router): Server`

Creates a server for a router.

### `listen(port: Int, callback?: Function): Null`

Starts listening on `0.0.0.0:port`, prints `Server listening on http://0.0.0.0:<port>`, and calls `callback` once the port is bound. The program keeps running and serves requests until it is stopped. Only the methods registered on the router are served, with one addition: a `HEAD` request is answered by the matching `get` route, with the status and headers (`Content-Length` included) of the `GET` response and no body. The handler sees `req.method` as `"HEAD"`.

```pleno norun
import { Router, Server } from "std:http";

let router = Router();
router.get("/", fn(req, res) => "Hello, Pleno!");

let server = Server(router);
server.listen(8080, fn() {
    print("Server running on port 8080");
});
```

---
