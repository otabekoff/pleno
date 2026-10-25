//! Subcommands reject arguments they do not understand instead of ignoring
//! them, print their own usage for `--help`, and the tools report a version.

use std::path::PathBuf;
use std::process::{Command, Output};

const PLENO: &str = env!("CARGO_BIN_EXE_pleno");

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pleno-cli-args-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn pleno(args: &[&str]) -> Output {
    Command::new(PLENO).args(args).output().unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn unknown_options_and_extra_arguments_are_errors() {
    let dir = temp_dir("unknown");
    let file = dir.join("a.pln");
    std::fs::write(&file, "print(1);\n").unwrap();
    let file = file.to_str().unwrap();

    let bogus = pleno(&["compile", file, "--bogus"]);
    assert_eq!(bogus.status.code(), Some(2));
    assert!(
        stderr(&bogus).contains("'compile' has no option '--bogus'"),
        "{}",
        stderr(&bogus)
    );

    let extra = pleno(&["check", file, "other.pln"]);
    assert_eq!(extra.status.code(), Some(2));
    assert!(stderr(&extra).contains("unexpected argument 'other.pln'"));

    let missing = pleno(&["compile", file, "-o"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(stderr(&missing).contains("'-o' needs a value"));

    let not_pln = pleno(&["build", "README.md"]);
    assert_eq!(not_pln.status.code(), Some(2));
    assert!(stderr(&not_pln).contains("'build' takes a .pln file"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn subcommands_print_their_usage() {
    for command in [
        "compile", "check", "format", "lint", "test", "watch", "doc", "build",
    ] {
        let help = pleno(&[command, "--help"]);
        assert!(help.status.success(), "{command}");
        assert!(
            stdout(&help).starts_with(&format!("Usage: pleno {command}")),
            "{command}"
        );
    }
}

#[test]
fn compile_writes_where_output_says() {
    let dir = temp_dir("output");
    let file = dir.join("a.pln");
    std::fs::write(&file, "print(\"compiled\");\n").unwrap();
    let out = dir.join("nested/dir/app.plc");
    let compile = pleno(&[
        "compile",
        file.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(compile.status.success(), "{}", stderr(&compile));
    let run = pleno(&["run", out.to_str().unwrap()]);
    assert_eq!(stdout(&run).trim(), "compiled");
    assert!(!dir.join("a.plc").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_tool_reports_its_version() {
    let version = env!("CARGO_PKG_VERSION");
    let bin = PathBuf::from(PLENO);
    let dir = bin.parent().unwrap();
    for tool in ["pleno", "pleno-dap", "pleno-repl", "pleno-lsp"] {
        let exe = dir.join(format!("{tool}{}", std::env::consts::EXE_SUFFIX));
        if !exe.exists() {
            continue; // Built by another package's tests; checked when present.
        }
        let output = Command::new(&exe).arg("--version").output().unwrap();
        let text = stdout(&output);
        assert!(text.contains(version), "{tool}: {text}");
    }
}
