//! `pleno-lsp --version` names the tool and its version, and `--help` exits.

use std::process::Command;

const EXE: &str = env!("CARGO_BIN_EXE_pleno-lsp");

#[test]
fn reports_its_version_and_help() {
    let output = Command::new(EXE).arg("--version").output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        text.trim(),
        concat!("pleno-lsp ", env!("CARGO_PKG_VERSION"))
    );
    let help = Command::new(EXE).arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--version"));
}
