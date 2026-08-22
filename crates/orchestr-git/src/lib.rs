//! A narrow, command-array based interface to the installed Git executable.
//!
//! Callers provide an explicit workspace path. The service never invokes a
//! shell, and it resolves repository roots before returning repository data.

use std::{
    ffi::OsStr,
    fmt,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitSummary {
    pub hash: String,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySummary {
    pub root_path: String,
    pub default_branch: String,
    pub current_branch: Option<String>,
    pub is_clean: bool,
    pub changed_file_count: usize,
    pub latest_commit: Option<CommitSummary>,
}

#[derive(Debug)]
pub struct GitError(String);

impl fmt::Display for GitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for GitError {}

pub type Result<T> = std::result::Result<T, GitError>;

pub struct GitService;

impl GitService {
    pub fn initialize_repository(path: &Path) -> Result<RepositorySummary> {
        let path = canonical_directory(path)?;
        run_git(&path, ["init"])?;
        Self::inspect_repository(&path)
    }

    pub fn inspect_repository(path: &Path) -> Result<RepositorySummary> {
        let root_path = repository_root(path)?;
        let current_branch = non_empty(run_git(&root_path, ["branch", "--show-current"])?);
        let default_branch = default_branch(&root_path, current_branch.as_deref())?;
        let status = run_git(&root_path, ["status", "--porcelain=v1"])?;
        let latest_commit = latest_commit(&root_path)?;

        Ok(RepositorySummary {
            root_path: root_path.to_string_lossy().into_owned(),
            default_branch,
            current_branch,
            is_clean: status.trim().is_empty(),
            changed_file_count: status.lines().count(),
            latest_commit,
        })
    }
}

fn canonical_directory(path: &Path) -> Result<PathBuf> {
    if !path.is_dir() {
        return Err(GitError(format!(
            "Workspace directory does not exist: {}",
            path.display()
        )));
    }
    path.canonicalize().map_err(|error| {
        GitError(format!(
            "Unable to resolve workspace directory {}: {error}",
            path.display()
        ))
    })
}

fn repository_root(path: &Path) -> Result<PathBuf> {
    let path = canonical_directory(path)?;
    let output = run_git(&path, ["rev-parse", "--show-toplevel"])?;
    let root = PathBuf::from(output.trim());
    canonical_directory(&root)
}

fn default_branch(path: &Path, current_branch: Option<&str>) -> Result<String> {
    if let Some(remote_head) = run_git_if_success(
        path,
        [
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    )? {
        if let Some((_, branch)) = remote_head.trim().split_once('/') {
            return Ok(branch.to_owned());
        }
    }

    current_branch
        .map(str::to_owned)
        .ok_or_else(|| GitError("Unable to determine the repository default branch.".into()))
}

fn latest_commit(path: &Path) -> Result<Option<CommitSummary>> {
    let Some(output) = run_git_if_success(path, ["log", "-1", "--format=%H%x1f%s"])? else {
        return Ok(None);
    };
    let Some((hash, subject)) = output.trim().split_once('\u{1f}') else {
        return Ok(None);
    };
    Ok(Some(CommitSummary {
        hash: hash.to_owned(),
        subject: subject.to_owned(),
    }))
}

fn run_git<I, S>(path: &Path, arguments: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = execute_git(path, arguments)?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(command_error(&output))
    }
}

fn run_git_if_success<I, S>(path: &Path, arguments: I) -> Result<Option<String>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = execute_git(path, arguments)?;
    if output.status.success() {
        Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()))
    } else {
        Ok(None)
    }
}

fn execute_git<I, S>(path: &Path, arguments: I) -> Result<std::process::Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new("git")
        .args(arguments)
        .current_dir(path)
        .output()
        .map_err(|error| GitError(format!("Unable to run Git: {error}")))
}

fn command_error(output: &std::process::Output) -> GitError {
    let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    GitError(if message.is_empty() {
        "Git command failed without an error message.".to_owned()
    } else {
        message
    })
}

fn non_empty(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::GitService;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn initializes_and_inspects_an_empty_repository() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("orchestr-git-{nonce}"));
        fs::create_dir(&directory).expect("temporary directory creates");

        let repository =
            GitService::initialize_repository(&directory).expect("repository initializes");
        assert_eq!(
            repository.current_branch.as_deref(),
            Some(repository.default_branch.as_str())
        );
        assert!(repository.is_clean);
        assert!(repository.latest_commit.is_none());

        fs::remove_dir_all(directory).expect("temporary directory removes");
    }
}
