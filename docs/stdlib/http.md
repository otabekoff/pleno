# std:http

> **Stability:** stable in 1.0: the client functions, and the server (`Router`, `Server`, middleware, static files and cookies).

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

A small HTTP/1.1 server. Build a `Router`, add routes and middleware to it, pass it to `Server`, then call `listen`.

```pleno norun
import { Router, Server } from "std:http";

let app = Router();

app.use(fn(req, res, next) {
    print(req.method + " " + req.path);
    next();
});
app.static("/assets", "./public");
app.get("/", fn(req, res) => "Hello, Pleno!");
app.get("/users/:id", fn(req, res) {
    res.json({ "id": req.params.id });
});

Server(app).listen(8080);
```

`examples/web_app/app.pln` is a complete app with middleware, static files, cookies and an error handler.

### `Router(): Router`

Creates an empty router. Every method below returns the router, so calls can be chained.

#### Routes: `get`, `post`, `put`, `delete`, `patch`

`router.get(path, handler)` and the others register a handler for a method and a path. The path must start with `/`. A segment written `:name` matches any non-empty value, which the handler reads as `req.params.name`. Registering the same method and path again replaces the first handler.

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

A handler gets `req` and `res` (described below). It can:

- answer with `res.send`, `res.json` or `res.redirect`;
- return a string, which becomes the body;
- be an `async fn` and `await`: the response is sent when it finishes;
- throw: the client gets a 500 response (see [Errors](#errors)).

If a handler returns without sending anything, the response has the status and headers it set and an empty body, so `res.status(204)` alone is a complete answer.

A request that matches no route gets `404 Not Found`. If the path has routes for other methods only, it gets `405 Method Not Allowed` with an `Allow` header, and an `OPTIONS` request gets `204` with the same `Allow` header. A `HEAD` request is answered by the `get` route, with the status and headers of the `GET` response (`Content-Length` included) and no body; the handler sees `req.method` as `"HEAD"`.

#### `use(middleware)` and `use(prefix, middleware)`

Adds a middleware: a function `fn(req, res, next)` that runs before the route. It can read and change `req` and `res`, answer the request itself, or call `next()` to pass the request on.

```pleno
import { Router } from "std:http";

let router = Router();

// Every request.
router.use(fn(req, res, next) {
    res.header("X-Powered-By", "Pleno");
    next();
});

// Only /admin and the paths below it, such as /admin/users (not /administrator).
router.use("/admin", fn(req, res, next) {
    if (req.cookies.role != "admin") {
        res.status(403).send("admins only");
        return;
    }
    next();
});
```

The rules:

- **Order.** Middleware runs in the order it was registered with `use` and `static`, and always before the route, whether it was registered before or after the routes. A request that matches no route still goes through the middleware.
- **Prefix.** `use("/api", ...)` runs for `/api` and every path below it. `req.path` is still the full path.
- **Ending the chain.** A middleware that sends a response (`res.send`, `res.json`, `res.redirect`) and does not call `next()` ends the chain: later middleware and the route do not run.
- **Forgetting `next()`.** A middleware that returns (or whose promise settles) without calling `next()` and without sending a response is a bug in the program. The server answers `500 Internal Server Error` and prints `GET /path: middleware 2 (use "/admin") returned without calling next() or sending a response` to stderr, where the number is the middleware's position among the `use` and `static` calls. The client is not left waiting.
- **Calling `next()` twice** throws an error.
- **`next(err)`** fails the request with `err`, as if the middleware had thrown it.

`next()` returns what the rest of the chain returned: a promise when it is async. An `async fn` middleware can `await next()` and then do more work, such as adding a header to the response the route built. The response is written only after every middleware and the route have finished, including the promises they returned.

```pleno
import { Router } from "std:http";
import { now } from "std:time";

let router = Router();

router.use(async fn(req, res, next) {
    let started = now();
    await next();
    res.header("X-Seconds", `${now() - started}`);
});
```

An error thrown by the rest of the chain comes out of `next()`, so a middleware can catch it:

```pleno
import { Router } from "std:http";

let router = Router();

router.use(fn(req, res, next) {
    try {
        next();
    } catch (e) {
        res.status(503).send("try again later");
    }
});
```

In an `async fn` middleware, `await next()` throws the error in the same way. If the error is not caught, the middleware fails too, so code after `await next()` does not run for a failed request.

`next()` must be called before the middleware returns, or before its promise settles. Calling it later, for example from a timer, throws, because the response has been written by then.

#### `static(prefix, folder)`

Serves the files of `folder` under the URL `prefix`. `folder` is relative to the working directory. It must exist when `static` is called, and the sandbox must allow reading it: `static` throws `SecurityError` otherwise.

```pleno norun
import { Router } from "std:http";

let router = Router();
router.static("/assets", "./public");   // ./public/css/site.css is /assets/css/site.css
router.static("/", "./site");           // ./site/index.html is /
```

`static` is a middleware, so it runs in registration order with `use`. For `GET` and `HEAD` requests:

- **A file** is sent with a `Content-Type` from its extension (`.html`, `.css`, `.js`, `.json`, `.svg`, `.png`, `.jpg`, `.woff2`, `.wasm` and other common types; `application/octet-stream` for the rest) and `X-Content-Type-Options: nosniff`.
- **A folder** is answered with its `index.html`. A folder requested without a trailing slash (`/docs`) is redirected (301) to `/docs/`, so that relative links in the page work.
- **Caching.** Responses have an `ETag`, a `Last-Modified` date and `Cache-Control: public, max-age=0`. A request with a matching `If-None-Match`, or with an `If-Modified-Since` date not older than the file, gets `304 Not Modified` without a body.
- **Not found.** When there is no such file, when the path is a folder without `index.html`, or for other methods, the request goes on to the next middleware or route. Names that start with `.` (`.env`, `.git`) are never served and also go on.

Paths cannot leave the folder. The request path is decoded first, so `%2e%2e` is `..`. Then:

- a path with a `..` or `.` segment, a backslash, a `:` (drive letters, Windows streams), a NUL or another control character is refused with `403 Forbidden`;
- the file is normalized the way the sandbox normalizes paths: `.` and `..` removed and symbolic links resolved. If the result is not inside the folder, for example because of a link that points elsewhere, the answer is `403 Forbidden`;
- the file is checked against the sandbox's read rule (`--allow-read`) before it is read.

Files are read into memory whole, so `static` suits a site's assets rather than large downloads.

#### `onError(handler)`

Sets the error handler: `fn(err, req, res)`, called when a middleware or route throws, returns a promise that is rejected, or calls `next(err)`. `err` is the error: an `Error` with `name` and `message` for a thrown error, or the value given to `next(err)`.

```pleno norun
import { Router } from "std:http";

let router = Router();

router.onError(fn(err, req, res) {
    print("error in " + req.path + ": " + err.message);
    res.status(500).json({ "error": "something went wrong" });
});
```

The handler starts from a fresh response: status 500, and none of the headers or cookies set before the error. It can be async. If it sends nothing, or throws, the client gets the default 500 response.

### Errors

Without `onError`, an error in a middleware or route is printed to stderr as `GET /path: Error: message`, and the client gets `500 Internal Server Error` with the body `Internal Server Error`. The error message is never sent to the client, as it can contain file paths, queries or secrets.

The server answers some requests itself, before any middleware runs:

| Request                                                                                                    | Status |
| ---------------------------------------------------------------------------------------------------------- | ------ |
| Not HTTP/1.x, a target that is not a path, a malformed header line, both `Content-Length` and `Transfer-Encoding` | 400    |
| A body over 10 MB                                                                                          | 413    |
| A request line and headers over 64 KB                                                                      | 431    |
| A `Transfer-Encoding` other than `chunked`                                                                 | 501    |

### `req`

A map with:

- `method`: `"GET"`, `"POST"`, ... in capitals.
- `path`: the path as the client sent it, without the query string and not decoded.
- `params`: the `:name` segments of the route, decoded (`/users/Ann%20Lee` gives `"Ann Lee"`).
- `query`: the query string as a map, with `+` and `%XX` decoded. A key given twice keeps its last value. `req.query.page ?? "1"` reads an optional parameter.
- `headers`: the request headers, with lowercase names. A header sent twice has its values joined with `, `.
- `cookies`: the cookies of the `Cookie` header, by name, with `%XX` decoded. A missing cookie reads as `null`.
- `body`: the body as a String (empty when there is none). `JSON.parse(req.body)` reads a JSON body. Chunked bodies are supported.
- `ip`: the client's IP address.

### `res`

The response. The methods that set something return `res`, so they can be chained.

- `res.status(code)`: sets the status, from 200 to 599.
- `res.header(name, value)`: sets a header, replacing one with the same name in any case. It throws for a name that is not a valid header name, for a name or value that contains CR, LF or NUL (so a value taken from the request cannot add headers of its own), for `Content-Length`, `Connection`, `Transfer-Encoding` and `Date`, which the server sets, and for `Set-Cookie` (use `res.cookie`).
- `res.send(body)`: sets the body: a String, or a number or Bool written as text. A map or array throws; use `res.json`. Without a `Content-Type`, the body is sent as `text/plain; charset=utf-8`.
- `res.json(value)`: sets the body to `value` as JSON and the `Content-Type` to `application/json`.
- `res.redirect(url, status?)`: sets the `Location` header and the status: 302, or the 3xx status given.
- `res.cookie(name, value, options?)`: sets a cookie (see [Cookies](#cookies)).
- `res.clearCookie(name, options?)`: removes a cookie.
- `res.statusCode`, `res.headers`, `res.body`: what has been set so far.
- `res.sent`: `true` once a response was sent with `send`, `json` or `redirect`.

A request has one response: calling `send`, `json` or `redirect` a second time throws. The status and headers can still change after `send`, until the response is written when the whole chain has finished.

### Cookies

`req.cookies` holds the cookies the client sent. `res.cookie` sets one:

```pleno norun
import { Router } from "std:http";

let router = Router();

router.post("/login", fn(req, res) {
    res.cookie("session", "f81d4fae", { "maxAge": 3600 }).send("welcome");
});
router.get("/me", fn(req, res) => req.cookies.session ?? "not logged in");
router.post("/logout", fn(req, res) {
    res.clearCookie("session").send("bye");
});
```

The options:

| Option     | Default                 | Meaning                                                                                                                       |
| ---------- | ----------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `maxAge`   | none (a session cookie) | Lifetime in **seconds**, as in the `Max-Age` attribute.                                                                       |
| `path`     | `"/"`                   | The paths the cookie is sent for.                                                                                             |
| `domain`   | none (this host only)   | The domain the cookie is sent to.                                                                                             |
| `httpOnly` | `true`                  | Hides the cookie from JavaScript in the page. Set `false` only if scripts must read it.                                       |
| `secure`   | `false`                 | Sends the cookie over HTTPS only. Set `true` when the site is served over HTTPS.                                              |
| `sameSite` | `"Lax"`                 | `"Strict"`, `"Lax"`, `"None"`, or `false` to leave the attribute out. `"None"` needs `secure: true`; browsers reject it otherwise. |

`httpOnly` and `sameSite: "Lax"` are on by default because they protect against stolen cookies and cross-site requests. Turn them off only when you need to.

The name must be a valid token: letters, digits and ``!#$%&'*+-.^_`|~``. The value can be any String: the characters a cookie value cannot hold (spaces, `;`, `,`, `"`, `\`, `%` and non-ASCII characters) are percent-encoded, and `req.cookies` decodes them again. `res.cookie` throws for a name or value with CR, LF or NUL, for a `path` or `domain` with spaces, `;`, `,` or control characters, and for an unknown option, so that a typo such as `maxage` does not go unnoticed.

Setting a cookie again with the same name, path and domain replaces it in the response. `res.clearCookie(name, options?)` sends the cookie with an empty value that has already expired; pass the same `path` and `domain` it was set with.

Cookies are not signed: a client can send any value. Store a random session id in the cookie and keep the session's data on the server, rather than storing the data itself in the cookie.

### `Server(router: Router): Server`

Creates a server for a router.

### `listen(port: Int, callback?: Function): Null`

Starts listening on `0.0.0.0:port`, prints `Server listening on http://0.0.0.0:<port>`, and calls `callback` once the port is bound. Port `0` picks a free port; `server.port` holds the port in use. The program keeps running and serves requests until it is stopped. `listen` throws `SecurityError` when the sandbox denies network access.

```pleno norun
import { Router, Server } from "std:http";

let router = Router();
router.get("/", fn(req, res) => "Hello, Pleno!");

let server = Server(router);
server.listen(8080, fn() {
    print(`Server running on port ${server.port}`);
});
```

### Limits

- Each connection serves one request (`Connection: close`).
- Requests are read one at a time on the program's single thread, with a 5 second timeout: a client that sends its request slowly delays the others until it finishes or times out. When the server faces the internet, put it behind a reverse proxy such as nginx or Caddy.
- Request bodies are limited to 10 MB, and static files are read into memory whole.
- There is no HTTPS; a reverse proxy can provide it.

---
