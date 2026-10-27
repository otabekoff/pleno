//! `pleno check` ends its messages about misspelled names with "did you mean
//! ...?" when a close name is in scope.

use std::path::PathBuf;
use std::process::Command;

const PLENO: &str = env!("CARGO_BIN_EXE_pleno");

/// What `pleno check` prints for `source`, written to a file named `name`.
fn check(name: &str, source: &str) -> String {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("pleno-check-hints-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("main.pln");
    std::fs::write(&file, source).unwrap();
    let output = Command::new(PLENO)
        .args(["check", file.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_reports(name: &str, source: &str, expected: &str) {
    let output = check(name, source);
    assert!(
        output.contains(expected),
        "expected {expected:?} in:\n{output}"
    );
}

#[test]
fn undefined_names_suggest_what_is_in_scope() {
    assert_reports(
        "local",
        "let length = 3;\nprint(lenght);\n",
        "main.pln:2:7: error: undefined variable 'lenght'; did you mean 'length'?",
    );
    assert_reports(
        "function",
        "fn helper() { return 1; }\nhelpr();\n",
        "error: undefined variable 'helpr'; did you mean 'helper'?",
    );
    assert_reports(
        "builtin",
        "pirnt(1);\n",
        "error: undefined variable 'pirnt'; did you mean 'print'?",
    );
    assert_reports(
        "param",
        "fn f(items) { return itmes; }\nf(1);\n",
        "error: undefined variable 'itmes'; did you mean 'items'?",
    );
    // A closure sees the enclosing locals.
    assert_reports(
        "capture",
        "let total = 1;\nfn f() { return () => totl; }\nf()();\n",
        "error: undefined variable 'totl'; did you mean 'total'?",
    );
    assert_reports(
        "import",
        "import { JSON } from \"std:json\";\nprint(JSNO);\n",
        "error: undefined variable 'JSNO'; did you mean 'JSON'?",
    );
}

#[test]
fn locals_out_of_scope_are_not_suggested() {
    let output = check(
        "scope",
        "{ let length = 1; print(length); }\nprint(lenght);\n",
    );
    assert!(output.contains("undefined variable 'lenght'"), "{output}");
    assert!(!output.contains("did you mean"), "{output}");
}

#[test]
fn unknown_named_arguments_and_methods_on_this() {
    assert_reports(
        "named",
        "fn f(name: String) { print(name); }\nf(nmae: \"x\");\n",
        "error: f has no parameter named 'nmae'; did you mean 'name'?",
    );
    assert_reports(
        "method",
        "class A { fn go() { return 1; } fn run() { return this.og(); } }\nprint(A().run());\n",
        "warning: A has no method 'og'; did you mean 'go'?",
    );
    // A superclass's private method is not suggested in a subclass.
    let output = check(
        "private",
        "class A { private fn hidden() { return 1; } }\n\
         class B extends A { fn peek() { return this.hiden(); } }\nprint(B().peek());\n",
    );
    assert!(output.contains("B has no method 'hiden'"), "{output}");
    assert!(!output.contains("did you mean"), "{output}");
}

#[test]
fn std_imports() {
    assert_reports(
        "module",
        "import { parse } from \"std:jsn\";\nprint(parse);\n",
        "main.pln:1:1: error: std module not found: jsn; did you mean 'std:json'?",
    );
    assert_reports(
        "export",
        "import { JSN } from \"std:json\";\nprint(JSN);\n",
        "main.pln:1:1: warning: 'std:json' does not export 'JSN'; did you mean 'JSON'?",
    );
    let output = check("fine", "import { JSON } from \"std:json\";\nprint(JSON);\n");
    assert!(
        !output.contains("error") && !output.contains("warning"),
        "{output}"
    );
}

/// What `pleno check` prints for `main.pln` holding `source`, next to `others`.
fn check_with(name: &str, source: &str, others: &[(&str, &str)]) -> String {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("pleno-check-files-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in others {
        std::fs::write(dir.join(file), text).unwrap();
    }
    let file = dir.join("main.pln");
    std::fs::write(&file, source).unwrap();
    let output = Command::new(PLENO)
        .args(["check", file.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn names_and_calls_from_other_files() {
    let module = (
        "m.pln",
        "export fn helper(a) { return a; }\nexport let name = 1;\n",
    );
    let output = check_with(
        "imports",
        "import { helper, nmae } from \"./m.pln\";\nprint(helper(1, 2), nmae);\n",
        &[module],
    );
    for expected in [
        "main.pln:1:1: warning: './m.pln' does not export 'nmae'; did you mean 'name'?",
        "main.pln:2:7: error: helper expects 1 argument, got 2",
    ] {
        assert!(
            output.contains(expected),
            "expected {expected:?} in:\n{output}"
        );
    }
    let output = check_with(
        "imports-fine",
        "import { helper, name } from \"./m.pln\";\nprint(helper(name));\n",
        &[module],
    );
    assert!(output.contains("Type check passed"), "{output}");
}

#[test]
fn a_file_that_exports_is_checked_as_a_module() {
    let output = check_with(
        "module",
        "module shapes;\nexport fn area(w, h) { return w * h; }\nexport { area as size };\n",
        &[],
    );
    assert!(output.contains("Type check passed"), "{output}");
}

#[test]
fn calls_on_known_classes_and_lambdas() {
    let output = check_with(
        "classes",
        "class Point {\n  fn shift(dx) { return dx; }\n  static origin() { return Point(); }\n}\n\
         let p = Point();\np.shfit(1);\nPoint.origin(1);\nlet twice = (x) => x * 2;\nprint(twice(1, 2));\n",
        &[],
    );
    for expected in [
        "main.pln:6:1: warning: Point has no method 'shfit'; did you mean 'shift'?",
        "main.pln:7:1: error: Point.origin expects no arguments, got 1",
        "main.pln:9:7: error: twice expects 1 argument, got 2",
    ] {
        assert!(
            output.contains(expected),
            "expected {expected:?} in:\n{output}"
        );
    }
}

#[test]
fn a_compile_error_does_not_hide_the_other_problems() {
    // Before, `pleno check` stopped at the compile error and reported nothing else.
    let out = check(
        "compile-error-and-more",
        "fn helper(a) { return a; }\nhelper(1, 2);\nlet total = 1;\ntotal = 2;\n",
    );
    assert!(out.contains("helper expects 1 argument, got 2"), "{out}");
    assert!(out.contains("cannot assign to immutable binding"), "{out}");
}
