# http

## Contents

- [Http](#http)

## Http

```pleno
import Http from "std:http";
```

HTTP client module for making HTTP requests.

### Methods

#### `static` `fn get(url: string): string`
Make a GET request and return the response body.

```pleno
let data = Http.get("https://api.example.com/users");
```

#### `static` `fn post(url: string, body?: string): string`
Make a POST request with optional body.

```pleno
let result = Http.post("https://api.example.com/users", '{"name": "John"}');
```

#### `static` `fn put(url: string, body?: string): string`
Make a PUT request with optional body.

#### `static` `fn patch(url: string, body?: string): string`
Make a PATCH request with optional body.

#### `static` `fn delete(url: string): string`
Make a DELETE request.

#### `static` `fn head(url: string): object`
Make a HEAD request. Returns a map containing response headers and status.

```pleno
let headers = Http.head("https://example.com");
print(headers.status);  // 200
```

#### `static` `fn options(url: string): object`
Make an OPTIONS request. Returns a map containing response headers (including `Allow`) and status.

```pleno
let options = Http.options("https://api.example.com/users");
print(options["allow"]);  // "GET, POST, OPTIONS"
```

#### `static` `fn request(method: string, url: string, body?: string, headers?: object): object`
Generic request method supporting any HTTP method including WebDAV methods.

Returns a response object with:
- `status`: HTTP status code
- `ok`: boolean indicating success (2xx)
- `headers`: map of response headers
- `body`: response body as string

```pleno
// Standard methods
let response = Http.request("GET", "https://api.example.com/users");
print(response.status, response.ok);

// With custom headers
let response = Http.request("POST", "https://api.example.com/data", 
    '{"key": "value"}',
    { "Content-Type": "application/json", "Authorization": "Bearer token123" }
);

// WebDAV methods
let propfind = Http.request("PROPFIND", "https://dav.example.com/files/");
let mkcol = Http.request("MKCOL", "https://dav.example.com/newfolder/");
```

### Supported Methods

| Method | Function | Description |
|--------|----------|-------------|
| GET | `get()`, `request()` | Retrieve a resource |
| POST | `post()`, `request()` | Create a resource |
| PUT | `put()`, `request()` | Replace a resource |
| PATCH | `patch()`, `request()` | Partially update a resource |
| DELETE | `delete()`, `request()` | Delete a resource |
| HEAD | `head()`, `request()` | Get headers only |
| OPTIONS | `options()`, `request()` | Get allowed methods |
| TRACE | `request()` | Echo request back |
| CONNECT | `request()` | Establish tunnel |
| PROPFIND | `request()` | WebDAV: Get properties |
| PROPPATCH | `request()` | WebDAV: Set properties |
| MKCOL | `request()` | WebDAV: Create collection |
| COPY | `request()` | WebDAV: Copy resource |
| MOVE | `request()` | WebDAV: Move resource |
| LOCK | `request()` | WebDAV: Lock resource |
| UNLOCK | `request()` | WebDAV: Unlock resource |

---
