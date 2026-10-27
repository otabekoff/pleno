//! `std:net` over real loopback connections: sockets bound to port 0,
//! echo in one program, a Pleno server with a Rust client and the other way
//! round, refused connections, the network permission, timeouts, closing,
//! and closing when the VM goes away.

use pleno_compiler::Compiler;
use pleno_value::{SandboxConfig, VMProvider, Value};
use pleno_vm::VM;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

fn compile(source: &str) -> pleno_bytecode::Chunk {
    let tokens = pleno_lexer::lex(source).expect("lexing failed");
    let mut program = pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .expect("parsing failed");
    Compiler::new(source.to_string())
        .compile(&mut program)
        .expect("compilation failed")
}

fn new_vm(allow_network: bool) -> VM {
    let mut vm = VM::new();
    vm.output = Some(String::new());
    pleno_stdlib::register(&mut vm);
    if !allow_network {
        vm.apply_config(SandboxConfig {
            allow_io: true,
            allow_network: false,
            ..Default::default()
        });
    }
    vm
}

/// Runs `source` and returns what it printed.
fn run(source: &str) -> String {
    let mut vm = new_vm(true);
    let chunk = compile(source);
    if let Err(error) = vm.execute(chunk) {
        panic!(
            "program failed: {}\noutput:\n{}",
            error.message,
            vm.output.take().unwrap_or_default()
        );
    }
    vm.output.take().unwrap_or_default()
}

/// A port nothing listens on.
fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

#[test]
fn echo_server_and_client_in_one_program() {
    let output = run(r#"
        import { Net } from "std:net";
        let listener = Net.listen("127.0.0.1", 0);
        print(listener.port > 0);
        print(listener.address == "127.0.0.1:" + listener.port.toString());

        async fn serve() {
            let conn = await listener.accept();
            let mut line = await conn.readLine();
            while (line != null) {
                await conn.write(line.upper() + "\n");
                line = await conn.readLine();
            }
            conn.close();
        }

        let server = serve();
        let client = await Net.connect("127.0.0.1", listener.port);
        print(client.remoteAddress == listener.address);
        await client.write("hello\nworld\n");
        print(await client.readLine());
        print(await client.readLine());
        client.close();
        await server;
        listener.close();
    "#);
    assert_eq!(output, "true\ntrue\ntrue\nHELLO\nWORLD\n");
}

#[test]
fn pleno_server_serves_a_rust_client() {
    let port = closed_port();
    let server = std::thread::spawn(move || {
        run(&format!(
            r#"
            import {{ Net }} from "std:net";
            let listener = Net.listen("127.0.0.1", {port});
            let conn = await listener.accept();
            let mut line = await conn.readLine();
            while (line != null) {{
                await conn.write("echo: " + line + "\n");
                line = await conn.readLine();
            }}
            conn.close();
            listener.close();
            print("served");
            "#
        ))
    });
    let mut stream = (0..200)
        .find_map(|_| {
            TcpStream::connect(("127.0.0.1", port)).ok().or_else(|| {
                std::thread::sleep(Duration::from_millis(25));
                None
            })
        })
        .expect("server did not start");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    // A line split across two writes, and a CRLF line ending.
    stream.write_all(b"one\r\ntw").unwrap();
    stream.flush().unwrap();
    std::thread::sleep(Duration::from_millis(50));
    stream.write_all(b"o\n").unwrap();
    stream.shutdown(std::net::Shutdown::Write).unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    assert_eq!(reply, "echo: one\necho: two\n");
    assert_eq!(server.join().unwrap(), "served\n");
}

#[test]
fn pleno_client_talks_to_a_rust_server() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let mut stream = stream;
        stream
            .write_all(format!("got {}", line).as_bytes())
            .unwrap();
        stream.write_all("héllo".as_bytes()).unwrap();
        // Closing ends the stream: the client's next read gives null.
    });
    let output = run(&format!(
        r#"
        import {{ Net }} from "std:net";
        let socket = await Net.connect("127.0.0.1", {port});
        await socket.write("ping\n");
        print(await socket.readLine());
        let mut rest = "";
        let mut chunk = await socket.read(2);
        while (chunk != null) {{
            rest = rest + chunk;
            chunk = await socket.read(2);
        }}
        print(rest);
        print(await socket.readLine());
        socket.close();
        "#
    ));
    server.join().unwrap();
    assert_eq!(output, "got ping\nhéllo\nnull\n");
}

#[test]
fn a_refused_connection_is_a_clear_error() {
    let port = closed_port();
    let output = run(&format!(
        r#"
        import {{ Net }} from "std:net";
        try {{
            await Net.connect("127.0.0.1", {port});
        }} catch (e) {{
            print(e.name);
            print(e.message);
        }}
        "#
    ));
    assert_eq!(
        output,
        format!("Error\nconnection refused: 127.0.0.1:{port}\n")
    );
}

#[test]
fn denied_network_access_is_a_security_error() {
    let mut vm = new_vm(false);
    vm.execute(compile(
        r#"
        import { Net } from "std:net";
        try { Net.listen("127.0.0.1", 0); } catch (e) { print(e.name + ": " + e.message); }
        try { Net.connect("127.0.0.1", 80); } catch (e) { print(e.name + ": " + e.message); }
        "#,
    ))
    .unwrap();
    assert_eq!(
        vm.output.take().unwrap(),
        "SecurityError: network access denied: listen on 127.0.0.1:0\n\
         SecurityError: network access denied: connect to 127.0.0.1:80\n"
    );
}

#[test]
fn a_sandbox_without_network_denies_sockets() {
    let output = run(r#"
        import { Net } from "std:net";
        import { Sandbox } from "std:security";
        let sandbox = Sandbox({ allowNetwork: false });
        try {
            sandbox.run(fn() { return Net.listen("127.0.0.1", 0); });
        } catch (e) {
            print(e.name);
        }
        // A socket made outside cannot be used inside either.
        let listener = Net.listen("127.0.0.1", 0);
        try {
            sandbox.run(fn() { return listener.accept(); });
        } catch (e) {
            print(e.name + ": " + e.message);
        }
        listener.close();
    "#);
    assert_eq!(
        output,
        "SecurityError\nSecurityError: network access denied: accept()\n"
    );
}

#[test]
fn reads_and_accepts_time_out() {
    let output = run(r#"
        import { Net } from "std:net";
        let listener = Net.listen("127.0.0.1", 0);
        listener.setTimeout(50);
        try { await listener.accept(); } catch (e) { print(e.name + ": " + e.message); }
        listener.setTimeout(null);

        let connecting = Net.connect("127.0.0.1", listener.port);
        let server = await listener.accept();
        let client = await connecting;
        client.setTimeout(50);
        try { await client.readLine(); } catch (e) { print(e.name + ": " + e.message); }
        try { await client.read(); } catch (e) { print(e.name + ": " + e.message); }
        // The socket still works after a timeout.
        await server.write("late\n");
        print(await client.readLine());
        server.close();
        client.close();
        listener.close();
    "#);
    assert_eq!(
        output,
        "TimeoutError: accept() timed out after 50 ms\n\
         TimeoutError: readLine() timed out after 50 ms\n\
         TimeoutError: read() timed out after 50 ms\n\
         late\n"
    );
}

#[test]
fn closing_is_idempotent_and_a_closed_socket_says_so() {
    let output = run(r#"
        import { Net } from "std:net";
        let listener = Net.listen("127.0.0.1", 0);
        let connecting = Net.connect("127.0.0.1", listener.port);
        let server = await listener.accept();
        let client = await connecting;

        // A read waiting when the socket closes fails.
        let waiting = client.readLine();
        client.close();
        try { await waiting; } catch (e) { print(e.message); }
        client.close();
        try { client.read(); } catch (e) { print(e.message); }
        try { client.write("x"); } catch (e) { print(e.message); }

        // The peer sees the end of the stream.
        print(await server.read());

        let pending = listener.accept();
        listener.close();
        listener.close();
        try { await pending; } catch (e) { print(e.message); }
        try { listener.accept(); } catch (e) { print(e.message); }
        server.close();
    "#);
    assert_eq!(
        output,
        "the socket was closed while a read was waiting\n\
         cannot read(): the socket is closed\n\
         cannot write(): the socket is closed\n\
         null\n\
         the listener was closed while accept() was waiting\n\
         cannot accept(): the listener is closed\n"
    );
}

#[test]
fn bad_arguments_are_explained() {
    let output = run(r#"
        import { Net } from "std:net";
        try { Net.listen("127.0.0.1:0"); } catch (e) { print(e.message); }
        try { Net.connect("127.0.0.1", 70000); } catch (e) { print(e.message); }
        let listener = Net.listen("127.0.0.1", 0);
        try { listener.setTimeout(-1); } catch (e) { print(e.message); }
        try { Net.listen("127.0.0.1", listener.port); } catch (e) { print(e.message.startsWith("cannot listen on 127.0.0.1:")); }
        listener.close();
    "#);
    assert_eq!(
        output,
        "listen(host, port) expects a host String and a port Int, as in listen(\"127.0.0.1\", 8080)\n\
         connect(): port 70000 is out of range (0 to 65535)\n\
         a timeout is a number of milliseconds greater than 0, or null for none; got -1 (Int)\n\
         true\n"
    );
}

#[test]
fn sockets_close_when_the_vm_is_dropped() {
    let mut vm = new_vm(true);
    let port = vm
        .execute(compile(
            r#"
            import { Net } from "std:net";
            let listener = Net.listen("127.0.0.1", 0);
            listener.port
            "#,
        ))
        .unwrap();
    let Value::Int(port) = port else {
        panic!("expected a port, got {port:?}");
    };
    // The kernel completes the handshake while the listener is open.
    assert!(TcpStream::connect(("127.0.0.1", port as u16)).is_ok());
    drop(vm);
    assert!(TcpStream::connect(("127.0.0.1", port as u16)).is_err());
}

#[test]
fn an_unreachable_listener_closes_when_collected() {
    let mut vm = new_vm(true);
    let port = vm
        .execute(compile(
            r#"
            import { Net } from "std:net";
            fn open(): Int {
                let listener = Net.listen("127.0.0.1", 0);
                return listener.port;
            }
            let port = open();
            // Reuse the registers `open` used.
            fn other(): Int { let a = 1; let b = 2; let c = 3; return a + b + c; }
            other();
            port
            "#,
        ))
        .unwrap();
    let Value::Int(port) = port else {
        panic!("expected a port, got {port:?}");
    };
    vm.collect();
    assert!(TcpStream::connect(("127.0.0.1", port as u16)).is_err());
}
