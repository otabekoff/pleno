//! Drives the `pleno-lsp` binary over stdio with real JSON-RPC messages.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A workspace folder under the system temp directory, removed on drop.
struct TempWorkspace(PathBuf);

impl TempWorkspace {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("pleno-lsp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn uri(&self, file: &str) -> String {
        url::Url::from_file_path(self.0.join(file))
            .unwrap()
            .to_string()
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The messages sent to the server, run in one session once complete.
struct Session {
    root: Option<String>,
    init_options: Value,
    messages: Vec<Value>,
    next_id: i64,
}

impl Session {
    fn new() -> Self {
        Self {
            root: None,
            init_options: json!({}),
            messages: Vec::new(),
            next_id: 1,
        }
    }

    fn request(&mut self, method: &str, params: Value) -> i64 {
        self.next_id += 1;
        self.messages.push(json!({
            "jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params
        }));
        self.next_id
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.messages
            .push(json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    fn open(&mut self, uri: &str, text: &str) {
        self.notify(
            "textDocument/didOpen",
            json!({ "textDocument": { "uri": uri, "languageId": "pleno", "version": 1, "text": text } }),
        );
    }

    /// Starts the server, sends `initialize` and every message, closes its
    /// input and returns everything it wrote.
    fn run(self) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pleno-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("start pleno-lsp");
        let mut stdin = child.stdin.take().unwrap();
        let initialize = json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": self.root,
                "capabilities": {},
                "initializationOptions": self.init_options
            }
        });
        let initialized = json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} });
        for message in [initialize, initialized].iter().chain(&self.messages) {
            let text = message.to_string();
            write!(stdin, "Content-Length: {}\r\n\r\n{}", text.len(), text).unwrap();
        }
        drop(stdin);

        let mut reader = BufReader::new(child.stdout.take().unwrap());
        let mut messages = Vec::new();
        loop {
            let mut length = None;
            let mut line = String::new();
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap() == 0 {
                    break;
                }
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.strip_prefix("Content-Length:") {
                    length = Some(value.trim().parse::<usize>().unwrap());
                }
            }
            let Some(length) = length else { break };
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            messages.push(serde_json::from_slice(&body).unwrap());
        }
        assert!(child.wait().unwrap().success());
        Output(messages)
    }
}

struct Output(Vec<Value>);

impl Output {
    fn response(&self, id: i64) -> &Value {
        self.0
            .iter()
            .find(|m| m["id"] == id)
            .unwrap_or_else(|| panic!("no response to request {id}"))
    }

    fn result(&self, id: i64) -> &Value {
        &self.response(id)["result"]
    }

    /// The diagnostics last published for `uri`.
    fn diagnostics(&self, uri: &str) -> &Vec<Value> {
        self.0
            .iter()
            .rev()
            .find(|m| m["method"] == "textDocument/publishDiagnostics" && m["params"]["uri"] == uri)
            .and_then(|m| m["params"]["diagnostics"].as_array())
            .expect("diagnostics were published")
    }
}

fn position(line: u32, character: u32) -> Value {
    json!({ "line": line, "character": character })
}

fn range(start: (u32, u32), end: (u32, u32)) -> Value {
    json!({ "start": position(start.0, start.1), "end": position(end.0, end.1) })
}

fn uri_for(path: &Path) -> String {
    url::Url::from_file_path(path).unwrap().to_string()
}

#[test]
fn initialize_advertises_the_editor_features() {
    let output = Session::new().run();
    let capabilities = &output.result(1)["capabilities"];
    assert_eq!(capabilities["documentHighlightProvider"], true);
    assert_eq!(capabilities["workspaceSymbolProvider"], true);
    assert_eq!(capabilities["selectionRangeProvider"], true);
    assert_eq!(capabilities["documentRangeFormattingProvider"], true);
    assert_eq!(capabilities["renameProvider"]["prepareProvider"], true);
    assert_eq!(capabilities["codeActionProvider"], true);
}

#[test]
fn lint_diagnostics_have_a_source_code_tag_and_full_range() {
    let workspace = TempWorkspace::new("diagnostics");
    let uri = workspace.uri("main.pln");
    let mut session = Session::new();
    session.open(&uri, "fn main() {\n    let total = 1;\n}\nmain();\n");
    let output = session.run();

    let diagnostics = output.diagnostics(&uri);
    let unused = diagnostics
        .iter()
        .find(|d| d["code"] == "no-unused-vars")
        .expect("an unused-variable diagnostic");
    assert_eq!(unused["source"], "pleno-lint");
    assert_eq!(unused["tags"], json!([1]));
    assert_eq!(unused["range"], range((1, 8), (1, 13)));
    for diagnostic in diagnostics {
        assert!(diagnostic["source"].is_string(), "{diagnostic}");
        assert!(diagnostic["code"].is_string(), "{diagnostic}");
    }
}

#[test]
fn diagnostics_for_misspelled_names_say_what_was_meant() {
    let workspace = TempWorkspace::new("did-you-mean");
    let uri = workspace.uri("main.pln");
    let mut session = Session::new();
    session.open(
        &uri,
        "let length = 3;\nprint(lenght);\n\
         class A { fn go() { return 1; } fn run() { return this.og(); } }\nprint(A().run());\n",
    );
    let output = session.run();

    let messages: Vec<&str> = output
        .diagnostics(&uri)
        .iter()
        .filter_map(|d| d["message"].as_str())
        .collect();
    assert!(
        messages.contains(&"Usage of undefined variable 'lenght'; did you mean 'length'?"),
        "{messages:?}"
    );
    assert!(
        messages.contains(&"A has no method 'og'; did you mean 'go'?"),
        "{messages:?}"
    );
}

#[test]
fn settings_turn_rules_off() {
    let workspace = TempWorkspace::new("settings");
    let uri = workspace.uri("main.pln");
    let text = "let x=1;\nprint(x);\n";

    let mut session = Session::new();
    session.open(&uri, text);
    let output = session.run();
    assert!(
        output
            .diagnostics(&uri)
            .iter()
            .any(|d| d["code"] == "operator-spacing")
    );

    let mut session = Session::new();
    session.init_options = json!({ "lint": { "rules": { "operator-spacing": false } } });
    session.open(&uri, text);
    let output = session.run();
    assert!(
        !output
            .diagnostics(&uri)
            .iter()
            .any(|d| d["code"] == "operator-spacing")
    );

    // Changing the settings re-publishes the open documents' diagnostics.
    let mut session = Session::new();
    session.open(&uri, text);
    session.notify(
        "workspace/didChangeConfiguration",
        json!({ "settings": { "pleno": { "lint": { "enabled": false } } } }),
    );
    let output = session.run();
    assert!(output.diagnostics(&uri).is_empty());
}

#[test]
fn document_highlight_marks_the_declaration_and_uses() {
    let workspace = TempWorkspace::new("highlight");
    let uri = workspace.uri("main.pln");
    let mut session = Session::new();
    session.open(&uri, "let count = 1;\nprint(count + count);\n");
    let id = session.request(
        "textDocument/documentHighlight",
        json!({ "textDocument": { "uri": uri }, "position": position(1, 8) }),
    );
    let output = session.run();
    assert_eq!(
        *output.result(id),
        json!([
            { "range": range((0, 4), (0, 9)), "kind": 3 },
            { "range": range((1, 6), (1, 11)), "kind": 2 },
            { "range": range((1, 14), (1, 19)), "kind": 2 }
        ])
    );
}

#[test]
fn workspace_symbols_cover_open_and_unopened_files() {
    let workspace = TempWorkspace::new("symbols");
    std::fs::create_dir_all(workspace.0.join("lib")).unwrap();
    std::fs::write(
        workspace.0.join("lib/shapes.pln"),
        "class Point {\n    x: Int;\n    fn norm() { return 0; }\n}\nexport fn parsePoint(s) { return s; }\n",
    )
    .unwrap();
    let main_uri = workspace.uri("main.pln");

    let mut session = Session::new();
    session.root = Some(uri_for(&workspace.0));
    session.open(
        &main_uri,
        "fn printPoint(p) { print(p); }\nprintPoint(1);\n",
    );
    let id = session.request("workspace/symbol", json!({ "query": "point" }));
    let all = session.request("workspace/symbol", json!({ "query": "" }));
    let output = session.run();

    let names: Vec<&str> = output
        .result(id)
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["printPoint", "Point", "parsePoint"]);
    let point = &output.result(id)[1];
    assert_eq!(point["kind"], 5);
    assert!(
        point["location"]["uri"]
            .as_str()
            .unwrap()
            .ends_with("lib/shapes.pln")
    );
    assert_eq!(point["location"]["range"], range((0, 6), (0, 11)));

    let norm = output
        .result(all)
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "norm")
        .expect("class members are listed");
    assert_eq!(norm["containerName"], "Point");
}

#[test]
fn prepare_rename_accepts_names_and_rejects_keywords_and_builtins() {
    let workspace = TempWorkspace::new("rename");
    let uri = workspace.uri("main.pln");
    let mut session = Session::new();
    session.open(&uri, "let count = 1;\nprint(count);\n");
    let doc = json!({ "uri": uri });
    let name = session.request(
        "textDocument/prepareRename",
        json!({ "textDocument": doc, "position": position(1, 9) }),
    );
    let keyword = session.request(
        "textDocument/prepareRename",
        json!({ "textDocument": doc, "position": position(0, 1) }),
    );
    let builtin = session.request(
        "textDocument/prepareRename",
        json!({ "textDocument": doc, "position": position(1, 2) }),
    );
    let output = session.run();

    assert_eq!(
        *output.result(name),
        json!({ "range": range((1, 6), (1, 11)), "placeholder": "count" })
    );
    let error = &output.response(keyword)["error"];
    assert!(
        error["message"].as_str().unwrap().contains("keyword"),
        "{error}"
    );
    let error = &output.response(builtin)["error"];
    assert!(
        error["message"].as_str().unwrap().contains("built in"),
        "{error}"
    );
}

#[test]
fn quick_fixes_remove_or_prefix_unused_names() {
    let workspace = TempWorkspace::new("fixes");
    let uri = workspace.uri("main.pln");
    let text = "import { a, b } from \"m\";\nfn main(arg) {\n    let total = 1;\n    let y=a;\n    print(y);\n}\nmain(0);\n";
    let mut session = Session::new();
    session.open(&uri, text);

    // The client sends back the diagnostics the server published; build them
    // the same way here.
    let unused_import = json!({
        "range": range((0, 12), (0, 13)), "severity": 2, "source": "pleno-lint",
        "code": "no-unused-vars", "message": "Variable 'b' is declared but never used"
    });
    let unused_let = json!({
        "range": range((2, 8), (2, 13)), "severity": 2, "source": "pleno-lint",
        "code": "no-unused-vars", "message": "Variable 'total' is declared but never used"
    });
    let unused_param = json!({
        "range": range((1, 8), (1, 11)), "severity": 2, "source": "pleno-lint",
        "code": "no-unused-vars", "message": "Variable 'arg' is declared but never used"
    });
    let spacing = json!({
        "range": range((3, 9), (3, 10)), "severity": 2, "source": "pleno-lint",
        "code": "operator-spacing", "message": "Missing spacing around operator '='"
    });
    let mut ids = Vec::new();
    for diagnostic in [&unused_import, &unused_let, &unused_param, &spacing] {
        ids.push(session.request(
            "textDocument/codeAction",
            json!({
                "textDocument": { "uri": uri },
                "range": diagnostic["range"],
                "context": { "diagnostics": [diagnostic] }
            }),
        ));
    }
    let output = session.run();

    // The published diagnostics match the ones built above.
    let published = output.diagnostics(&uri);
    for expected in [&unused_import, &unused_let, &unused_param, &spacing] {
        assert!(
            published
                .iter()
                .any(|d| d["range"] == expected["range"] && d["code"] == expected["code"]),
            "{expected} not in {published:?}"
        );
    }

    let titles_and_edits = |id: i64| -> Vec<(String, Vec<Value>)> {
        output
            .result(id)
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                let edits = a["edit"]["changes"][uri.as_str()]
                    .as_array()
                    .unwrap()
                    .clone();
                (a["title"].as_str().unwrap().to_string(), edits)
            })
            .collect()
    };

    let import = titles_and_edits(ids[0]);
    assert_eq!(import.len(), 1);
    assert_eq!(import[0].0, "Remove unused import 'b'");
    assert_eq!(import[0].1[0]["newText"], "import { a } from \"m\";");

    let local = titles_and_edits(ids[1]);
    assert_eq!(local[0].0, "Remove unused variable 'total'");
    assert_eq!(
        local[0].1,
        [json!({ "range": range((2, 0), (3, 0)), "newText": "" })]
    );
    assert_eq!(local[1].0, "Prefix 'total' with '_' to mark it as unused");
    assert_eq!(
        local[1].1,
        [json!({ "range": range((2, 8), (2, 8)), "newText": "_" })]
    );

    let param = titles_and_edits(ids[2]);
    assert_eq!(param.len(), 1);
    assert_eq!(param[0].0, "Prefix 'arg' with '_' to mark it as unused");

    let spacing = titles_and_edits(ids[3]);
    assert_eq!(spacing[0].0, "Add spaces around '='");
    assert_eq!(
        spacing[0].1,
        [
            json!({ "range": range((3, 9), (3, 9)), "newText": " " }),
            json!({ "range": range((3, 10), (3, 10)), "newText": " " })
        ]
    );
}

#[test]
fn selection_range_and_range_formatting() {
    let workspace = TempWorkspace::new("selection");
    let uri = workspace.uri("main.pln");
    let mut session = Session::new();
    session.open(&uri, "let a   =  1;\nlet b = a + 2;\nprint(b);\n");
    let selection = session.request(
        "textDocument/selectionRange",
        json!({ "textDocument": { "uri": uri }, "positions": [position(1, 8)] }),
    );
    let format = session.request(
        "textDocument/rangeFormatting",
        json!({
            "textDocument": { "uri": uri },
            "range": range((0, 0), (0, 5)),
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    );
    let output = session.run();

    let innermost = &output.result(selection)[0];
    assert_eq!(innermost["range"], range((1, 8), (1, 9)));
    assert_eq!(innermost["parent"]["range"], range((1, 8), (1, 13)));

    assert_eq!(
        *output.result(format),
        json!([{ "range": range((0, 0), (0, 13)), "newText": "let a = 1;" }])
    );
}
