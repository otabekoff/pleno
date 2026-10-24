//! The local registry serves packages from `<root>/<name>/<version>/`.

use pleno_package::{PackageManager, Registry};
use semver::{Version, VersionReq};

#[test]
fn serves_packages_from_the_registry_directory() {
    let root = std::env::temp_dir().join(format!("pleno-local-registry-{}", std::process::id()));
    for version in ["1.0.0", "1.2.0", "2.0.0"] {
        let dir = root.join("demo").join(version);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("pleno.toml"),
            format!("[package]\nname = \"demo\"\nversion = \"{version}\"\n"),
        )
        .unwrap();
    }
    // A version directory without a manifest is not a package.
    std::fs::create_dir_all(root.join("demo").join("3.0.0")).unwrap();
    // SAFETY: this test binary runs only this test, so nothing reads the
    // environment concurrently.
    unsafe { std::env::set_var("PLENO_LOCAL_REGISTRY", &root) };

    let manager = PackageManager::new(root.join("installed"));
    let versions = manager
        .query("demo", &VersionReq::parse("^1").unwrap())
        .unwrap();
    assert_eq!(versions, vec![Version::new(1, 0, 0), Version::new(1, 2, 0)]);
    assert!(
        manager
            .query("demo", &VersionReq::parse("^3").unwrap())
            .unwrap()
            .is_empty()
    );
    assert!(
        manager
            .query("missing", &VersionReq::STAR)
            .unwrap()
            .is_empty()
    );

    let (source, _) = manager
        .fetch_source("demo", &Version::new(1, 2, 0))
        .unwrap();
    assert!(source.ends_with("1.2.0"), "source was {source}");
    let manifest = manager
        .fetch_manifest("demo", &Version::new(1, 2, 0))
        .unwrap();
    assert_eq!(manifest.package.version, "1.2.0");
    assert!(
        manager
            .fetch_source("demo", &Version::new(9, 0, 0))
            .is_err()
    );

    std::fs::remove_dir_all(&root).ok();
}
