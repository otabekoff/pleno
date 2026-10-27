//! `std:http`'s server, tested by serving real requests on a free local port:
//! routes, `req` and `res`, middleware, static files and cookies.

use pleno_compiler::Compiler;
use pleno_lexer::lex;
use pleno_parser::Parser;
use pleno_value::{SandboxConfig, Value};
use pleno_vm::VM;
use std::path::{Path, PathBuf};

fn compile(source: &str) -> pleno_bytecode::Chunk {
    let tokens = lex(source).expect("Lexing failed");
    let mut parser = Parser::new(tokens, source);
    let mut program = parser.parse_program().expect("Parsing failed");
    let mut compiler = Compiler::new(source.to_string());
    compiler.compile(&mut program).expect("Compilation failed")
}

fn run_source(source: &str) -> Result<Value, String> {
    let _ = env_logger::builder().is_test(true).try_init();
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(compile(source)).map_err(|e| e.message)
}

#[test]
fn test_http_router_registration() {
    let source = r#"
        import { Router } from "std:http";
        let r = Router();
        r.get("/", fn(req, res) { return "Home"; });
        r.post("/api", fn(req, res) { return "API"; });
        r.use(fn(req, res, next) { next(); });
        r.use("/api", fn(req, res, next) { next(); });
        r.onError(fn(err, req, res) { res.send("x"); });
        true
    "#;
    assert!(matches!(run_source(source), Ok(Value::Bool(true))));
}

#[test]
fn router_rejects_bad_registrations() {
    for (code, message) in [
        (
            r#"r.get("users", fn(req, res) => "x");"#,
            "must start with '/'",
        ),
        (r#"r.get("/", "not a function");"#, "must be a function"),
        (
            r#"r.use("api", fn(req, res, next) { next(); });"#,
            "must start with '/'",
        ),
        (r#"r.use(42);"#, "must be a function"),
        (
            r#"r.static("/", "./definitely-not-a-folder");"#,
            "is not a folder",
        ),
        (r#"r.onError(1);"#, "onError() expects a function"),
        (r#"Server(1);"#, "expects a Router"),
        (r#"Server(r).listen(70000);"#, "port from 0 to 65535"),
        (r#"Server(r).listen(0, host: 5);"#, "host must be a String"),
        (
            r#"Server(r).listen(0, host: "not a host!");"#,
            "is not an address or host name",
        ),
    ] {
        let source =
            format!("import {{ Router, Server }} from \"std:http\";\nlet r = Router();\n{code}");
        let error = run_source(&source).expect_err(code);
        assert!(error.contains(message), "{code}: {error}");
    }
}

/// Starts a server on a free local port, in a thread of its own, and returns
/// the port once it accepts connections. `setup` is Pleno code that registers
/// handlers on `router`. The thread runs until the test exits.
fn serve_with(setup: String, config: Option<SandboxConfig>) -> u16 {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("free port")
        .port();
    std::thread::spawn(move || {
        let source = format!(
            r#"
            import {{ Router, Server }} from "std:http";
            import {{ JSON }} from "std:json";
            import {{ sleep }} from "std:time";
            import {{ collect }} from "std:gc";
            let router = Router();
            {setup}
            Server(router).listen({port});
            "#
        );
        let mut vm = VM::new();
        pleno_stdlib::register(&mut vm);
        if let Some(config) = config {
            vm.apply_config(config);
        }
        vm.execute(compile(&source)).expect("server failed");
    });
    for _ in 0..200 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return port;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("server did not start on port {port}");
}

fn serve(setup: &str) -> u16 {
    serve_with(setup.to_string(), None)
}

/// Sends `request` as is and returns the whole response.
fn send(port: u16, request: &str) -> String {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    String::from_utf8_lossy(&response).into_owned()
}

/// A request with no body.
fn request(port: u16, method: &str, target: &str, headers: &str) -> String {
    send(
        port,
        &format!("{method} {target} HTTP/1.1\r\nHost: x\r\n{headers}\r\n"),
    )
}

fn post(port: u16, target: &str, body: &str) -> String {
    send(
        port,
        &format!(
            "POST {target} HTTP/1.1
Host: x
Content-Length: {}

{body}",
            body.len()
        ),
    )
}

fn get(port: u16, target: &str) -> String {
    request(port, "GET", target, "")
}

fn status(response: &str) -> &str {
    response.get(9..12).unwrap_or("")
}

fn body(response: &str) -> &str {
    response.split_once("\r\n\r\n").map_or("", |(_, b)| b)
}

/// The value of the first `name` header, by a case-sensitive name match.
fn header<'a>(response: &'a str, name: &str) -> Option<&'a str> {
    let head = response.split_once("\r\n\r\n").map_or(response, |(h, _)| h);
    head.lines()
        .find_map(|line| line.strip_prefix(&format!("{name}: ")))
}

fn headers<'a>(response: &'a str, name: &str) -> Vec<&'a str> {
    let head = response.split_once("\r\n\r\n").map_or(response, |(h, _)| h);
    head.lines()
        .filter_map(|line| line.strip_prefix(&format!("{name}: ")))
        .collect()
}

#[test]
fn test_http_server_serves_get_head_and_404() {
    let port = serve(
        r#"
        router.get("/hello/:name", fn(req, res) {
            res.header("X-Method", req.method).send("Hello, " + req.params.name);
        });
        router.post("/echo", fn(req, res) => req.body);
        router.get("/later", async fn(req, res) { return "done"; });
        router.get("/fail", fn(req, res) { throw Error("boom: secret detail"); });
        "#,
    );

    let get_response = get(port, "/hello/ann");
    assert!(
        get_response.starts_with("HTTP/1.1 200 OK\r\n"),
        "{get_response}"
    );
    assert_eq!(header(&get_response, "Content-Length"), Some("10"));
    assert_eq!(header(&get_response, "X-Method"), Some("GET"));
    assert!(header(&get_response, "Date").is_some(), "{get_response}");
    assert_eq!(body(&get_response), "Hello, ann");

    // HEAD on a `get` route: the same status and headers, no body.
    let head = request(port, "HEAD", "/hello/ann", "");
    assert!(head.starts_with("HTTP/1.1 200 OK\r\n"), "{head}");
    assert_eq!(header(&head, "Content-Length"), Some("10"));
    assert_eq!(header(&head, "X-Method"), Some("HEAD"));
    assert!(head.ends_with("\r\n\r\n"), "{head}");

    let post = send(
        port,
        "POST /echo HTTP/1.1\r\nHost: x\r\nContent-Length: 4\r\n\r\nping",
    );
    assert!(post.starts_with("HTTP/1.1 200 OK\r\n"), "{post}");
    assert_eq!(body(&post), "ping");

    // An async handler answers once its promise settles; HEAD drops the body.
    assert_eq!(body(&get(port, "/later")), "done");
    let later_head = request(port, "HEAD", "/later", "");
    assert_eq!(header(&later_head, "Content-Length"), Some("4"));
    assert!(later_head.ends_with("\r\n\r\n"), "{later_head}");

    // A failing handler is a 500 that does not show the error to the client.
    let fail = get(port, "/fail");
    assert!(
        fail.starts_with("HTTP/1.1 500 Internal Server Error\r\n"),
        "{fail}"
    );
    assert_eq!(body(&fail), "Internal Server Error");
    assert!(!fail.contains("boom"), "{fail}");
    let fail_head = request(port, "HEAD", "/fail", "");
    assert_eq!(status(&fail_head), "500");
    assert!(fail_head.ends_with("\r\n\r\n"), "{fail_head}");

    let missing = get(port, "/nope");
    assert!(
        missing.starts_with("HTTP/1.1 404 Not Found\r\n"),
        "{missing}"
    );
    let missing_head = request(port, "HEAD", "/nope", "");
    assert_eq!(status(&missing_head), "404");
    assert!(missing_head.ends_with("\r\n\r\n"), "{missing_head}");

    // A path with routes for other methods: 405 with `Allow`; OPTIONS lists them.
    let head_post = request(port, "HEAD", "/echo", "");
    assert_eq!(status(&head_post), "405", "{head_post}");
    let put = request(port, "PUT", "/echo", "");
    assert_eq!(status(&put), "405", "{put}");
    assert_eq!(header(&put, "Allow"), Some("POST, OPTIONS"));
    let options = request(port, "OPTIONS", "/hello/bob", "");
    assert_eq!(status(&options), "204", "{options}");
    assert_eq!(header(&options, "Allow"), Some("GET, HEAD, OPTIONS"));
    assert_eq!(header(&options, "Content-Length"), None);
}

#[test]
fn requests_are_parsed_and_checked() {
    let port = serve(
        r#"
        router.get("/query", fn(req, res) => req.query.q + "|" + req.query.empty + "|" + (req.query.missing ?? "none"));
        router.get("/files/:name", fn(req, res) => req.params.name);
        router.get("/headers", fn(req, res) => req.headers["x-thing"]);
        router.post("/json", fn(req, res) {
            let data = JSON.parse(req.body);
            res.status(201).json({ "got": data.name, "n": data.n + 1 });
        });
        router.post("/length", fn(req, res) => req.body.length.toString());
        router.delete("/items/:id", fn(req, res) { res.status(204); });
        router.get("/teapot", fn(req, res) { res.status(418).send("short and stout"); });
        "#,
    );

    // `+` and %XX are decoded in query strings and route parameters.
    assert_eq!(body(&get(port, "/query?q=a+b%26c&empty=")), "a b&c||none");
    assert_eq!(body(&get(port, "/files/my%20file.txt")), "my file.txt");
    // Repeated headers are joined.
    assert_eq!(
        body(&request(
            port,
            "GET",
            "/headers",
            "X-Thing: a\r\nX-Thing: b\r\n"
        )),
        "a, b"
    );

    let json = post(port, "/json", r#"{"name":"Ada","n":41}"#);
    assert_eq!(status(&json), "201", "{json}");
    assert_eq!(header(&json, "Content-Type"), Some("application/json"));
    assert_eq!(body(&json), r#"{"got":"Ada","n":42}"#);
    // Invalid JSON: the handler throws, the client gets a plain 500.
    let bad_json = post(port, "/json", "{bad");
    assert_eq!(status(&bad_json), "500");
    assert!(!bad_json.contains("parse"), "{bad_json}");

    // A chunked body.
    let chunked = send(
        port,
        "POST /length HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n",
    );
    assert_eq!(body(&chunked), "11", "{chunked}");

    // 204 has no body and no Content-Length.
    let deleted = request(port, "DELETE", "/items/3", "");
    assert!(
        deleted.starts_with("HTTP/1.1 204 No Content\r\n"),
        "{deleted}"
    );
    assert_eq!(header(&deleted, "Content-Length"), None);
    assert!(deleted.ends_with("\r\n\r\n"), "{deleted}");
    let teapot = get(port, "/teapot");
    assert!(teapot.starts_with("HTTP/1.1 418 \r\n"), "{teapot}");

    // Requests the server refuses before any handler runs.
    assert_eq!(status(&send(port, "NONSENSE\r\n\r\n")), "400");
    assert_eq!(status(&send(port, "GET nopath HTTP/1.1\r\n\r\n")), "400");
    assert_eq!(
        status(&send(port, "GET / HTTP/1.1\r\nBad Header\r\n\r\n")),
        "400"
    );
    assert_eq!(
        status(&send(
            port,
            "POST /length HTTP/1.1\r\nContent-Length: 99999999999\r\n\r\n"
        )),
        "413"
    );
    assert_eq!(
        status(&send(
            port,
            "POST /length HTTP/1.1\r\nContent-Length: 4\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n"
        )),
        "400"
    );
    let huge = format!("GET / HTTP/1.1\r\nX-Big: {}\r\n\r\n", "a".repeat(70_000));
    assert_eq!(status(&send(port, &huge)), "431");
}

#[test]
fn middleware_runs_in_order_and_can_stop_the_chain() {
    let port = serve(
        r#"
        let calls = [];
        router.use(fn(req, res, next) {
            calls.push("first");
            res.header("X-First", "1");
            next();
        });
        router.use("/api", fn(req, res, next) {
            calls.push("api");
            if (req.headers["x-token"] != "secret") {
                res.status(401).send("no token");
                return;
            }
            next();
        });
        router.use(async fn(req, res, next) {
            calls.push("async");
            await sleep(1);
            await next();
            res.header("X-After", "added after the handler");
        });
        router.get("/calls", fn(req, res) {
            res.json(calls);
            calls.clear();
        });
        router.get("/api/data", fn(req, res) => "data");
        router.get("/apiary", fn(req, res) => "bees");
        router.get("/slow", async fn(req, res) {
            await sleep(5);
            res.send("slow");
        });
        "#,
    );

    // Order of registration; the "/api" middleware does not run for /calls.
    let calls = get(port, "/calls");
    assert_eq!(body(&calls), r#"["first","async"]"#, "{calls}");
    assert_eq!(header(&calls, "X-First"), Some("1"));
    // An async middleware can add headers after `await next()`.
    assert_eq!(header(&calls, "X-After"), Some("added after the handler"));

    // A middleware that answers ends the chain.
    let denied = get(port, "/api/data");
    assert_eq!(status(&denied), "401", "{denied}");
    assert_eq!(body(&denied), "no token");
    assert_eq!(header(&denied, "X-First"), Some("1"));
    assert_eq!(header(&denied, "X-After"), None);
    assert_eq!(
        body(&get(port, "/calls")),
        r#"["first","api","first","async"]"#
    );

    let allowed = request(port, "GET", "/api/data", "X-Token: secret\r\n");
    assert_eq!(body(&allowed), "data");
    // "/api" covers /api and /api/..., not /apiary.
    let calls = body(&get(port, "/calls")).to_string();
    assert_eq!(calls, r#"["first","api","async","first","async"]"#);
    assert_eq!(body(&get(port, "/apiary")), "bees");

    let slow = get(port, "/slow");
    assert_eq!(body(&slow), "slow");
    assert_eq!(header(&slow, "X-After"), Some("added after the handler"));
    // A 404 still goes through the middleware.
    let missing = get(port, "/missing");
    assert_eq!(status(&missing), "404");
    assert_eq!(header(&missing, "X-After"), Some("added after the handler"));
}

#[test]
fn middleware_mistakes_and_errors_become_500s() {
    let port = serve(
        r#"
        router.use("/forgot", fn(req, res, next) {
            // Neither next() nor a response.
        });
        router.use("/twice", fn(req, res, next) {
            next();
            next();
        });
        router.use("/throws", fn(req, res, next) {
            throw Error("password=hunter2");
        });
        router.use("/rejects", async fn(req, res, next) {
            await sleep(1);
            throw Error("async secret");
        });
        router.use("/next-error", fn(req, res, next) {
            next(Error("passed to next"));
        });
        router.use("/caught", fn(req, res, next) {
            try {
                next();
            } catch (e) {
                res.status(503).send("recovered: " + e.message);
            }
        });
        router.get("/forgot", fn(req, res) => "unreachable");
        router.get("/twice", fn(req, res) => "handler ran");
        router.get("/throws", fn(req, res) => "unreachable");
        router.get("/rejects", fn(req, res) => "unreachable");
        router.get("/next-error", fn(req, res) => "unreachable");
        router.get("/caught", fn(req, res) { throw Error("handler failed"); });
        router.get("/sent-twice", fn(req, res) {
            res.send("one");
            res.send("two");
        });
        router.get("/bad-send", fn(req, res) { res.send({ "a": 1 }); });
        "#,
    );
    for path in [
        "/forgot",
        "/twice",
        "/throws",
        "/rejects",
        "/next-error",
        "/sent-twice",
        "/bad-send",
    ] {
        let response = get(port, path);
        assert!(
            response.starts_with("HTTP/1.1 500 Internal Server Error\r\n"),
            "{path}: {response}"
        );
        assert_eq!(body(&response), "Internal Server Error", "{path}");
    }
    // A middleware can catch an error from the rest of the chain.
    let caught = get(port, "/caught");
    assert_eq!(status(&caught), "503", "{caught}");
    assert_eq!(body(&caught), "recovered: handler failed");
}

#[test]
fn on_error_replaces_the_500_page() {
    let port = serve(
        r#"
        router.get("/sync", fn(req, res) { throw Error("sync failure"); });
        router.get("/async", async fn(req, res) {
            await sleep(1);
            throw Error("async failure");
        });
        router.get("/partial", fn(req, res) {
            res.header("X-Leak", "should not survive").cookie("half", "done");
            throw Error("after headers");
        });
        router.get("/hook-fails", fn(req, res) { throw Error("first"); });
        router.get("/hook-silent", fn(req, res) { throw Error("quiet"); });
        router.onError(async fn(err, req, res) {
            if (req.path == "/hook-fails") {
                throw Error("the hook broke");
            }
            if (req.path == "/hook-silent") {
                return;
            }
            await sleep(1);
            res.status(502).json({ "error": err.message, "path": req.path });
        });
        "#,
    );
    let sync = get(port, "/sync");
    assert_eq!(status(&sync), "502", "{sync}");
    assert_eq!(body(&sync), r#"{"error":"sync failure","path":"/sync"}"#);
    let async_response = get(port, "/async");
    assert_eq!(
        body(&async_response),
        r#"{"error":"async failure","path":"/async"}"#
    );
    // The hook starts from a clean response.
    let partial = get(port, "/partial");
    assert_eq!(status(&partial), "502");
    assert_eq!(header(&partial, "X-Leak"), None, "{partial}");
    assert_eq!(header(&partial, "Set-Cookie"), None, "{partial}");
    // A hook that fails, or sends nothing, leaves the plain 500.
    for path in ["/hook-fails", "/hook-silent"] {
        let response = get(port, path);
        assert_eq!(status(&response), "500", "{path}: {response}");
        assert_eq!(body(&response), "Internal Server Error");
    }
}

/// A folder with a `public` folder to serve, and files beside it that must
/// stay private.
fn static_fixture(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pleno-http-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let public = dir.join("public");
    std::fs::create_dir_all(public.join("docs")).unwrap();
    std::fs::create_dir_all(public.join("empty")).unwrap();
    std::fs::write(public.join("index.html"), "<h1>home</h1>").unwrap();
    std::fs::write(public.join("style.css"), "body {}").unwrap();
    std::fs::write(public.join("app.js"), "let x = 1;").unwrap();
    std::fs::write(public.join("data.bin"), [0u8, 159, 146, 150]).unwrap();
    std::fs::write(public.join("docs/index.html"), "<h1>docs</h1>").unwrap();
    std::fs::write(public.join(".env"), "API_KEY=secret").unwrap();
    std::fs::write(dir.join("secret.txt"), "top secret").unwrap();
    dir
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[test]
fn static_files_are_served_with_types_index_and_caching() {
    let dir = static_fixture("serve");
    let public = slash_path(&dir.join("public"));
    let port = serve_with(
        format!(
            r#"
            router.static("/assets", "{public}");
            router.get("/assets/dynamic", fn(req, res) => "from a route");
            router.get("/.env", fn(req, res) => "route, not the file");
            "#
        ),
        None,
    );

    let css = get(port, "/assets/style.css");
    assert_eq!(status(&css), "200", "{css}");
    assert_eq!(
        header(&css, "Content-Type"),
        Some("text/css; charset=utf-8")
    );
    assert_eq!(header(&css, "X-Content-Type-Options"), Some("nosniff"));
    assert_eq!(body(&css), "body {}");
    assert_eq!(
        header(&get(port, "/assets/app.js"), "Content-Type"),
        Some("text/javascript; charset=utf-8")
    );
    let binary = get(port, "/assets/data.bin");
    assert_eq!(
        header(&binary, "Content-Type"),
        Some("application/octet-stream")
    );
    assert_eq!(header(&binary, "Content-Length"), Some("4"));

    // Folders serve their index.html; without the trailing slash they redirect.
    assert_eq!(body(&get(port, "/assets/")), "<h1>home</h1>");
    assert_eq!(body(&get(port, "/assets/docs/")), "<h1>docs</h1>");
    let redirect = get(port, "/assets/docs");
    assert_eq!(status(&redirect), "301", "{redirect}");
    assert_eq!(header(&redirect, "Location"), Some("/assets/docs/"));
    let redirect = get(port, "/assets//docs");
    assert_eq!(header(&redirect, "Location"), Some("/assets/docs/"));
    // A folder without index.html, a missing file and other methods go on to the routes.
    assert_eq!(status(&get(port, "/assets/empty/")), "404");
    assert_eq!(body(&get(port, "/assets/dynamic")), "from a route");
    assert_eq!(status(&get(port, "/assets/nothing.txt")), "404");
    assert_eq!(
        status(&request(port, "POST", "/assets/style.css", "")),
        "404"
    );

    // HEAD: the headers of GET, no body.
    let head = request(port, "HEAD", "/assets/style.css", "");
    assert_eq!(header(&head, "Content-Length"), Some("7"));
    assert!(head.ends_with("\r\n\r\n"), "{head}");

    // Conditional requests: 304 without a body.
    let etag = header(&css, "ETag").expect("ETag").to_string();
    let last_modified = header(&css, "Last-Modified")
        .expect("Last-Modified")
        .to_string();
    let cached = request(
        port,
        "GET",
        "/assets/style.css",
        &format!("If-None-Match: {etag}\r\n"),
    );
    assert!(
        cached.starts_with("HTTP/1.1 304 Not Modified\r\n"),
        "{cached}"
    );
    assert_eq!(header(&cached, "Content-Length"), None);
    assert!(cached.ends_with("\r\n\r\n"), "{cached}");
    let cached = request(
        port,
        "GET",
        "/assets/style.css",
        &format!("If-Modified-Since: {last_modified}\r\n"),
    );
    assert_eq!(status(&cached), "304", "{cached}");
    let stale = request(
        port,
        "GET",
        "/assets/style.css",
        "If-None-Match: \"something-else\"\r\n",
    );
    assert_eq!(status(&stale), "200");

    // Dotfiles are not served; the request goes on to the routes.
    assert_eq!(status(&get(port, "/assets/.env")), "404");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn static_files_cannot_leave_their_folder() {
    let dir = static_fixture("traversal");
    let public = slash_path(&dir.join("public"));
    let secret = slash_path(&dir.join("secret.txt"));
    let port = serve_with(format!(r#"router.static("/", "{public}");"#), None);

    for target in [
        "/../secret.txt",
        "/docs/../../secret.txt",
        "/%2e%2e/secret.txt",
        "/%2E%2E%2Fsecret.txt",
        "/docs/%2e%2e%2f%2e%2e%2fsecret.txt",
        "/..%5csecret.txt",
        "/%5c..%5csecret.txt",
        "/a%00b",
        "/C:/Windows/win.ini",
        "/C:%5cWindows%5cwin.ini",
    ] {
        let response = get(port, target);
        assert!(
            !response.contains("top secret"),
            "{target} leaked the file: {response}"
        );
        assert_eq!(status(&response), "403", "{target}: {response}");
    }
    // An absolute path in the URL is a path inside the folder (on Windows
    // its drive letter is refused).
    let absolute = get(port, &format!("/{secret}"));
    assert!(!absolute.contains("top secret"), "{absolute}");
    assert!(["403", "404"].contains(&status(&absolute)), "{absolute}");
    let double = get(port, &format!("//{}", secret.trim_start_matches('/')));
    assert!(!double.contains("top secret"), "{double}");

    // A symbolic link that points out of the folder is refused.
    let link = dir.join("public").join("escape");
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(dir.join("secret.txt"), &link).is_ok();
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_file(dir.join("secret.txt"), &link).is_ok();
    if linked {
        let response = get(port, "/escape");
        assert!(!response.contains("top secret"), "{response}");
        assert_eq!(status(&response), "403", "{response}");
    } else {
        eprintln!("skipped the symbolic link check: cannot create links here");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn static_files_obey_the_sandbox() {
    let dir = static_fixture("sandbox");
    let public = slash_path(&dir.join("public"));
    let elsewhere = slash_path(&dir.join("public").join("docs"));
    let source = |folder: &str| {
        format!(
            "import {{ Router }} from \"std:http\";\nlet r = Router();\nr.static(\"/\", \"{folder}\");\n"
        )
    };
    let run = |config: SandboxConfig| {
        let mut vm = VM::new();
        pleno_stdlib::register(&mut vm);
        vm.apply_config(config);
        vm.execute(compile(&source(&public))).map_err(|e| e.message)
    };
    let base = SandboxConfig {
        allow_io: true,
        allow_network: true,
        ..SandboxConfig::default()
    };
    // Reading only another folder: the static folder is refused.
    let error = run(SandboxConfig {
        allowed_read_paths: vec![elsewhere],
        ..base.clone()
    })
    .expect_err("read rule");
    assert!(error.contains("SecurityError"), "{error}");
    // No file access at all.
    let error = run(SandboxConfig {
        allow_io: false,
        ..base.clone()
    })
    .expect_err("no io");
    assert!(error.contains("SecurityError"), "{error}");
    // Allowed: files are served.
    let port = serve_with(
        format!(r#"router.static("/", "{public}");"#),
        Some(SandboxConfig {
            allowed_read_paths: vec![public.clone()],
            ..base
        }),
    );
    assert_eq!(body(&get(port, "/style.css")), "body {}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cookies_are_parsed_and_set() {
    let port = serve(
        r#"
        router.get("/read", fn(req, res) => req.cookies.session + "|" + req.cookies.name + "|" + (req.cookies.none ?? "none"));
        router.get("/set", fn(req, res) {
            res.cookie("session", "abc123")
               .cookie("prefs", "dark mode; big=yes", { "maxAge": 3600, "httpOnly": false, "sameSite": "Strict", "path": "/app", "domain": "example.com", "secure": true })
               .cookie("session", "replaced")
               .send("ok");
        });
        router.get("/clear", fn(req, res) {
            res.clearCookie("session").clearCookie("prefs", { "path": "/app" }).send("cleared");
        });
        "#,
    );
    let read = request(
        port,
        "GET",
        "/read",
        "Cookie: session=abc; name=\"Ann%20Lee\"\r\nCookie: session=ignored\r\n",
    );
    assert_eq!(body(&read), "abc|Ann Lee|none", "{read}");

    let set = get(port, "/set");
    assert_eq!(
        headers(&set, "Set-Cookie"),
        vec![
            "session=replaced; Path=/; HttpOnly; SameSite=Lax",
            "prefs=dark%20mode%3B%20big=yes; Max-Age=3600; Domain=example.com; Path=/app; Secure; SameSite=Strict",
        ],
        "{set}"
    );
    let clear = get(port, "/clear");
    assert_eq!(
        headers(&clear, "Set-Cookie"),
        vec![
            "session=; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path=/; HttpOnly; SameSite=Lax",
            "prefs=; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path=/app; HttpOnly; SameSite=Lax",
        ],
        "{clear}"
    );
}

#[test]
fn header_injection_is_refused() {
    let port = serve(
        r#"
        router.get("/header", fn(req, res) { res.header("X-Echo", req.query.v).send("ok"); });
        router.get("/header-name", fn(req, res) { res.header(req.query.v, "x").send("ok"); });
        router.get("/cookie", fn(req, res) { res.cookie("c", req.query.v).send("ok"); });
        router.get("/cookie-name", fn(req, res) { res.cookie(req.query.v, "x").send("ok"); });
        router.get("/cookie-path", fn(req, res) { res.cookie("c", "x", { "path": req.query.v }).send("ok"); });
        router.get("/redirect", fn(req, res) { res.redirect(req.query.v); });
        router.get("/length", fn(req, res) { res.header("Content-Length", "1").send("ok"); });
        router.get("/bad-option", fn(req, res) { res.cookie("c", "x", { "maxage": 10 }).send("ok"); });
        router.get("/none-insecure", fn(req, res) { res.cookie("c", "x", { "sameSite": "None" }).send("ok"); });
        "#,
    );
    let injected = "a%0D%0ASet-Cookie:%20evil=1";
    for path in [
        "/header",
        "/header-name",
        "/cookie",
        "/cookie-name",
        "/cookie-path",
        "/redirect",
    ] {
        let response = get(port, &format!("{path}?v={injected}"));
        assert_eq!(status(&response), "500", "{path}: {response}");
        assert!(!response.contains("evil"), "{path}: {response}");
    }
    // Only a bare LF.
    let response = get(port, "/header?v=a%0Aevil:%201");
    assert_eq!(status(&response), "500", "{response}");
    assert!(!response.contains("evil"), "{response}");
    // Safe values pass.
    assert_eq!(body(&get(port, "/header?v=fine")), "ok");
    let redirect = get(port, "/redirect?v=/elsewhere");
    assert_eq!(status(&redirect), "302");
    assert_eq!(header(&redirect, "Location"), Some("/elsewhere"));
    for path in ["/length", "/bad-option", "/none-insecure"] {
        assert_eq!(status(&get(port, path)), "500", "{path}");
    }
}

#[test]
fn a_server_survives_garbage_collection() {
    // `serve_with` does not keep the server in a variable: only the event
    // loop refers to it, and a collection must not clear it, nor the values
    // of a request that waits for a promise.
    let port = serve(
        r#"
        router.use(async fn(req, res, next) {
            collect();
            await sleep(1);
            collect();
            await next();
        });
        router.get("/", fn(req, res) {
            collect();
            return "still here";
        });
        "#,
    );
    for _ in 0..3 {
        assert_eq!(body(&get(port, "/")), "still here");
    }
}

#[test]
fn a_server_listens_on_this_machine_only_unless_asked() {
    // `listen(port)` binds 127.0.0.1, so tests and examples are not exposed to
    // the network (and Windows asks no firewall question); `host:` chooses.
    for (host_arg, expected) in [("", "127.0.0.1"), (", host: \"localhost\"", "localhost")] {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .expect("free port")
            .port();
        let source = format!(
            "import {{ Router, Server }} from \"std:http\";
             let r = Router();
             let s = Server(r);
             r.get(\"/host\", fn(req, res) => s.host);
             s.listen({port}{host_arg});"
        );
        std::thread::spawn(move || {
            let mut vm = VM::new();
            pleno_stdlib::register(&mut vm);
            vm.output = Some(String::new());
            let _ = vm.execute(compile(&source));
        });
        let mut ready = false;
        for _ in 0..200 {
            if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
                ready = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(ready, "the server never accepted connections on 127.0.0.1");
        assert_eq!(body(&get(port, "/host")), expected);
    }
}
