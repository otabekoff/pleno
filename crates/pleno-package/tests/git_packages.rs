//! Git dependencies end to end, with local bare repositories served over
//! `file://` URLs (no network).

use pleno_package::{
    AddRequest, Change, DepSource, PackageError, PackageManager, Want, checksum_dir, publish,
};
use semver::VersionReq;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "pleno-gitpkg-{name}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn git(dir: &Path, args: &[&str]) -> String {
    let mut cmd = Command::new("git");
    // Tests can run inside a git hook, where GIT_DIR and friends point at
    // this repository; the test repositories must not see them.
    for variable in pleno_package::git::REPOSITORY_VARIABLES {
        cmd.env_remove(variable);
    }
    let output = cmd
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
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn file_url(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.starts_with('/') {
        format!("file://{text}")
    } else {
        format!("file:///{text}")
    }
}

/// A package repository: a work tree that pushes to a bare repository.
struct Repo {
    work: PathBuf,
    url: String,
}

impl Repo {
    fn new(base: &Path, name: &str) -> Self {
        let bare = base.join("remotes").join(format!("{name}.git"));
        let work = base.join("work").join(name);
        std::fs::create_dir_all(&bare).unwrap();
        std::fs::create_dir_all(&work).unwrap();
        git(&bare, &["init", "--quiet", "--bare"]);
        git(&work, &["init", "--quiet", "-b", "main"]);
        // `plm publish --push` runs plain `git tag -a`, which needs these.
        git(&work, &["config", "user.name", "Test"]);
        git(&work, &["config", "user.email", "test@example.com"]);
        git(&work, &["config", "tag.gpgsign", "false"]);
        let url = file_url(&bare);
        git(&work, &["remote", "add", "origin", &url]);
        Repo { work, url }
    }

    /// Commits a package version with these dependencies and tags it.
    fn release(&self, name: &str, version: &str, deps: &str) -> String {
        self.commit(name, version, deps);
        git(&self.work, &["tag", &format!("v{version}")]);
        self.push();
        git(&self.work, &["rev-parse", "HEAD"])
    }

    fn commit(&self, name: &str, version: &str, deps: &str) -> String {
        std::fs::write(
            self.work.join("pleno.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"{version}\"\nmain = \"lib.pln\"\n\n[dependencies]\n{deps}"
            ),
        )
        .unwrap();
        std::fs::write(
            self.work.join("lib.pln"),
            format!("export fn version() {{ return \"{version}\"; }}\n"),
        )
        .unwrap();
        git(&self.work, &["add", "-A"]);
        git(&self.work, &["commit", "--quiet", "-m", version]);
        self.push();
        git(&self.work, &["rev-parse", "HEAD"])
    }

    fn push(&self) {
        git(
            &self.work,
            &["push", "--quiet", "--force", "origin", "main"],
        );
        git(
            &self.work,
            &["push", "--quiet", "--force", "origin", "--tags"],
        );
    }
}

/// A project folder with its own cache.
fn project(base: &Path, deps: &str) -> PackageManager {
    let root = base.join("app");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("pleno.toml"),
        format!(
            "# my app\n[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\n{deps}"
        ),
    )
    .unwrap();
    PackageManager::new(&root).with_cache_dir(base.join("cache"))
}

fn dep(name: &str, url: &str, rest: &str) -> String {
    format!("{name} = {{ git = \"{url}\"{rest} }}\n")
}

fn installed_version(manager: &PackageManager, name: &str) -> String {
    std::fs::read_to_string(manager.packages_dir().join(name).join("lib.pln")).unwrap()
}

fn locked<'a>(report: &'a pleno_package::Report, name: &str) -> &'a pleno_package::LockedPackage {
    report.lockfile.get(name).unwrap()
}

#[test]
fn add_picks_the_newest_tag_and_locks_commit_and_checksum() {
    let base = temp("add");
    let greet = Repo::new(&base, "greet");
    greet.release("greet", "1.0.0", "");
    let v11 = greet.release("greet", "1.1.0", "");
    greet.release("greet", "2.0.0-beta.1", "");
    let manager = project(&base, "");

    let (name, report) = manager
        .add(&AddRequest {
            name: None,
            source: DepSource::Git(greet.url.clone()),
            want: None,
            version_text: None,
            dev: false,
        })
        .unwrap();
    assert_eq!(name, "greet");
    // The pre-release is not a release.
    let manifest = std::fs::read_to_string(manager.manifest_path()).unwrap();
    assert!(manifest.contains("version = \"^1.1.0\""), "{manifest}");
    assert!(
        manifest.contains("# my app"),
        "comments are kept: {manifest}"
    );
    let entry = locked(&report, "greet");
    assert_eq!(entry.version.as_deref(), Some("1.1.0"));
    assert_eq!(entry.tag.as_deref(), Some("v1.1.0"));
    assert_eq!(entry.commit.as_deref(), Some(v11.as_str()));
    let checksum = entry.checksum.clone().unwrap();
    assert_eq!(
        checksum,
        checksum_dir(&manager.packages_dir().join("greet")).unwrap()
    );
    assert!(installed_version(&manager, "greet").contains("1.1.0"));
    // No Git metadata is copied.
    assert!(!manager.packages_dir().join("greet/.git").exists());
    let lock = std::fs::read_to_string(manager.lock_path()).unwrap();
    assert!(lock.contains("version = 1"), "{lock}");
    assert!(lock.contains(&format!("commit = \"{v11}\"")), "{lock}");
    assert!(lock.contains("checksum = \"sha256:"), "{lock}");
    assert!(matches!(&report.changes[..], [Change::Added { .. }]));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn install_keeps_the_lock_update_moves_it_and_offline_uses_the_cache() {
    let base = temp("lock");
    let greet = Repo::new(&base, "greet");
    greet.release("greet", "1.0.0", "");
    let manager = project(&base, &dep("greet", &greet.url, ", version = \"^1.0\""));
    manager.install(&[]).unwrap();

    // A newer release does not change an install with a lockfile.
    greet.release("greet", "1.3.0", "");
    let report = manager.install(&[]).unwrap();
    assert_eq!(locked(&report, "greet").version.as_deref(), Some("1.0.0"));
    assert!(report.changes.is_empty());

    // With the cache, a fresh checkout installs offline.
    std::fs::remove_dir_all(manager.packages_dir()).unwrap();
    let offline = PackageManager::new(&manager.root)
        .with_cache_dir(manager.cache.root.clone())
        .with_offline(true);
    offline.install(&[]).unwrap();
    assert!(installed_version(&manager, "greet").contains("1.0.0"));

    // Offline without the cache fails and says why.
    let empty_cache = PackageManager::new(&manager.root)
        .with_cache_dir(base.join("other-cache"))
        .with_offline(true);
    std::fs::remove_dir_all(manager.packages_dir()).unwrap();
    let err = empty_cache.install(&[]).unwrap_err();
    assert!(matches!(err, PackageError::Offline(_)), "{err}");
    assert!(err.to_string().contains("not in the cache"), "{err}");

    // Offline update cannot list tags.
    let err = offline.update(&[]).unwrap_err();
    assert!(err.to_string().contains("--offline"), "{err}");

    // update chooses again.
    let report = manager.update(&[]).unwrap();
    assert_eq!(locked(&report, "greet").version.as_deref(), Some("1.3.0"));
    assert!(
        matches!(&report.changes[..], [Change::Updated { from, to, .. }] if from == "v1.0.0" && to == "v1.3.0"),
        "{:?}",
        report.changes
    );
    assert!(installed_version(&manager, "greet").contains("1.3.0"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_changed_requirement_is_resolved_again_on_install() {
    let base = temp("changed");
    let greet = Repo::new(&base, "greet");
    greet.release("greet", "1.0.0", "");
    greet.release("greet", "2.1.0", "");
    let manager = project(&base, &dep("greet", &greet.url, ", version = \"^1\""));
    manager.install(&[]).unwrap();
    std::fs::write(
        manager.manifest_path(),
        format!(
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\n{}",
            dep("greet", &greet.url, ", version = \"^2\"")
        ),
    )
    .unwrap();
    let report = manager.install(&[]).unwrap();
    assert_eq!(locked(&report, "greet").version.as_deref(), Some("2.1.0"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_tampered_package_fails_the_checksum() {
    let base = temp("tamper");
    let greet = Repo::new(&base, "greet");
    let commit = greet.release("greet", "1.0.0", "");
    let manager = project(&base, &dep("greet", &greet.url, ", version = \"1\""));
    manager.install(&[]).unwrap();

    // Someone changes the cached copy of the commit.
    let cached = manager.cache.commit_dir(&greet.url, &commit);
    std::fs::write(
        cached.join("lib.pln"),
        "export fn version() { return \"evil\"; }\n",
    )
    .unwrap();
    std::fs::remove_dir_all(manager.packages_dir()).unwrap();
    let err = manager.install(&[]).unwrap_err();
    assert!(
        matches!(err, PackageError::ChecksumMismatch { .. }),
        "{err}"
    );
    assert!(
        err.to_string().contains("checksum mismatch for 'greet'"),
        "{err}"
    );
    assert!(!manager.packages_dir().join("greet").exists());

    // An edited lockfile checksum is caught the same way.
    std::fs::remove_dir_all(&cached).unwrap();
    let lock = std::fs::read_to_string(manager.lock_path()).unwrap();
    let start = lock.find("sha256:").unwrap();
    let edited = format!(
        "{}sha256:{}{}",
        &lock[..start],
        "0".repeat(64),
        &lock[start + 71..]
    );
    std::fs::write(manager.lock_path(), edited).unwrap();
    let err = manager.install(&[]).unwrap_err();
    assert!(
        matches!(err, PackageError::ChecksumMismatch { .. }),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_force_pushed_tag_is_refused() {
    let base = temp("moved");
    let greet = Repo::new(&base, "greet");
    let original = greet.release("greet", "1.0.0", "");
    let manager = project(&base, &dep("greet", &greet.url, ", tag = \"v1.0.0\""));
    manager.install(&[]).unwrap();

    // The author moves the tag to another commit.
    greet.commit("greet", "1.0.0-changed", "");
    git(&greet.work, &["tag", "--force", "v1.0.0"]);
    greet.push();

    // install keeps the locked commit, which still exists.
    let report = manager.install(&[]).unwrap();
    assert_eq!(
        locked(&report, "greet").commit.as_deref(),
        Some(original.as_str())
    );

    // Choosing again finds the tag moved, and fails.
    let err = manager.update(&[]).unwrap_err();
    assert!(matches!(err, PackageError::TagMoved { .. }), "{err}");
    assert!(err.to_string().contains("force-pushed"), "{err}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_locked_commit_that_disappeared_is_reported() {
    let base = temp("gone");
    let greet = Repo::new(&base, "greet");
    greet.commit("greet", "0.9.0", "");
    let manager = project(&base, &dep("greet", &greet.url, ", branch = \"main\""));
    manager.install(&[]).unwrap();
    // History is rewritten: the locked commit is gone from the remote.
    git(
        &greet.work,
        &["commit", "--quiet", "--amend", "-m", "rewritten"],
    );
    greet.push();
    let bare = base.join("remotes/greet.git");
    git(&bare, &["reflog", "expire", "--expire=now", "--all"]);
    git(&bare, &["gc", "--quiet", "--prune=now"]);

    let fresh = PackageManager::new(&manager.root).with_cache_dir(base.join("cache2"));
    let err = fresh.install(&[]).unwrap_err();
    assert!(err.to_string().contains("no longer in"), "{err}");
    // update follows the branch to its new commit.
    let report = fresh.update(&["greet".to_string()]).unwrap();
    assert_eq!(locked(&report, "greet").branch.as_deref(), Some("main"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn ranges_choose_the_highest_tag_that_fits_every_requirement() {
    let base = temp("ranges");
    let common = Repo::new(&base, "common");
    for v in ["1.0.0", "1.1.0", "1.2.0", "1.3.0", "2.0.0"] {
        common.release("common", v, "");
    }
    let a = Repo::new(&base, "a");
    a.release(
        "a",
        "1.0.0",
        &dep("common", &common.url, ", version = \"^1.1\""),
    );
    let b = Repo::new(&base, "b");
    b.release(
        "b",
        "1.0.0",
        &dep("common", &common.url, ", version = \">=1.0, <1.3\""),
    );

    let manager = project(
        &base,
        &format!(
            "{}{}",
            dep("a", &a.url, ", version = \"1\""),
            dep("b", &b.url, ", version = \"1\"")
        ),
    );
    let report = manager.install(&[]).unwrap();
    assert_eq!(locked(&report, "common").version.as_deref(), Some("1.2.0"));
    assert_eq!(
        locked(&report, "a").dependencies,
        vec!["common".to_string()]
    );
    assert!(manager.packages_dir().join("common/lib.pln").exists());

    let tree = manager.tree().unwrap();
    assert!(tree.starts_with("app v0.1.0\n"), "{tree}");
    assert!(tree.contains("├── a v1.0.0"), "{tree}");
    assert!(tree.contains("│   └── common v1.2.0"), "{tree}");
    assert!(tree.contains("└── b v1.0.0"), "{tree}");

    // No tag matches.
    let none = project(
        &base.join("none"),
        &dep("common", &common.url, ", version = \"^5\""),
    );
    let err = none.install(&[]).unwrap_err();
    assert!(
        matches!(err, PackageError::NoMatchingVersion { .. }),
        "{err}"
    );
    assert!(err.to_string().contains("v2.0.0, v1.3.0"), "{err}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn transitive_conflicts_are_explained_and_the_root_pin_wins() {
    let base = temp("conflict");
    let common = Repo::new(&base, "common");
    common.release("common", "1.4.0", "");
    common.release("common", "2.0.0", "");
    let a = Repo::new(&base, "a");
    a.release(
        "a",
        "1.0.0",
        &dep("common", &common.url, ", version = \"^1\""),
    );
    let b = Repo::new(&base, "b");
    b.release(
        "b",
        "1.0.0",
        &dep("common", &common.url, ", version = \"^2\""),
    );
    let deps = format!("{}{}", dep("a", &a.url, ""), dep("b", &b.url, ""));

    let manager = project(&base, &deps);
    let err = manager.install(&[]).unwrap_err().to_string();
    assert!(err.contains("no version of 'common' matches"), "{err}");
    assert!(
        err.contains("^1 (from a v1.0.0)") && err.contains("^2 (from b v1.0.0)"),
        "{err}"
    );

    // Pinning it in pleno.toml decides.
    let pinned = project(
        &base.join("pinned"),
        &format!("{deps}{}", dep("common", &common.url, ", tag = \"v2.0.0\"")),
    );
    let report = pinned.install(&[]).unwrap();
    assert_eq!(locked(&report, "common").version.as_deref(), Some("2.0.0"));

    // Two different repositories under one name.
    let other = Repo::new(&base, "other");
    other.release("common", "1.0.0", "");
    let c = Repo::new(&base, "c");
    c.release("c", "1.0.0", &dep("common", &other.url, ""));
    let clash = project(
        &base.join("clash"),
        &format!("{}{}", dep("a", &a.url, ""), dep("c", &c.url, "")),
    );
    let err = clash.install(&[]).unwrap_err().to_string();
    assert!(err.contains("required from different places"), "{err}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn branch_rev_and_path_dependencies() {
    let base = temp("pins");
    let greet = Repo::new(&base, "greet");
    let first = greet.commit("greet", "0.1.0", "");
    let second = greet.commit("greet", "0.2.0", "");
    let local = base.join("local");
    std::fs::create_dir_all(&local).unwrap();
    std::fs::write(local.join("lib.pln"), "export fn x() { return 1; }\n").unwrap();

    let manager = project(
        &base,
        &format!(
            "{}local = {{ path = \"../local\" }}\n",
            dep("greet", &greet.url, ", branch = \"main\""),
        ),
    );
    let report = manager.install(&[]).unwrap();
    let entry = locked(&report, "greet");
    assert_eq!(entry.branch.as_deref(), Some("main"));
    assert_eq!(entry.commit.as_deref(), Some(second.as_str()));
    assert!(locked(&report, "local").source.starts_with("path+"));
    assert!(manager.packages_dir().join("local/lib.pln").exists());

    let rev = project(
        &base.join("p3"),
        &dep(
            "greet",
            &greet.url,
            &format!(", rev = \"{}\"", &first[..10]),
        ),
    );
    let report = rev.install(&[]).unwrap();
    assert_eq!(
        locked(&report, "greet").commit.as_deref(),
        Some(first.as_str())
    );
    assert!(installed_version(&rev, "greet").contains("0.1.0"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn remove_takes_the_package_out_everywhere() {
    let base = temp("remove");
    let greet = Repo::new(&base, "greet");
    greet.release("greet", "1.0.0", "");
    let manager = project(&base, &dep("greet", &greet.url, ""));
    manager.install(&[]).unwrap();
    assert!(manager.packages_dir().join("greet").exists());

    let report = manager.remove("greet").unwrap();
    assert!(matches!(&report.changes[..], [Change::Removed { .. }]));
    assert!(!manager.packages_dir().join("greet").exists());
    assert!(report.lockfile.packages.is_empty());
    let manifest = std::fs::read_to_string(manager.manifest_path()).unwrap();
    assert!(!manifest.contains("greet"), "{manifest}");
    assert!(manager.remove("greet").is_err());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn errors_name_the_problem() {
    let base = temp("errors");
    let missing = file_url(&base.join("remotes/missing.git"));
    let manager = project(&base, &dep("missing", &missing, ""));
    let err = manager.install(&[]).unwrap_err().to_string();
    assert!(err.contains("was not found"), "{err}");

    let bare = project(&base.join("bare"), "greet = \"^1.0\"\n");
    let err = bare.install(&[]).unwrap_err().to_string();
    assert!(err.contains("no central registry"), "{err}");

    // A failed add leaves pleno.toml alone.
    let manager = project(&base.join("add"), "");
    let before = std::fs::read_to_string(manager.manifest_path()).unwrap();
    let err = manager
        .add(&AddRequest {
            name: None,
            source: DepSource::Git(missing.clone()),
            want: Some(Want::Range(VersionReq::parse("^1").unwrap())),
            version_text: Some("^1".into()),
            dev: false,
        })
        .unwrap_err();
    assert!(err.to_string().contains("was not found"), "{err}");
    assert_eq!(
        std::fs::read_to_string(manager.manifest_path()).unwrap(),
        before
    );

    let tagless = Repo::new(&base, "tagless");
    tagless.commit("tagless", "0.1.0", "");
    let err = manager
        .add(&AddRequest {
            name: None,
            source: DepSource::Git(tagless.url.clone()),
            want: None,
            version_text: None,
            dev: false,
        })
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("no release tags") && err.contains("--rev"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn publish_checks_the_package_before_tagging() {
    let base = temp("publish");
    let greet = Repo::new(&base, "greet");
    greet.release("greet", "1.0.0", "");

    // The released version's tag exists.
    let err = publish::check(&greet.work).unwrap_err().to_string();
    assert!(err.contains("tag v1.0.0 already exists"), "{err}");

    // A new version, uncommitted.
    std::fs::write(
        greet.work.join("pleno.toml"),
        "[package]\nname = \"greet\"\nversion = \"1.1.0\"\nmain = \"lib.pln\"\n",
    )
    .unwrap();
    let err = publish::check(&greet.work).unwrap_err().to_string();
    assert!(err.contains("uncommitted changes"), "{err}");

    git(&greet.work, &["commit", "--quiet", "-am", "1.1.0"]);
    let plan = publish::check(&greet.work).unwrap();
    assert_eq!(plan.tag, "v1.1.0");
    assert_eq!(plan.remote.as_deref(), Some("origin"));
    assert_eq!(
        plan.command_lines(),
        vec![
            "git tag -a v1.1.0 -m \"greet v1.1.0\"".to_string(),
            "git push origin v1.1.0".to_string()
        ]
    );
    publish::push(&greet.work, &plan).unwrap();

    // The pushed tag is installable.
    let manager = project(&base, &dep("greet", &greet.url, ", version = \"^1.1\""));
    let report = manager.install(&[]).unwrap();
    assert_eq!(locked(&report, "greet").version.as_deref(), Some("1.1.0"));

    // Path dependencies cannot be published.
    std::fs::write(
        greet.work.join("pleno.toml"),
        "[package]\nname = \"greet\"\nversion = \"1.2.0\"\n\n[dependencies]\nx = { path = \"../x\" }\n",
    )
    .unwrap();
    git(&greet.work, &["commit", "--quiet", "-am", "path"]);
    let err = publish::check(&greet.work).unwrap_err().to_string();
    assert!(err.contains("folders on your machine"), "{err}");
    let _ = std::fs::remove_dir_all(&base);
}
