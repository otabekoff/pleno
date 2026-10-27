//! Subcommands reject arguments they do not understand instead of ignoring
//! them, print their own usage for `--help`, and report their version.

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
fn pleno_and_plm_report_their_version() {
    let version = env!("CARGO_PKG_VERSION");
    for exe in [PLENO, env!("CARGO_BIN_EXE_plm")] {
        let output = Command::new(exe).arg("--version").output().unwrap();
        assert!(
            stdout(&output).contains(version),
            "{exe}: {}",
            stdout(&output)
        );
    }
}

#[test]
fn new_creates_a_package_in_a_new_folder() {
    let dir = temp_dir("new");
    let run_in = |args: &[&str]| {
        Command::new(PLENO)
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap()
    };
    let created = run_in(&["new", "apps/hello"]);
    assert!(created.status.success(), "{}", stderr(&created));
    let manifest = std::fs::read_to_string(dir.join("apps/hello/pleno.toml")).unwrap();
    assert!(manifest.contains("name = \"hello\""), "{manifest}");
    assert!(manifest.contains("version = \"0.1.0\""), "{manifest}");
    assert!(dir.join("apps/hello/src/main.pln").exists());

    // A folder with files in it is not overwritten.
    let again = run_in(&["new", "apps/hello"]);
    assert_eq!(again.status.code(), Some(1));
    assert!(
        stderr(&again).contains("already exists"),
        "{}",
        stderr(&again)
    );

    let named = run_in(&["new", "lib", "--name", "cool-lib"]);
    assert!(named.status.success(), "{}", stderr(&named));
    let manifest = std::fs::read_to_string(dir.join("lib/pleno.toml")).unwrap();
    assert!(manifest.contains("name = \"cool-lib\""), "{manifest}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unknown_command_is_named_with_a_suggestion() {
    let out = pleno(&["chek"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("unknown command 'chek'; did you mean 'check'?"),
        "{}",
        stderr(&out)
    );
    let out = pleno(&["tree"]);
    assert!(
        stderr(&out).contains("did you mean 'pleno plm tree'?"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn a_file_starting_with_a_byte_order_mark_runs() {
    let dir = temp_dir("bom");
    let file = dir.join("bom.pln");
    std::fs::write(&file, b"\xef\xbb\xbfprint(\"bom\");\n").unwrap();
    let out = pleno(&["run", file.to_str().unwrap()]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim(), "bom");
}
