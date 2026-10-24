// Shared by the build scripts of the binary crates: embeds Windows version
// metadata in the executables. Code signing (SignPath) requires every signed
// binary to carry the product name and an identical product version.
fn embed_windows_metadata(file_description: &str) {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let mut resource = winresource::WindowsResource::new();
    resource
        .set("ProductName", "Pleno")
        .set("FileDescription", file_description)
        .set("ProductVersion", &version)
        .set("FileVersion", &version)
        .set("LegalCopyright", "Copyright (c) 2024-2026 Pleno Contributors. MIT License.");
    if let Err(err) = resource.compile() {
        // Building without the Windows resource compiler still produces a
        // working binary; release builds on GitHub Actions have it.
        println!("cargo:warning=could not embed Windows metadata: {err}");
    }
}
