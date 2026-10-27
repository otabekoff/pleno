//! What `plm add` accepts: a Git URL, a host shorthand or a folder, with an
//! optional `@<version requirement>`.

use crate::PackageError;
use crate::manifest::{DepSource, parse_requirement};

/// Hosts that may be written without `https://`, as in `github.com/ada/greet`.
const KNOWN_HOSTS: [&str; 4] = [
    "github.com/",
    "gitlab.com/",
    "codeberg.org/",
    "bitbucket.org/",
];

/// Shorthands: `github:ada/greet` is `https://github.com/ada/greet`.
const SHORTHANDS: [(&str, &str); 3] = [
    ("github:", "https://github.com/"),
    ("gitlab:", "https://gitlab.com/"),
    ("codeberg:", "https://codeberg.org/"),
];

/// A parsed `plm add` argument: where the package is and, if `@` was given,
/// the version requirement (without a leading `v`).
#[derive(Debug, PartialEq, Eq)]
pub struct AddSpec {
    pub source: DepSource,
    pub version: Option<String>,
}

fn looks_like_path(spec: &str) -> bool {
    spec.starts_with("./")
        || spec.starts_with("../")
        || spec.starts_with(".\\")
        || spec.starts_with("..\\")
        || spec == "."
        || spec == ".."
        || spec.starts_with('/')
        || (spec.len() > 2
            && spec.as_bytes()[1] == b':'
            && matches!(spec.as_bytes()[2], b'\\' | b'/'))
}

fn looks_like_url(spec: &str) -> bool {
    spec.contains("://")
        // scp-like: git@github.com:ada/greet.git
        || (spec.contains('@') && spec.contains(':') && !spec.contains("://"))
}

/// Splits `url@req` when the part after the last `@` is a version, not part
/// of the URL (`git@host:path`, `https://user@host/path`).
fn split_version(spec: &str) -> (&str, Option<&str>) {
    if let Some((before, after)) = spec.rsplit_once('@')
        && !before.is_empty()
        && !after.is_empty()
        && !after.contains('/')
        && !after.contains(':')
    {
        return (before, Some(after));
    }
    (spec, None)
}

pub fn parse_add_spec(spec: &str) -> Result<AddSpec, PackageError> {
    let spec = spec.trim();
    if looks_like_path(spec) {
        return Ok(AddSpec {
            source: DepSource::Path(spec.to_string()),
            version: None,
        });
    }
    let (base, version) = split_version(spec);
    let url = if let Some((prefix, host)) = SHORTHANDS.iter().find(|(p, _)| base.starts_with(p)) {
        let rest = &base[prefix.len()..];
        if rest.split('/').filter(|s| !s.is_empty()).count() != 2 {
            return Err(PackageError::InvalidManifest(format!(
                "'{spec}': write {prefix}<owner>/<repository>"
            )));
        }
        format!("{host}{rest}")
    } else if KNOWN_HOSTS.iter().any(|h| base.starts_with(h)) {
        format!("https://{base}")
    } else if looks_like_url(base) {
        base.to_string()
    } else if std::path::Path::new(spec).is_dir() {
        return Ok(AddSpec {
            source: DepSource::Path(spec.to_string()),
            version: None,
        });
    } else {
        return Err(PackageError::InvalidManifest(format!(
            "'{spec}' is not a package source. Pleno has no central registry: give a Git URL \
             (https://github.com/<owner>/{0}), a shorthand (github:<owner>/{0}) or a folder (../{0}). \
             Find packages at https://github.com/topics/pleno-package",
            base
        )));
    };
    let version = match version {
        Some(text) => {
            parse_requirement(text)?;
            Some(
                text.strip_prefix('v')
                    .filter(|r| r.starts_with(|c: char| c.is_ascii_digit()))
                    .unwrap_or(text)
                    .to_string(),
            )
        }
        None => None,
    };
    Ok(AddSpec {
        source: DepSource::Git(url),
        version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(url: &str, version: Option<&str>) -> AddSpec {
        AddSpec {
            source: DepSource::Git(url.to_string()),
            version: version.map(str::to_string),
        }
    }

    #[test]
    fn add_specs() {
        assert_eq!(
            parse_add_spec("github:ada/greet@^1.2").unwrap(),
            git("https://github.com/ada/greet", Some("^1.2"))
        );
        assert_eq!(
            parse_add_spec("github.com/ada/greet@v1.2.0").unwrap(),
            git("https://github.com/ada/greet", Some("1.2.0"))
        );
        assert_eq!(
            parse_add_spec("https://gitlab.example.com/team/greet.git").unwrap(),
            git("https://gitlab.example.com/team/greet.git", None)
        );
        assert_eq!(
            parse_add_spec("git@github.com:ada/greet.git").unwrap(),
            git("git@github.com:ada/greet.git", None)
        );
        assert_eq!(
            parse_add_spec("git@github.com:ada/greet.git@~1.4").unwrap(),
            git("git@github.com:ada/greet.git", Some("~1.4"))
        );
        assert_eq!(
            parse_add_spec("file:///C:/repos/greet@1").unwrap(),
            git("file:///C:/repos/greet", Some("1"))
        );
        assert_eq!(
            parse_add_spec("../greet").unwrap().source,
            DepSource::Path("../greet".into())
        );
    }

    #[test]
    fn bare_names_are_not_sources() {
        let err = parse_add_spec("greet@1.0").unwrap_err().to_string();
        assert!(err.contains("no central registry"), "{err}");
        assert!(parse_add_spec("github:ada").is_err());
        assert!(parse_add_spec("github:ada/greet@not-a-version").is_err());
    }
}
