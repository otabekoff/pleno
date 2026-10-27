//! `plm add`, `install`, `tree`, `update` and `remove` against a local bare
//! Git repository (a `file://` URL, no network), and importing the installed
//! package from Pleno code.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PLENO: &str = env!("CARGO_BIN_EXE_pleno");
const PLM: &str = env!("CARGO_BIN_EXE_plm");

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pleno-cli-pkg-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn git(dir: &Path, args: &[&str]) {
    // Tests can run inside a git hook, where GIT_DIR and friends point at
    // this repository; the test repositories must not see them.
    let output = Command::new("git")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_PREFIX")
        .current_dir(dir)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
            "-c",
            "core.autocrlf=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn release(work: &Path, version: &str) {
    std::fs::write(
        work.join("pleno.toml"),
        format!("[package]\nname = \"greet\"\nversion = \"{version}\"\nmain = \"lib.pln\"\n"),
    )
    .unwrap();
    std::fs::write(
        work.join("lib.pln"),
        format!("export fn hello(name) {{ return \"Hello, \" + name + \" from {version}\"; }}\n"),
    )
    .unwrap();
    git(work, &["add", "-A"]);
    git(work, &["commit", "--quiet", "-m", version]);
    git(work, &["tag", &format!("v{version}")]);
    git(work, &["push", "--quiet", "origin", "main", "--tags"]);
}

fn run(exe: &str, dir: &Path, cache: &Path, args: &[&str]) -> Output {
    Command::new(exe)
        .current_dir(dir)
        .env("PLENO_CACHE_DIR", cache)
        .env("NO_COLOR", "1")
        .args(args)
        .output()
        .unwrap()
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn plm_adds_installs_and_imports_a_git_package() {
    let base = temp_dir("flow");
    let bare = base.join("greet.git");
    let work = base.join("greet");
    std::fs::create_dir_all(&bare).unwrap();
    std::fs::create_dir_all(&work).unwrap();
    git(&bare, &["init", "--quiet", "--bare"]);
    git(&work, &["init", "--quiet", "-b", "main"]);
    let path = bare.to_string_lossy().replace('\\', "/");
    let url = if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    };
    git(&work, &["remote", "add", "origin", &url]);
    release(&work, "1.0.0");

    let app = base.join("app");
    let cache = base.join("cache");
    std::fs::create_dir_all(&app).unwrap();
    std::fs::write(
        app.join("pleno.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\n",
    )
    .unwrap();
    std::fs::write(
        app.join("main.pln"),
        "import { hello } from \"greet\";\nprint(hello(\"Ada\"));\n",
    )
    .unwrap();

    let add = run(PLM, &app, &cache, &["add", &format!("{url}@^1")]);
    assert!(add.status.success(), "{}", text(&add));
    assert!(text(&add).contains("Added greet"), "{}", text(&add));
    assert!(app.join("pleno.lock").is_file());

    let ran = run(PLENO, &app, &cache, &["run", "main.pln"]);
    assert!(ran.status.success(), "{}", text(&ran));
    assert!(
        text(&ran).contains("Hello, Ada from 1.0.0"),
        "{}",
        text(&ran)
    );

    let tree = run(PLM, &app, &cache, &["tree"]);
    assert!(text(&tree).contains("└── greet v1.0.0"), "{}", text(&tree));

    // A fresh checkout installs offline from the lockfile and the cache.
    std::fs::remove_dir_all(app.join("packages")).unwrap();
    let offline = run(PLM, &app, &cache, &["install", "--offline"]);
    assert!(offline.status.success(), "{}", text(&offline));

    release(&work, "1.1.0");
    let update = run(PLENO, &app, &cache, &["plm", "update", "greet"]);
    assert!(update.status.success(), "{}", text(&update));
    assert!(
        text(&update).contains("greet v1.0.0 -> v1.1.0"),
        "{}",
        text(&update)
    );

    let remove = run(PLM, &app, &cache, &["remove", "greet"]);
    assert!(remove.status.success(), "{}", text(&remove));
    assert!(!app.join("packages/greet").exists());

    // A bare name is not a source.
    let bad = run(PLM, &app, &cache, &["add", "greet"]);
    assert_eq!(bad.status.code(), Some(2));
    assert!(text(&bad).contains("no central registry"), "{}", text(&bad));

    let search = run(PLM, &app, &cache, &["search", "json"]);
    assert!(
        text(&search).contains("topic%3Apleno-package"),
        "{}",
        text(&search)
    );
    let _ = std::fs::remove_dir_all(&base);
}
