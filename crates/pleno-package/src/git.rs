//! Running `git`. plm never talks to a Git host itself: it runs the user's
//! `git`, so HTTPS credential helpers, SSH keys and `insteadOf` rules work as
//! they do for `git clone`, and plm never sees or stores a token.

use crate::PackageError;
use std::io::IsTerminal;
use std::path::Path;
use std::process::{Command, Output};

/// The tags and branches of a remote repository.
#[derive(Debug, Default, Clone)]
pub struct RemoteRefs {
    /// `(tag name, commit)`, with annotated tags peeled to their commit.
    pub tags: Vec<(String, String)>,
    /// `(branch name, commit)`.
    pub heads: Vec<(String, String)>,
}

impl RemoteRefs {
    pub fn tag(&self, name: &str) -> Option<&str> {
        self.tags
            .iter()
            .find(|(tag, _)| tag == name)
            .map(|(_, commit)| commit.as_str())
    }

    pub fn head(&self, name: &str) -> Option<&str> {
        self.heads
            .iter()
            .find(|(head, _)| head == name)
            .map(|(_, commit)| commit.as_str())
    }
}

/// Variables that point git at one particular repository. plm can run inside
/// a git hook, where they are set; left in place, `git init <scratch>` would
/// re-initialize the hook's repository instead of creating the scratch one.
pub const REPOSITORY_VARIABLES: [&str; 8] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_PREFIX",
];

fn git() -> Command {
    let mut cmd = Command::new("git");
    for variable in REPOSITORY_VARIABLES {
        cmd.env_remove(variable);
    }
    // Asking for a password on the terminal is fine when someone is there to
    // answer; in CI or a pipe it would hang, so git fails instead.
    if !std::io::stdin().is_terminal() {
        cmd.env("GIT_TERMINAL_PROMPT", "0");
    }
    cmd
}

fn run(mut cmd: Command, url: &str) -> Result<Output, PackageError> {
    let output = cmd.output().map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            PackageError::Git(
                "plm needs git to fetch packages, and `git` is not on PATH. Install Git from https://git-scm.com"
                    .to_string(),
            )
        } else {
            PackageError::Git(format!("cannot run git for {url}: {err}"))
        }
    })?;
    Ok(output)
}

/// Turns git's error output into a message that says what to do.
pub fn describe_failure(url: &str, stderr: &str) -> PackageError {
    let lower = stderr.to_ascii_lowercase();
    let last = stderr
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .unwrap_or("git failed")
        .to_string();
    let message = if [
        "could not read username",
        "could not read password",
        "authentication failed",
        "terminal prompts disabled",
        "permission denied (publickey)",
        "invalid username or password",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        format!(
            "cannot read {url}: git has no credentials for it. If the repository is private, plm uses your own \
             git login and never stores tokens: set up a credential helper (for GitHub, `gh auth setup-git`) \
             or an SSH key (and an ssh:// or git@ URL), then check that `git ls-remote {url}` works. If it is \
             not private, check the URL: GitHub asks for a login when a repository does not exist. ({last})"
        )
    } else if lower.contains("repository not found")
        || lower.contains("does not appear to be a git repository")
        || lower.contains("not found")
        || lower.contains("does not exist")
    {
        format!(
            "repository {url} was not found. Check the URL; a private repository also needs your git \
             credentials. ({last})"
        )
    } else if lower.contains("could not resolve host")
        || lower.contains("unable to access")
        || lower.contains("connection")
        || lower.contains("timed out")
    {
        format!(
            "cannot reach {url}: {last}. Check your network, or use `--offline` if pleno.lock and the cache \
             have everything"
        )
    } else {
        format!("git failed for {url}: {last}")
    };
    PackageError::Git(message)
}

/// Lists the tags and branches of `url` with `git ls-remote`.
pub fn ls_remote(url: &str) -> Result<RemoteRefs, PackageError> {
    let mut cmd = git();
    cmd.args(["ls-remote", "--tags", "--heads", "--", url]);
    let output = run(cmd, url)?;
    if !output.status.success() {
        return Err(describe_failure(
            url,
            &String::from_utf8_lossy(&output.stderr),
        ));
    }
    Ok(parse_ls_remote(&String::from_utf8_lossy(&output.stdout)))
}

pub fn parse_ls_remote(text: &str) -> RemoteRefs {
    let mut refs = RemoteRefs::default();
    let mut peeled: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let Some((commit, name)) = line.split_once('\t') else {
            continue;
        };
        let commit = commit.trim().to_string();
        if let Some(tag) = name.strip_prefix("refs/tags/") {
            if let Some(tag) = tag.strip_suffix("^{}") {
                peeled.push((tag.to_string(), commit));
            } else {
                refs.tags.push((tag.to_string(), commit));
            }
        } else if let Some(head) = name.strip_prefix("refs/heads/") {
            refs.heads.push((head.to_string(), commit));
        }
    }
    // An annotated tag is listed twice; the `^{}` line names its commit.
    for (tag, commit) in peeled {
        if let Some(entry) = refs.tags.iter_mut().find(|(name, _)| *name == tag) {
            entry.1 = commit;
        }
    }
    refs
}

/// A scratch bare repository, removed when dropped.
struct Scratch {
    dir: std::path::PathBuf,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Scratch {
    fn new(parent: &Path) -> Result<Self, PackageError> {
        std::fs::create_dir_all(parent)?;
        let dir = parent.join(format!(
            ".fetch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let scratch = Scratch { dir };
        let mut cmd = git();
        cmd.args(["init", "--quiet", "--bare"]).arg(&scratch.dir);
        let output = run(cmd, "a scratch repository")?;
        if !output.status.success() {
            return Err(PackageError::Git(format!(
                "git init failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(scratch)
    }

    fn git(&self) -> Command {
        let mut cmd = git();
        cmd.arg("--git-dir").arg(&self.dir);
        cmd
    }

    /// Fetches `what` (a commit or a ref) from `url`; `depth` 1 is a shallow
    /// fetch of that commit alone.
    fn fetch(&self, url: &str, what: &[&str], shallow: bool) -> Result<Output, PackageError> {
        let mut cmd = self.git();
        cmd.args(["fetch", "--quiet", "--no-tags"]);
        if shallow {
            cmd.args(["--depth", "1"]);
        }
        cmd.arg("--").arg(url).args(what);
        run(cmd, url)
    }

    fn has_commit(&self, commit: &str) -> bool {
        let mut cmd = self.git();
        cmd.args(["cat-file", "-e", &format!("{commit}^{{commit}}")]);
        cmd.output().is_ok_and(|o| o.status.success())
    }

    fn rev_parse(&self, rev: &str) -> Option<String> {
        let mut cmd = self.git();
        cmd.args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{rev}^{{commit}}"),
        ]);
        let output = cmd.output().ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

const ALL_REFS: [&str; 2] = ["+refs/heads/*:refs/heads/*", "+refs/tags/*:refs/tags/*"];

/// Expands an abbreviated commit SHA by fetching the repository's history.
pub fn expand_rev(url: &str, rev: &str, scratch_parent: &Path) -> Result<String, PackageError> {
    let scratch = Scratch::new(scratch_parent)?;
    let output = scratch.fetch(url, &ALL_REFS, false)?;
    if !output.status.success() {
        return Err(describe_failure(
            url,
            &String::from_utf8_lossy(&output.stderr),
        ));
    }
    scratch.rev_parse(rev).ok_or_else(|| {
        PackageError::Git(format!(
            "commit {rev} is not in {url}: check the rev, or use the full 40-character SHA"
        ))
    })
}

/// Fetches `commit` from `url` and writes its files (regular files only,
/// with LF line endings as stored in Git) into `dest`, which must not exist.
/// `hint` is the tag or branch the commit came from, tried when the server
/// refuses to send a commit by its SHA.
pub fn fetch_commit_files(
    url: &str,
    commit: &str,
    hint: Option<&str>,
    dest: &Path,
) -> Result<(), PackageError> {
    let parent = dest.parent().unwrap_or(Path::new("."));
    let scratch = Scratch::new(parent)?;
    // 1. A shallow fetch of just this commit (GitHub, GitLab and git 2.x
    //    servers allow it).
    let first = scratch.fetch(url, &[commit], true)?;
    if !scratch.has_commit(commit) {
        // 2. The tag or branch it came from, then 3. everything.
        let mut stderr = String::from_utf8_lossy(&first.stderr).into_owned();
        if let Some(hint) = hint {
            let refspec = format!("+{hint}:refs/plm/hint");
            let output = scratch.fetch(url, &[&refspec], false)?;
            stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        }
        if !scratch.has_commit(commit) {
            let output = scratch.fetch(url, &ALL_REFS, false)?;
            if !output.status.success() {
                return Err(describe_failure(
                    url,
                    &format!("{stderr}\n{}", String::from_utf8_lossy(&output.stderr)),
                ));
            }
        }
        if !scratch.has_commit(commit) {
            return Err(PackageError::Git(format!(
                "commit {commit} is no longer in {url}. Its tag or branch was probably force-pushed or deleted \
                 after pleno.lock was written. Ask the package's author, or run `plm update` to choose again"
            )));
        }
    }

    let mut cmd = scratch.git();
    // The same bytes on every OS: no CRLF conversion.
    cmd.args([
        "-c",
        "core.autocrlf=false",
        "-c",
        "core.eol=lf",
        "archive",
        "--format=tar",
        commit,
    ]);
    let output = run(cmd, url)?;
    if !output.status.success() {
        return Err(PackageError::Git(format!(
            "git archive failed for {url} at {commit}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    unpack_tar(&output.stdout, dest)
}

fn unpack_tar(bytes: &[u8], dest: &Path) -> Result<(), PackageError> {
    std::fs::create_dir_all(dest)?;
    let mut archive = tar::Archive::new(bytes);
    for entry in archive.entries()? {
        let mut entry = entry?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path()?.into_owned();
        // Only plain relative paths: nothing may land outside `dest`.
        if !path
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
        {
            continue;
        }
        let target = dest.join(&path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::File::create(&target)?;
        std::io::copy(&mut entry, &mut file)?;
    }
    Ok(())
}

/// Runs git in `dir` and returns its trimmed output, or None if it failed.
pub fn output_in(dir: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = git();
    cmd.current_dir(dir).args(args);
    let output = cmd.output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Runs git in `dir`, inheriting the terminal, for `plm publish --push`.
pub fn run_in(dir: &Path, args: &[&str]) -> Result<(), PackageError> {
    let status = git().current_dir(dir).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(PackageError::Git(format!(
            "`git {}` failed ({status})",
            args.join(" ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ls_remote_output_is_parsed_with_peeled_tags() {
        let refs = parse_ls_remote(
            "aaa\trefs/heads/main\nbbb\trefs/tags/v1.0.0\nccc\trefs/tags/v1.1.0\nddd\trefs/tags/v1.1.0^{}\n",
        );
        assert_eq!(refs.head("main"), Some("aaa"));
        assert_eq!(refs.tag("v1.0.0"), Some("bbb"));
        assert_eq!(refs.tag("v1.1.0"), Some("ddd"));
    }

    #[test]
    fn git_failures_say_what_to_do() {
        let err = describe_failure(
            "https://github.com/a/b",
            "fatal: could not read Username for 'https://github.com': terminal prompts disabled",
        )
        .to_string();
        assert!(err.contains("credential helper"), "{err}");
        let err = describe_failure(
            "file:///nope",
            "fatal: '/nope' does not appear to be a git repository",
        )
        .to_string();
        assert!(err.contains("was not found"), "{err}");
    }
}
