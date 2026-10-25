use pleno_compiler::Compiler;
use pleno_lexer::lex;
use pleno_parser::Parser;
use pleno_value::Value;
use pleno_vm::VM;

fn run_source(source: &str) -> Value {
    let _ = env_logger::builder().is_test(true).try_init();
    let tokens = lex(source).expect("Lexing failed");
    let mut parser = Parser::new(tokens, source);
    let mut program = parser.parse_program().expect("Parsing failed");
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler.compile(&mut program).expect("Compilation failed");
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).expect("Execution failed")
}

#[test]
fn test_http_router_registration() {
    let source = r#"
        import { Router } from "std:http";
        let r = Router();
        r.get("/", fn(req, res) { return "Home"; });
        r.post("/api", fn(req, res) { return "API"; });
        true
    "#;
    match run_source(source) {
        Value::Bool(b) => assert!(b),
        val => panic!("Expected bool, got {:?}", val),
    }
}

/// Starts a server for `routes` (Pleno code calling `router.get(...)` etc.)
/// on a free local port, in a thread of its own, and returns the port once
/// the server accepts connections. The thread runs until the test exits.
fn serve(routes: &'static str) -> u16 {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("free port")
        .port();
    std::thread::spawn(move || {
        let source = format!(
            r#"
            import {{ Router, Server }} from "std:http";
            let router = Router();
            {routes}
            Server(router).listen({port});
            "#
        );
        let tokens = lex(&source).expect("Lexing failed");
        let mut parser = Parser::new(tokens, &source);
        let mut program = parser.parse_program().expect("Parsing failed");
        let mut compiler = Compiler::new(source.clone());
        let chunk = compiler.compile(&mut program).expect("Compilation failed");
        let mut vm = VM::new();
        pleno_stdlib::register(&mut vm);
        vm.execute(chunk).expect("server failed");
    });
    for _ in 0..200 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return port;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("server did not start on port {port}");
}

/// Sends `request` and returns the whole response.
fn send(port: u16, request: &str) -> String {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
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
        router.get("/fail", fn(req, res) { throw Error("boom"); });
        "#,
    );

    let get = send(port, "GET /hello/ann HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(get.starts_with("HTTP/1.1 200 OK\r\n"), "{get}");
    assert!(get.contains("Content-Length: 10\r\n"), "{get}");
    assert!(get.contains("X-Method: GET\r\n"), "{get}");
    assert!(get.ends_with("\r\n\r\nHello, ann"), "{get}");

    // HEAD on a `get` route: the same status and headers, no body.
    let head = send(port, "HEAD /hello/ann HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(head.starts_with("HTTP/1.1 200 OK\r\n"), "{head}");
    assert!(head.contains("Content-Length: 10\r\n"), "{head}");
    assert!(head.contains("X-Method: HEAD\r\n"), "{head}");
    assert!(head.ends_with("\r\n\r\n"), "{head}");

    let post = send(
        port,
        "POST /echo HTTP/1.1\r\nHost: x\r\nContent-Length: 4\r\n\r\nping",
    );
    assert!(post.starts_with("HTTP/1.1 200 OK\r\n"), "{post}");
    assert!(post.ends_with("\r\n\r\nping"), "{post}");

    // An async handler answers once its promise settles; HEAD drops the body.
    let later = send(port, "GET /later HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(later.ends_with("\r\n\r\ndone"), "{later}");
    let later_head = send(port, "HEAD /later HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(later_head.contains("Content-Length: 4\r\n"), "{later_head}");
    assert!(later_head.ends_with("\r\n\r\n"), "{later_head}");

    let fail = send(port, "GET /fail HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(fail.starts_with("HTTP/1.1 500 "), "{fail}");
    assert!(fail.contains("boom"), "{fail}");
    let fail_head = send(port, "HEAD /fail HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(fail_head.starts_with("HTTP/1.1 500 "), "{fail_head}");
    assert!(fail_head.ends_with("\r\n\r\n"), "{fail_head}");

    let missing = send(port, "GET /nope HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(
        missing.starts_with("HTTP/1.1 404 Not Found\r\n"),
        "{missing}"
    );
    let missing_head = send(port, "HEAD /nope HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(
        missing_head.starts_with("HTTP/1.1 404 Not Found\r\n"),
        "{missing_head}"
    );
    assert!(missing_head.ends_with("\r\n\r\n"), "{missing_head}");
    // HEAD only falls back to `get` routes.
    let head_post = send(port, "HEAD /echo HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(
        head_post.starts_with("HTTP/1.1 404 Not Found\r\n"),
        "{head_post}"
    );
}
