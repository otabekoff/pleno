//! `plm publish`: publishing a package is pushing a `vX.Y.Z` tag. This checks
//! that the package is ready and says (or runs) the two git commands.

use crate::manifest::{DepSource, Manifest, default_name};
use crate::{PackageError, git};
use semver::Version;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct PublishPlan {
    pub name: String,
    pub version: Version,
    pub tag: String,
    /// The remote to push to (`origin` when there is one).
    pub remote: Option<String>,
    pub remote_url: Option<String>,
    /// The commit that will be tagged (HEAD).
    pub commit: String,
    pub warnings: Vec<String>,
}

impl PublishPlan {
    /// The commands that publish the package.
    pub fn commands(&self) -> Vec<Vec<String>> {
        let remote = self.remote.clone().unwrap_or_else(|| "origin".to_string());
        vec![
            vec![
                "git".into(),
                "tag".into(),
                "-a".into(),
                self.tag.clone(),
                "-m".into(),
                format!("{} {}", self.name, self.tag),
            ],
            vec!["git".into(), "push".into(), remote, self.tag.clone()],
        ]
    }

    /// The commands as you would type them.
    pub fn command_lines(&self) -> Vec<String> {
        self.commands()
            .iter()
            .map(|args| {
                args.iter()
                    .map(|a| {
                        if a.contains(' ') {
                            format!("\"{a}\"")
                        } else {
                            a.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect()
    }
}

fn fail(message: impl Into<String>) -> PackageError {
    PackageError::Publish(message.into())
}

/// Checks the package in `root` for publishing; does not change anything.
pub fn check(root: &Path) -> Result<PublishPlan, PackageError> {
    let manifest = Manifest::load_from_file(root.join("pleno.toml"))?;
    manifest.validate()?;
    let version = Version::parse(&manifest.package.version).map_err(|_| {
        fail(format!(
            "version '{}' in pleno.toml is not a semantic version such as 1.2.0",
            manifest.package.version
        ))
    })?;
    let path_deps: Vec<&str> = manifest
        .dependencies
        .iter()
        .filter(|(name, dep)| matches!(dep.spec(name), Ok((DepSource::Path(_), _))))
        .map(|(name, _)| name.as_str())
        .collect();
    if !path_deps.is_empty() {
        return Err(fail(format!(
            "these dependencies are folders on your machine, which others cannot install: {}. Publish them \
             as Git repositories and depend on them with {{ git = ..., version = ... }}",
            path_deps.join(", ")
        )));
    }

    let toplevel = git::output_in(root, &["rev-parse", "--show-toplevel"]).ok_or_else(|| {
        fail(
            "this folder is not in a Git repository. A Pleno package is a Git repository: run `git init`, \
             commit, and push it to GitHub (or any Git host) first",
        )
    })?;
    let same = |a: &Path, b: &Path| match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if !same(Path::new(&toplevel), root) {
        return Err(fail(format!(
            "pleno.toml must be at the top of the Git repository ({toplevel}): plm installs a whole repository \
             as one package"
        )));
    }
    let commit = git::output_in(root, &["rev-parse", "HEAD"])
        .ok_or_else(|| fail("the repository has no commits yet: commit your package first"))?;
    let status = git::output_in(root, &["status", "--porcelain", "--untracked-files=no"])
        .unwrap_or_default();
    if !status.is_empty() {
        return Err(fail(format!(
            "{} tracked file(s) have uncommitted changes. Commit them first, so the tag holds what you tested",
            status.lines().count()
        )));
    }

    let tag = format!("v{version}");
    if git::output_in(
        root,
        &[
            "rev-parse",
            "--quiet",
            "--verify",
            &format!("refs/tags/{tag}"),
        ],
    )
    .is_some()
    {
        return Err(fail(format!(
            "tag {tag} already exists. A published tag must never move: raise `version` in pleno.toml"
        )));
    }

    let mut warnings = Vec::new();
    let local_tags = git::output_in(root, &["tag", "--list"]).unwrap_or_default();
    if let Some(highest) = local_tags
        .lines()
        .filter_map(|t| Version::parse(t.trim().strip_prefix('v').unwrap_or(t.trim())).ok())
        .max()
        && highest > version
    {
        warnings.push(format!(
            "v{version} is lower than the newest tag, v{highest}; `version = \"^...\"` requirements pick the highest"
        ));
    }

    let remotes = git::output_in(root, &["remote"]).unwrap_or_default();
    let remote = remotes
        .lines()
        .find(|r| *r == "origin")
        .or_else(|| remotes.lines().next())
        .map(str::to_string);
    let remote_url = remote
        .as_ref()
        .and_then(|r| git::output_in(root, &["remote", "get-url", r]));
    match (&remote, &remote_url) {
        (Some(remote), Some(url)) => {
            match git::output_in(
                root,
                &["ls-remote", "--tags", remote, &format!("refs/tags/{tag}")],
            ) {
                Some(out) if !out.is_empty() => {
                    return Err(fail(format!(
                        "{url} already has a tag {tag}. Raise `version` in pleno.toml"
                    )));
                }
                Some(_) => {}
                None => warnings.push(format!(
                    "could not check {url} for an existing {tag} tag (no network, or no access)"
                )),
            }
            let repo = default_name(url);
            if repo != manifest.package.name {
                warnings.push(format!(
                    "the repository is named '{repo}' but the package '{}': `plm add` names the dependency \
                     after the repository unless given --name",
                    manifest.package.name
                ));
            }
        }
        _ => warnings.push(
            "the repository has no remote: add one (`git remote add origin https://github.com/<you>/<repo>`) \
             before pushing"
                .to_string(),
        ),
    }

    Ok(PublishPlan {
        name: manifest.package.name,
        version,
        tag,
        remote,
        remote_url,
        commit,
        warnings,
    })
}

/// Creates the tag and pushes it.
pub fn push(root: &Path, plan: &PublishPlan) -> Result<(), PackageError> {
    if plan.remote.is_none() {
        return Err(fail(
            "the repository has no remote to push to: add one with `git remote add origin <url>`",
        ));
    }
    for command in plan.commands() {
        let args: Vec<&str> = command[1..].iter().map(String::as_str).collect();
        git::run_in(root, &args)?;
    }
    Ok(())
}
