//! Drives a full debug session through the adapter, as an editor would.

use pleno_dap::server::DapServer;
use serde_json::{Value, json};
use std::io::Cursor;

fn frame(seq: i64, command: &str, arguments: Value) -> String {
    let body = json!({ "seq": seq, "type": "request", "command": command, "arguments": arguments })
        .to_string();
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

fn parse_messages(output: &[u8]) -> Vec<Value> {
    let text = String::from_utf8_lossy(output);
    text.split("Content-Length: ")
        .filter_map(|chunk| chunk.split_once("\r\n\r\n").map(|(_, body)| body))
        .map(|body| serde_json::from_str(body).expect("valid JSON message"))
        .collect()
}

fn run_session(requests: &[String]) -> Vec<Value> {
    let input = Cursor::new(requests.concat().into_bytes());
    let mut output = Vec::new();
    DapServer::new(input, &mut output)
        .run()
        .expect("session runs");
    parse_messages(&output)
}

fn script(name: &str, source: &str) -> String {
    let path = std::env::temp_dir().join(format!("pleno-dap-{}-{name}", std::process::id()));
    std::fs::write(&path, source).unwrap();
    path.to_string_lossy().into_owned()
}

fn events<'a>(messages: &'a [Value], name: &str) -> Vec<&'a Value> {
    messages
        .iter()
        .filter(|m| m["type"] == "event" && m["event"] == name)
        .collect()
}

fn response<'a>(messages: &'a [Value], command: &str) -> &'a Value {
    messages
        .iter()
        .find(|m| m["type"] == "response" && m["command"] == command)
        .unwrap_or_else(|| panic!("no {command} response"))
}

#[test]
fn breakpoint_step_and_output() {
    let program = script(
        "main.pln",
        "let a = 1;\nlet b = a + 1;\nprint(\"b is \" + b);\nprint(\"done\");\n",
    );
    let messages = run_session(&[
        frame(1, "initialize", json!({ "adapterID": "pleno" })),
        frame(2, "launch", json!({ "program": program })),
        frame(
            3,
            "setBreakpoints",
            json!({ "source": { "path": program }, "breakpoints": [{ "line": 2 }] }),
        ),
        frame(4, "setExceptionBreakpoints", json!({ "filters": [] })),
        frame(5, "configurationDone", json!({})),
        frame(6, "stackTrace", json!({ "threadId": 1 })),
        frame(7, "next", json!({ "threadId": 1 })),
        frame(8, "stackTrace", json!({ "threadId": 1 })),
        frame(9, "continue", json!({ "threadId": 1 })),
        frame(10, "disconnect", json!({})),
    ]);

    for command in [
        "initialize",
        "launch",
        "setBreakpoints",
        "setExceptionBreakpoints",
        "configurationDone",
    ] {
        assert_eq!(
            response(&messages, command)["success"],
            true,
            "{command} failed"
        );
    }

    let traces: Vec<&Value> = messages
        .iter()
        .filter(|m| m["type"] == "response" && m["command"] == "stackTrace")
        .collect();
    assert_eq!(
        traces[0]["body"]["stackFrames"][0]["line"], 2,
        "stops at the breakpoint"
    );
    assert_eq!(
        traces[1]["body"]["stackFrames"][0]["line"], 3,
        "`next` moves one line"
    );

    let output: String = events(&messages, "output")
        .iter()
        .map(|e| e["body"]["output"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(output, "b is 2\ndone\n");
    assert_eq!(events(&messages, "stopped").len(), 2);
    assert_eq!(events(&messages, "exited")[0]["body"]["exitCode"], 0);
}

#[test]
fn compile_errors_fail_the_launch() {
    let program = script("broken.pln", "let x = ;\n");
    let messages = run_session(&[
        frame(1, "initialize", json!({})),
        frame(2, "launch", json!({ "program": program })),
        frame(3, "disconnect", json!({})),
    ]);
    let launch = response(&messages, "launch");
    assert_eq!(launch["success"], false);
    assert!(launch["message"].as_str().unwrap().contains("Syntax error"));
}

#[test]
fn runtime_errors_are_reported_as_output() {
    let program = script("crash.pln", "print(\"before\");\nlet x = 1 / 0;\n");
    let messages = run_session(&[
        frame(1, "initialize", json!({})),
        frame(2, "launch", json!({ "program": program })),
        frame(3, "configurationDone", json!({})),
        frame(4, "disconnect", json!({})),
    ]);
    let stderr: String = events(&messages, "output")
        .iter()
        .filter(|e| e["body"]["category"] == "stderr")
        .map(|e| e["body"]["output"].as_str().unwrap_or_default())
        .collect();
    assert!(stderr.contains("division by zero"), "stderr was {stderr:?}");
    assert_eq!(events(&messages, "exited")[0]["body"]["exitCode"], 1);
}

#[test]
fn step_into_and_out_of_a_function() {
    let program = script(
        "calls.pln",
        "fn double(n) {\n    let r = n * 2;\n    return r;\n}\nlet x = double(4);\nprint(x);\n",
    );
    let messages = run_session(&[
        frame(1, "initialize", json!({})),
        frame(2, "launch", json!({ "program": program })),
        frame(
            3,
            "setBreakpoints",
            json!({ "source": { "path": program }, "breakpoints": [{ "line": 5 }] }),
        ),
        frame(4, "configurationDone", json!({})),
        frame(5, "stepIn", json!({ "threadId": 1 })),
        frame(6, "stackTrace", json!({ "threadId": 1 })),
        frame(7, "stepOut", json!({ "threadId": 1 })),
        frame(8, "stackTrace", json!({ "threadId": 1 })),
        frame(9, "continue", json!({ "threadId": 1 })),
        frame(10, "disconnect", json!({})),
    ]);
    let traces: Vec<&Value> = messages
        .iter()
        .filter(|m| m["type"] == "response" && m["command"] == "stackTrace")
        .collect();
    let inner = &traces[0]["body"]["stackFrames"];
    assert_eq!(inner[0]["name"], "double", "stepIn enters the call");
    assert_eq!(inner[0]["line"], 2);
    let outer = &traces[1]["body"]["stackFrames"];
    assert_eq!(
        outer[0]["name"], "<script>",
        "stepOut returns to the caller"
    );
    assert_eq!(outer.as_array().unwrap().len(), 1);
}
