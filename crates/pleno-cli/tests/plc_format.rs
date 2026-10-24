//! `pleno compile` writes a versioned `.plc` file and `pleno run` checks it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PLENO: &str = env!("CARGO_BIN_EXE_pleno");

/// A fresh directory for one test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("pleno-plc-format-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self, file: &str) -> PathBuf {
        self.0.join(file)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn pleno(dir: &Path, args: &[&str]) -> Output {
    Command::new(PLENO)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

fn compile_sample(dir: &TempDir) -> Vec<u8> {
    std::fs::write(
        dir.path("main.pln"),
        "fn add(a, b) { return a + b; }\nprint(\"sum\", add(2, 3));\n",
    )
    .unwrap();
    let out = pleno(&dir.0, &["compile", "main.pln"]);
    assert!(out.status.success(), "{out:?}");
    std::fs::read(dir.path("main.plc")).unwrap()
}

fn run_plc(dir: &TempDir, bytes: &[u8]) -> Output {
    std::fs::write(dir.path("test.plc"), bytes).unwrap();
    pleno(&dir.0, &["run", "test.plc"])
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn compile_then_run() {
    let dir = TempDir::new("round-trip");
    let bytes = compile_sample(&dir);
    assert_eq!(&bytes[..4], b"PLNC");
    assert_eq!(
        &bytes[4..8],
        &pleno_bytecode::BYTECODE_FORMAT_VERSION.to_le_bytes()
    );

    let out = pleno(&dir.0, &["run", "main.plc"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "sum 5");
}

#[test]
fn headerless_file_is_not_bytecode() {
    let dir = TempDir::new("magic");
    let bytes = compile_sample(&dir);
    // What an older pleno wrote: the payload with no header.
    let out = run_plc(&dir, &bytes[8..]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("not a Pleno bytecode file"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn other_format_version_is_rejected() {
    let dir = TempDir::new("version");
    let mut bytes = compile_sample(&dir);
    let other = pleno_bytecode::BYTECODE_FORMAT_VERSION + 1;
    bytes[4..8].copy_from_slice(&other.to_le_bytes());
    let out = run_plc(&dir, &bytes);
    assert!(!out.status.success());
    let expected = format!(
        "this .plc file was compiled by a different Pleno version (bytecode format {other}, \
         this pleno reads {}); recompile it with `pleno compile`",
        pleno_bytecode::BYTECODE_FORMAT_VERSION
    );
    assert!(stderr(&out).contains(&expected), "{}", stderr(&out));
    assert!(!stderr(&out).contains("panicked"), "{}", stderr(&out));
}

#[test]
fn truncated_file_is_rejected() {
    let dir = TempDir::new("truncated");
    let bytes = compile_sample(&dir);
    for len in [6, 8, bytes.len() / 2, bytes.len() - 1] {
        let out = run_plc(&dir, &bytes[..len]);
        assert!(!out.status.success(), "length {len}");
        assert!(stderr(&out).contains("truncated"), "{}", stderr(&out));
        assert!(!stderr(&out).contains("panicked"), "{}", stderr(&out));
    }
}
