//! The `with` statement: resources are closed however the body ends.

use pleno_compiler::Compiler;
use pleno_vm::VM;

fn eval_result(source: &str) -> Result<pleno_value::Value, String> {
    let tokens = pleno_lexer::lex(source).map_err(|e| e.message)?;
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let mut program = parser.parse_program().map_err(|e| e.message)?;
    let mut compiler = Compiler::new(source.to_string());
    let chunk = compiler
        .compile(&mut program)
        .map_err(|e| e.diagnostic.message)?;
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);
    vm.execute(chunk).map_err(|e| e.message)
}

/// The printed form of the program's last expression, run after `RES`.
fn eval(source: &str) -> String {
    let source = format!("{RES}\n{source}");
    eval_result(&source)
        .unwrap_or_else(|e| panic!("{source}\nfailed: {e}"))
        .to_string()
}

fn eval_err(source: &str) -> String {
    let source = format!("{RES}\n{source}");
    match eval_result(&source) {
        Ok(value) => panic!("{source}\nexpected an error, got {value}"),
        Err(e) => e,
    }
}

/// A resource that records when it opens and closes in `log`, and whose
/// `close()` throws when asked to.
const RES: &str = r#"
let log = [];
class Res {
    name: String;
    failClose: Bool;
    constructor(name: String, failClose: Bool = false) {
        this.name = name;
        this.failClose = failClose;
        log.push("open " + name);
    }
    fn close() {
        log.push("close " + this.name);
        if (this.failClose) {
            throw Error("close " + this.name + " failed");
        }
    }
}
"#;

#[test]
fn closes_after_the_body() {
    let source = r#"
        with (r = Res("a")) {
            log.push("body " + r.name);
        }
        log;
    "#;
    assert_eq!(eval(source), "[open a, body a, close a]");
}

#[test]
fn several_resources_close_in_reverse_order() {
    let source = r#"
        with (a = Res("a"), b = Res("b"), c = Res("c")) {
            log.push("body");
        }
        log;
    "#;
    assert_eq!(
        eval(source),
        "[open a, open b, open c, body, close c, close b, close a]"
    );
}

#[test]
fn later_resources_see_earlier_names() {
    let source = r#"
        with (a = Res("a"), b = Res(a.name + "2")) {
            log.push(b.name);
        }
        log;
    "#;
    assert_eq!(eval(source), "[open a, open a2, a2, close a2, close a]");
}

#[test]
fn closes_when_the_body_throws_and_the_error_propagates() {
    let source = r#"
        let mut message = "";
        try {
            with (a = Res("a"), b = Res("b")) {
                throw Error("body failed");
            }
        } catch (e) {
            message = e.message;
        }
        (message, log);
    "#;
    assert_eq!(
        eval(source),
        "(body failed, [open a, open b, close b, close a])"
    );
}

#[test]
fn an_uncaught_body_error_reaches_the_top() {
    let source = r#"
        with (a = Res("a")) {
            throw Error("boom");
        }
    "#;
    assert!(eval_err(source).contains("boom"));
}

#[test]
fn body_error_wins_over_close_errors_which_are_suppressed() {
    let source = r#"
        let mut result = null;
        try {
            with (a = Res("a", true), b = Res("b", true)) {
                throw Error("body failed");
            }
        } catch (e) {
            result = (e.message, e.suppressed.map((s) => s.message), log);
        }
        result;
    "#;
    assert_eq!(
        eval(source),
        "(body failed, [close b failed, close a failed], [open a, open b, close b, close a])"
    );
}

#[test]
fn a_close_error_after_a_normal_body_propagates_and_outer_resources_still_close() {
    let source = r#"
        let mut result = null;
        try {
            with (a = Res("a", true), b = Res("b", true)) {
                log.push("body");
            }
        } catch (e) {
            result = (e.message, e.suppressed.map((s) => s.message), log);
        }
        result;
    "#;
    assert_eq!(
        eval(source),
        "(close b failed, [close a failed], [open a, open b, body, close b, close a])"
    );
}

#[test]
fn a_thrown_non_error_value_propagates_unchanged() {
    let source = r#"
        let mut caught = null;
        try {
            with (a = Res("a", true)) {
                throw "plain";
            }
        } catch (e) {
            caught = e;
        }
        (caught, log);
    "#;
    assert_eq!(eval(source), "(plain, [open a, close a])");
}

#[test]
fn a_failing_resource_closes_the_ones_before_it() {
    let source = r#"
        fn broken() { throw Error("cannot open"); }
        let mut message = "";
        try {
            with (a = Res("a"), b = broken(), c = Res("c")) {
                log.push("body");
            }
        } catch (e) {
            message = e.message;
        }
        (message, log);
    "#;
    assert_eq!(eval(source), "(cannot open, [open a, close a])");
}

#[test]
fn return_closes() {
    let source = r#"
        fn first() {
            with (a = Res("a"), b = Res("b")) {
                return a.name + b.name;
            }
        }
        let value = first();
        (value, log);
    "#;
    assert_eq!(eval(source), "(ab, [open a, open b, close b, close a])");
}

#[test]
fn break_and_continue_close() {
    let source = r#"
        for (i in [1, 2, 3]) {
            with (r = Res(i.toString())) {
                if (i == 1) { continue; }
                if (i == 2) { break; }
                log.push("never");
            }
        }
        log;
    "#;
    assert_eq!(eval(source), "[open 1, close 1, open 2, close 2]");
}

#[test]
fn labeled_continue_closes_every_resource_it_leaves() {
    let source = r#"
        outer: for (i in [1, 2]) {
            with (a = Res("a" + i.toString())) {
                for (j in [1, 2]) {
                    with (b = Res("b" + j.toString())) {
                        continue outer;
                    }
                }
            }
        }
        log;
    "#;
    assert_eq!(
        eval(source),
        "[open a1, open b1, close b1, close a1, open a2, open b1, close b1, close a2]"
    );
}

#[test]
fn a_loop_inside_the_body_does_not_close_early() {
    let source = r#"
        with (r = Res("r")) {
            for (i in [1, 2]) {
                if (i == 1) { continue; }
                log.push("i" + i.toString());
                break;
            }
            log.push("after loop");
        }
        log;
    "#;
    assert_eq!(eval(source), "[open r, i2, after loop, close r]");
}

#[test]
fn nested_with_statements_close_inner_first() {
    let source = r#"
        with (o = Res("outer")) {
            with (i = Res("inner")) {
                log.push(o.name + "+" + i.name);
            }
            log.push("between");
        }
        log;
    "#;
    assert_eq!(
        eval(source),
        "[open outer, open inner, outer+inner, close inner, between, close outer]"
    );
}

#[test]
fn closures_in_the_body_can_capture_the_resource() {
    let source = r#"
        let mut f = null;
        with (r = Res("r")) {
            f = () => r.name;
            log.push(f());
        }
        (f(), log);
    "#;
    assert_eq!(eval(source), "(r, [open r, r, close r])");
}

#[test]
fn try_finally_inside_with_runs_before_close() {
    let source = r#"
        fn f() {
            with (r = Res("r")) {
                try {
                    return 1;
                } finally {
                    log.push("finally");
                }
            }
        }
        (f(), log);
    "#;
    assert_eq!(eval(source), "(1, [open r, finally, close r])");
}

#[test]
fn the_names_are_not_visible_after_the_statement() {
    let err = eval_err(
        r#"
        with (r = Res("r")) {}
        r;
    "#,
    );
    assert!(err.contains("'r'"), "{err}");
}

#[test]
fn the_names_cannot_be_reassigned() {
    let err = eval_err(
        r#"
        with (r = Res("r")) {
            r = Res("other");
        }
    "#,
    );
    assert!(err.contains("immutable"), "{err}");
}

#[test]
fn a_value_without_close_is_an_error_before_the_body_runs() {
    let source = r#"
        let mut out = [];
        for (value in [5, "text", {"a": 1}, [1], null]) {
            try {
                with (x = value) {
                    out.push("body ran");
                }
            } catch (e) {
                out.push(e.name + ": " + e.message);
            }
        }
        out;
    "#;
    assert_eq!(
        eval(source),
        "[TypeError: 'with' needs a value with a close() method, got Int, \
         TypeError: 'with' needs a value with a close() method, got String, \
         TypeError: 'with' needs a value with a close() method, got Map, \
         TypeError: 'with' needs a value with a close() method, got Array, \
         TypeError: 'with' needs a value with a close() method, got Null]"
    );
}

#[test]
fn a_map_with_a_close_function_is_a_resource() {
    let source = r#"
        with (m = {"close": () => log.push("map closed")}) {
            log.push("body");
        }
        log;
    "#;
    assert_eq!(eval(source), "[body, map closed]");
}

#[test]
fn files_close() {
    let path = std::env::temp_dir().join(format!("pleno_with_{}.txt", std::process::id()));
    let path = path.to_string_lossy().replace('\\', "/");
    let source = format!(
        r#"
        import {{ File }} from "std:io";
        with (out = File.open("{path}", "w")) {{
            out.write("hello");
        }}
        let mut kept = null;
        let mut text = "";
        with (input = File.open("{path}", "r")) {{
            text = input.read();
            kept = input;
        }}
        let mut after = "";
        try {{ kept.read(); }} catch (e) {{ after = e.message; }}
        File.delete("{path}");
        (text, after);
    "#
    );
    assert_eq!(
        eval(&source),
        "(hello, file is closed or not open for reading)"
    );
}

#[test]
fn databases_close() {
    let source = r#"
        import { Database } from "std:db";
        let mut kept = null;
        with (db = Database.connect("sqlite::memory:")) {
            db.execute("CREATE TABLE t (x INTEGER)");
            db.execute("INSERT INTO t (x) VALUES (?)", [7]);
            log.push(db.query("SELECT x FROM t")[0]["x"]);
            kept = db;
        }
        let mut after = "";
        try { kept.query("SELECT 1"); } catch (e) { after = "closed"; }
        (log, after);
    "#;
    assert_eq!(eval(source), "([7], closed)");
}

#[test]
fn async_functions_close_across_await() {
    let source = r#"
        async fn work() {
            with (r = Res("r")) {
                let v = await Promise.resolve(r.name);
                log.push("got " + v);
                return v;
            }
        }
        work();
        log;
    "#;
    let out = eval(source);
    assert!(out.starts_with("[open r"), "{out}");
}

#[test]
fn break_out_of_try_pops_its_handler() {
    // Before `with`, `break` out of a `try` left its handler in place, so a
    // later error jumped back into the old `catch`.
    let source = r#"
        let mut hits = 0;
        fn run() {
            for (i in [1]) {
                try {
                    break;
                } catch (e) {
                    hits = hits + 1;
                }
            }
            throw Error("later");
        }
        let mut message = "";
        try { run(); } catch (e) { message = e.message; }
        (hits, message);
    "#;
    assert_eq!(eval(source), "(0, later)");
}

#[test]
fn break_and_continue_run_finally() {
    let source = r#"
        for (i in [1, 2, 3]) {
            try {
                if (i == 1) { continue; }
                if (i == 2) { break; }
            } finally {
                log.push("finally " + i.toString());
            }
        }
        log;
    "#;
    assert_eq!(eval(source), "[finally 1, finally 2]");
}

#[test]
fn inline_functions_close_and_return_their_value() {
    let source = r#"
        @inline
        fn pick(x: Int): Int {
            if (x > 0) {
                return 1;
            }
            return 2;
        }
        @inline
        fn opened(): String {
            with (r = Res("r")) {
                return r.name;
            }
        }
        (pick(5), pick(-5), opened(), log);
    "#;
    assert_eq!(eval(source), "(1, 2, r, [open r, close r])");
}
