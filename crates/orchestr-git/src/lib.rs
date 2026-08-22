//! A narrow, command-array based interface to the installed Git executable.
//!
//! Callers provide an explicit workspace path. The service never invokes a
//! shell, and it resolves repository roots before returning repository data.

use std::{
    ffi::OsStr,
    fmt, fs,
    path::{Path, PathBuf},
    process::Command,
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitSummary {
    pub hash: String,
    pub short_hash: String,
    pub subject: String,
    pub author: String,
    pub authored_at: String,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryDetails {
    pub summary: RepositorySummary,
    pub recent_commits: Vec<CommitSummary>,
    pub changed_files: Vec<ChangedFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FilePreview {
    Text { content: String, truncated: bool },
    Image { data: String, mime_type: String },
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
        repository_summary(&root_path)
    }

    pub fn repository_details(path: &Path) -> Result<RepositoryDetails> {
        let root_path = repository_root(path)?;
        Ok(RepositoryDetails {
            summary: repository_summary(&root_path)?,
            recent_commits: recent_commits(&root_path)?,
            changed_files: changed_files(&root_path)?,
        })
    }

    pub fn file_diff(path: &Path, file_path: &str) -> Result<Option<String>> {
        let root_path = repository_root(path)?;
        validate_repository_relative_path(file_path)?;
        if !changed_files(&root_path)?
            .iter()
            .any(|file| file.path == file_path)
        {
            return Err(GitError(
                "The file is not changed in this repository.".into(),
            ));
        }
        if run_git_if_success(&root_path, ["rev-parse", "--verify", "HEAD"])?.is_none() {
            return Ok(None);
        }

        let diff = run_git(
            &root_path,
            [
                "diff",
                "--no-ext-diff",
                "--no-color",
                "HEAD",
                "--",
                file_path,
            ],
        )?;
        Ok((!diff.is_empty()).then(|| truncate_diff(diff)))
    }

    pub fn file_preview(path: &Path, file_path: &str) -> Result<Option<FilePreview>> {
        const MAX_TEXT_PREVIEW_BYTES: usize = 100_000;
        const MAX_IMAGE_PREVIEW_BYTES: usize = 5 * 1024 * 1024;

        let root_path = repository_root(path)?;
        validate_repository_relative_path(file_path)?;
        let file_path = root_path.join(file_path);
        let metadata = match fs::metadata(&file_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(GitError(format!("Unable to inspect the file: {error}"))),
        };
        if !metadata.is_file() {
            return Ok(None);
        }
        let canonical_file = fs::canonicalize(&file_path)
            .map_err(|error| GitError(format!("Unable to resolve the file path: {error}")))?;
        if !canonical_file.starts_with(&root_path) {
            return Err(GitError(
                "The file path must stay within the repository.".into(),
            ));
        }

        let bytes = fs::read(&canonical_file)
            .map_err(|error| GitError(format!("Unable to read the file: {error}")))?;
        if let Some(mime_type) = image_mime_type(&bytes) {
            if bytes.len() > MAX_IMAGE_PREVIEW_BYTES {
                return Err(GitError(
                    "Images larger than 5 MB cannot be previewed.".into(),
                ));
            }
            return Ok(Some(FilePreview::Image {
                data: STANDARD.encode(bytes),
                mime_type: mime_type.into(),
            }));
        }
        if bytes.contains(&0) {
            return Err(GitError("Binary files cannot be previewed.".into()));
        }
        let truncated = bytes.len() > MAX_TEXT_PREVIEW_BYTES;
        let preview = &bytes[..bytes.len().min(MAX_TEXT_PREVIEW_BYTES)];
        let mut content = String::from_utf8_lossy(preview).into_owned();
        if truncated {
            content.push_str("\n\n[Preview truncated after 100 KB]");
        }
        Ok(Some(FilePreview::Text { content, truncated }))
    }
}

fn image_mime_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn repository_summary(root_path: &Path) -> Result<RepositorySummary> {
    let current_branch = non_empty(run_git(root_path, ["branch", "--show-current"])?);
    let default_branch = default_branch(root_path, current_branch.as_deref())?;
    let status = run_git(root_path, ["status", "--porcelain=v1"])?;
    let latest_commit = latest_commit(root_path)?;

    Ok(RepositorySummary {
        root_path: root_path.to_string_lossy().into_owned(),
        default_branch,
        current_branch,
        is_clean: status.trim().is_empty(),
        changed_file_count: status.lines().count(),
        latest_commit,
    })
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
    Ok(recent_commits(path)?.into_iter().next())
}

fn recent_commits(path: &Path) -> Result<Vec<CommitSummary>> {
    let Some(output) = run_git_if_success(
        path,
        ["log", "-12", "--format=%H%x1f%h%x1f%an%x1f%aI%x1f%s"],
    )?
    else {
        return Ok(Vec::new());
    };

    Ok(output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\u{1f}');
            Some(CommitSummary {
                hash: fields.next()?.to_owned(),
                short_hash: fields.next()?.to_owned(),
                author: fields.next()?.to_owned(),
                authored_at: fields.next()?.to_owned(),
                subject: fields.next()?.to_owned(),
            })
        })
        .collect())
}

fn changed_files(path: &Path) -> Result<Vec<ChangedFile>> {
    let output = run_git(path, ["status", "--porcelain=v1", "-z"])?;
    let mut entries = output.split('\0');
    let mut files = Vec::new();

    while let Some(entry) = entries.next() {
        if entry.is_empty() {
            continue;
        }
        let status = entry
            .get(..2)
            .ok_or_else(|| GitError("Git returned an invalid file status.".into()))?;
        let file_path = entry
            .get(3..)
            .ok_or_else(|| GitError("Git returned an invalid changed-file path.".into()))?;
        files.push(ChangedFile {
            path: file_path.to_owned(),
            status: status.to_owned(),
        });
        if status.contains('R') || status.contains('C') {
            entries.next();
        }
    }
    Ok(files)
}

fn validate_repository_relative_path(file_path: &str) -> Result<()> {
    let path = Path::new(file_path);
    if file_path.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(GitError(
            "The file path must stay within the repository.".into(),
        ));
    }
    Ok(())
}

fn truncate_diff(mut diff: String) -> String {
    const MAX_DIFF_BYTES: usize = 100_000;
    if diff.len() <= MAX_DIFF_BYTES {
        return diff;
    }
    let boundary = diff
        .char_indices()
        .take_while(|(index, _)| *index <= MAX_DIFF_BYTES)
        .map(|(index, _)| index)
        .last()
        .unwrap_or_default();
    diff.truncate(boundary);
    diff.push_str("\n\n[Diff truncated after 100 KB]");
    diff
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
    use super::{FilePreview, GitService};
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn run_git(path: &std::path::Path, arguments: &[&str]) {
        let output = Command::new("git")
            .args(arguments)
            .current_dir(path)
            .output()
            .expect("git command starts");
        assert!(
            output.status.success(),
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

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

    #[test]
    fn exposes_recent_commits_changed_files_and_file_diffs() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("orchestr-git-details-{nonce}"));
        fs::create_dir(&directory).expect("temporary directory creates");
        GitService::initialize_repository(&directory).expect("repository initializes");

        fs::write(directory.join("README.md"), "initial\n").expect("file writes");
        run_git(&directory, &["add", "README.md"]);
        run_git(
            &directory,
            &[
                "-c",
                "user.name=Orchestr Test",
                "-c",
                "user.email=orchestr@example.test",
                "commit",
                "-m",
                "Initial project",
            ],
        );

        fs::write(directory.join("README.md"), "initial\nchanged\n").expect("file updates");
        fs::write(directory.join("new-file.txt"), "untracked\n").expect("file writes");

        let details = GitService::repository_details(&directory).expect("details load");
        assert!(!details.summary.is_clean);
        assert_eq!(details.summary.changed_file_count, 2);
        assert_eq!(details.recent_commits[0].subject, "Initial project");
        assert!(details
            .changed_files
            .iter()
            .any(|file| file.path == "README.md"));
        assert!(details
            .changed_files
            .iter()
            .any(|file| file.path == "new-file.txt"));

        let diff = GitService::file_diff(&directory, "README.md")
            .expect("diff loads")
            .expect("tracked file has a diff");
        assert!(diff.contains("+changed"));
        assert!(GitService::file_diff(&directory, "../outside.txt").is_err());
        let preview = GitService::file_preview(&directory, "new-file.txt")
            .expect("file preview loads")
            .expect("file exists");
        assert!(matches!(preview, FilePreview::Text { content, .. } if content == "untracked\n"));

        fs::write(
            directory.join("diagram.png"),
            [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
        )
        .expect("image writes");
        let preview = GitService::file_preview(&directory, "diagram.png")
            .expect("image preview loads")
            .expect("image exists");
        assert!(
            matches!(preview, FilePreview::Image { mime_type, .. } if mime_type == "image/png")
        );
        assert!(GitService::file_preview(&directory, "../outside.txt").is_err());

        fs::remove_dir_all(directory).expect("temporary directory removes");
    }
}
