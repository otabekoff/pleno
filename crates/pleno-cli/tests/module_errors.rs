//! An error in an imported module is reported in that module: its file, line
//! and source line, in `pleno run`'s message and backtrace and in `pleno test`.
//! Positions in a module used to be read against the main file's text, and a
//! module that did not parse was reported at the `import` that loaded it.

use std::path::PathBuf;
use std::process::{Command, Output};

const PLENO: &str = env!("CARGO_BIN_EXE_pleno");

/// A folder with the given files, which `pleno` runs from.
fn project(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("pleno-module-errors-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir
}

fn pleno(dir: &PathBuf, args: &[&str]) -> (Output, String) {
    let out = Command::new(PLENO)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    let text =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    (out, text)
}

const LIB: &str = "// helper module\nexport fn half(items) {\n    let middle = items.length / 2;\n    return items[..middle];\n}\n";
const MAIN: &str = "// the main file\n// line two\nimport { half } from \"lib.pln\";\n\nfn main() {\n    print(half([1, 2, 3, 4]));\n    return 1;\n}\n\nmain();\n";

#[test]
fn a_runtime_error_in_a_module_points_into_the_module() {
    let dir = project("runtime", &[("lib.pln", LIB), ("main.pln", MAIN)]);
    let (out, text) = pleno(&dir, &["main.pln"]);
    assert!(!out.status.success());
    assert!(text.contains("--> lib.pln:4:20"), "{text}");
    assert!(text.contains("return items[..middle];"), "{text}");
    assert!(text.contains("at half (lib.pln:4:20)"), "{text}");
    // The caller's frame stays in the main file.
    assert!(text.contains("at main (6:"), "{text}");
    assert!(!text.contains("main.pln:6:22"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_module_that_does_not_parse_is_reported_where_it_fails() {
    let dir = project(
        "parse",
        &[
            ("bad.pln", "export fn broken( {\n"),
            (
                "main.pln",
                "// a comment\n// another\nimport { broken } from \"bad.pln\";\n",
            ),
        ],
    );
    let (out, text) = pleno(&dir, &["main.pln"]);
    assert!(!out.status.success());
    assert!(text.contains("Expected parameter name"), "{text}");
    assert!(text.contains("--> bad.pln:1:19"), "{text}");
    assert!(text.contains("export fn broken( {"), "{text}");
    assert!(!text.contains("main.pln:3:1"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pleno_test_names_the_place_of_a_runtime_error() {
    let dir = project(
        "test",
        &[
            ("lib.pln", LIB),
            ("bad.pln", "export fn broken( {\n"),
            (
                "half_test.pln",
                "import { half } from \"lib.pln\";\nhalf([1, 2, 3]);\n",
            ),
            ("bad_test.pln", "import { broken } from \"bad.pln\";\n"),
        ],
    );
    let (_, text) = pleno(&dir, &["test", "half_test.pln"]);
    assert!(text.contains("--> lib.pln:4:20"), "{text}");
    let (_, text) = pleno(&dir, &["test", "bad_test.pln"]);
    assert!(text.contains("--> bad.pln:1:19"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}
