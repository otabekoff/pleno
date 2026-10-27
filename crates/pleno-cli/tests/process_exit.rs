//! `exit(code)` ends the program with that status, through `catch` blocks,
//! callbacks and `async` functions, and output printed before it is kept. In a
//! `Sandbox` it ends only the sandboxed function: the host process survives.

use std::process::Command;

const PLENO: &str = env!("CARGO_BIN_EXE_pleno");

fn run(name: &str, source: &str) -> (i32, String) {
    let dir = std::env::temp_dir().join(format!("pleno-exit-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("main.pln");
    std::fs::write(&file, source).unwrap();
    let output = Command::new(PLENO).arg("run").arg(&file).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
    )
}

const IMPORT: &str = "import { exit } from \"std:process\";\n";

#[test]
fn exit_sets_the_status_and_cannot_be_caught() {
    let (code, out) = run(
        "catch",
        &format!(
            "{IMPORT}print(\"before\");\n\
             try {{ exit(3); }} catch (e) {{ print(\"caught\"); }}\n\
             print(\"after\");\n"
        ),
    );
    assert_eq!((code, out.as_str()), (3, "before\n"));
}

#[test]
fn exit_from_a_callback_or_async_function() {
    let (code, _) = run(
        "callback",
        &format!("{IMPORT}[1].map(fn(x) {{ exit(7); }});\n"),
    );
    assert_eq!(code, 7);
    let (code, _) = run(
        "async",
        &format!("{IMPORT}async fn g() {{ exit(5); }}\nawait g();\nprint(\"never\");\n"),
    );
    assert_eq!(code, 5);
    let (code, out) = run(
        "zero",
        &format!("{IMPORT}print(\"ok\");\nexit(0);\nprint(\"never\");\n"),
    );
    assert_eq!((code, out.as_str()), (0, "ok\n"));
}

#[test]
fn exit_in_a_sandbox_ends_only_the_sandbox() {
    // Before, it ended the whole process: a sandboxed program could stop its host.
    let (code, out) = run(
        "sandbox",
        &format!(
            "{IMPORT}import {{ Sandbox }} from \"std:security\";\n\
             let box = Sandbox({{}});\n\
             try {{ box.run(fn() {{ exit(9); }}); }} catch (e) {{ print(e.message); }}\n\
             print(\"host continues\");\n"
        ),
    );
    assert_eq!(
        (code, out.as_str()),
        (0, "program exited with code 9\nhost continues\n")
    );
}
