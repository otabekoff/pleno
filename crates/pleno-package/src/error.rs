use std::path::PathBuf;
use thiserror::Error;

/// Everything that can go wrong while reading a package, resolving its
/// dependencies or installing them. The messages are written for the person
/// at the terminal: each says what failed and, where there is one, what to do.
#[derive(Error, Debug)]
pub enum PackageError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("cannot read TOML: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("cannot write TOML: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("package manifest not found at {0}")]
    NotFound(PathBuf),
    #[error("missing 'name' field in manifest")]
    MissingName,
    #[error("invalid version requirement '{0}'")]
    InvalidVersionReq(String),
    /// A `pleno.toml` that parses but says something plm cannot use.
    #[error("{0}")]
    InvalidManifest(String),
    /// `pleno.lock` cannot be used.
    #[error("{0}")]
    Lockfile(String),
    /// `git` failed, or the repository cannot be reached.
    #[error("{0}")]
    Git(String),
    /// No version satisfies a requirement.
    #[error(
        "no version of '{name}' matches {requirement}: {url} has {}",
        available_list(available)
    )]
    NoMatchingVersion {
        name: String,
        url: String,
        requirement: String,
        available: Vec<String>,
    },
    /// Two packages want different, incompatible things.
    #[error("{0}")]
    Conflict(String),
    /// The files of a package do not match the checksum in `pleno.lock`.
    #[error(
        "checksum mismatch for '{name}' at commit {commit}: pleno.lock has {expected}, the files give {actual}. \
         The files were changed after pleno.lock was written (a corrupt cache, or a tampered package). \
         Delete {cache} and run `plm install` again; if it still fails, do not trust this commit"
    )]
    ChecksumMismatch {
        name: String,
        commit: String,
        expected: String,
        actual: String,
        cache: PathBuf,
    },
    /// A tag now points to another commit than the one in `pleno.lock`.
    #[error(
        "tag {tag} of {url} now points to commit {now}, but pleno.lock has {locked}: the tag was moved \
         (force-pushed) after you locked it. plm will not install the new commit silently. If you trust it, \
         delete the '{name}' entry from pleno.lock and run `plm install`"
    )]
    TagMoved {
        name: String,
        url: String,
        tag: String,
        locked: String,
        now: String,
    },
    /// `--offline` was given and something needs the network.
    #[error("{0}")]
    Offline(String),
    /// `plm publish` found a problem.
    #[error("{0}")]
    Publish(String),
    #[error("Compile error: {0}")]
    CompileError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Lex error: {0}")]
    LexError(String),
}

fn available_list(available: &[String]) -> String {
    if available.is_empty() {
        return "no tags of the form vX.Y.Z".to_string();
    }
    let shown: Vec<&str> = available.iter().rev().take(8).map(String::as_str).collect();
    let more = if available.len() > shown.len() {
        format!(" and {} older", available.len() - shown.len())
    } else {
        String::new()
    };
    format!("{}{more}", shown.join(", "))
}
