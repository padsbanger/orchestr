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
#[serde(rename_all = "camelCase")]
pub struct TaskReview {
    pub branch: String,
    pub base_branch: String,
    pub commits: Vec<CommitSummary>,
    pub diff: String,
    pub changed_files: Vec<ChangedFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FilePreview {
    Text { content: String, truncated: bool },
    Image { data: String, mime_type: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskWorktree {
    pub branch: String,
    pub path: PathBuf,
    pub created_branch: bool,
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

    pub fn create_initial_commit(path: &Path) -> Result<RepositorySummary> {
        let root_path = repository_root(path)?;
        if run_git_if_success(&root_path, ["rev-parse", "--verify", "HEAD"])?.is_some() {
            return Err(GitError(
                "The repository already has a commit and does not need initialization.".into(),
            ));
        }

        let mut arguments: Vec<String> = Vec::new();
        if run_git_if_success(&root_path, ["config", "--get", "user.name"])?
            .and_then(non_empty)
            .is_none()
        {
            arguments.extend(["-c".into(), "user.name=Orchestr".into()]);
        }
        if run_git_if_success(&root_path, ["config", "--get", "user.email"])?
            .and_then(non_empty)
            .is_none()
        {
            arguments.extend(["-c".into(), "user.email=orchestr@local".into()]);
        }
        arguments.extend([
            "commit".into(),
            "--allow-empty".into(),
            "--message".into(),
            "chore: initialize project".into(),
        ]);
        run_git(&root_path, arguments)?;
        Self::inspect_repository(&root_path)
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

    pub fn task_review(path: &Path, base_branch: &str) -> Result<TaskReview> {
        let root_path = repository_root(path)?;
        let branch = run_git(&root_path, ["branch", "--show-current"])?;
        let branch = non_empty(branch)
            .ok_or_else(|| GitError("The task worktree is not on a branch.".into()))?;
        run_git(&root_path, ["rev-parse", "--verify", base_branch])?;
        let commits = commits_between(&root_path, base_branch)?;
        let committed_diff = run_git(
            &root_path,
            [
                "diff",
                "--no-ext-diff",
                "--no-color",
                &format!("{base_branch}...HEAD"),
            ],
        )?;
        let working_diff = run_git(&root_path, ["diff", "--no-ext-diff", "--no-color", "HEAD"])?;
        let mut diff = String::new();
        if !committed_diff.is_empty() {
            diff.push_str("# Committed branch changes\n\n");
            diff.push_str(&committed_diff);
        }
        if !working_diff.is_empty() {
            if !diff.is_empty() {
                diff.push_str("\n\n");
            }
            diff.push_str("# Uncommitted worktree changes\n\n");
            diff.push_str(&working_diff);
        }
        Ok(TaskReview {
            branch,
            base_branch: base_branch.to_owned(),
            commits,
            diff: truncate_diff(diff),
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

    pub fn create_task_worktree(
        repository_path: &Path,
        worktree_path: &Path,
        branch: &str,
        base_branch: &str,
    ) -> Result<TaskWorktree> {
        let root_path = repository_root(repository_path)?;
        if run_git_if_success(&root_path, ["rev-parse", "--verify", "HEAD"])?.is_none() {
            return Err(GitError(
                "The repository has no commits yet. Create an initial commit before starting an isolated task run."
                    .into(),
            ));
        }
        if !worktree_path.is_absolute() {
            return Err(GitError("The worktree path must be absolute.".into()));
        }
        if worktree_path.exists() {
            return Err(GitError(format!(
                "The task worktree path already exists: {}",
                worktree_path.display()
            )));
        }
        run_git(&root_path, ["check-ref-format", "--branch", branch])?;

        let parent = worktree_path.parent().ok_or_else(|| {
            GitError("The task worktree path must have a parent directory.".into())
        })?;
        fs::create_dir_all(parent).map_err(|error| {
            GitError(format!(
                "Unable to create the task worktree directory {}: {error}",
                parent.display()
            ))
        })?;

        let branch_ref = format!("refs/heads/{branch}");
        let branch_exists = run_git_if_success(
            &root_path,
            ["show-ref", "--verify", "--quiet", branch_ref.as_str()],
        )?
        .is_some();
        let mut arguments: Vec<std::ffi::OsString> = vec!["worktree".into(), "add".into()];
        if !branch_exists {
            arguments.extend(["-b".into(), branch.into()]);
        }
        arguments.push(git_argument_path(worktree_path).into());
        arguments.push(if branch_exists {
            branch.into()
        } else {
            base_branch.into()
        });
        run_git(&root_path, arguments)?;

        Ok(TaskWorktree {
            branch: branch.to_owned(),
            path: canonical_directory(worktree_path)?,
            created_branch: !branch_exists,
        })
    }

    pub fn remove_task_worktree(repository_path: &Path, worktree_path: &Path) -> Result<()> {
        let root_path = repository_root(repository_path)?;
        let worktree_path = canonical_directory(worktree_path)?;
        if worktree_path == root_path {
            return Err(GitError(
                "The primary repository checkout cannot be removed as a task worktree.".into(),
            ));
        }
        let registered = run_git(&root_path, ["worktree", "list", "--porcelain"])?
            .lines()
            .filter_map(|line| line.strip_prefix("worktree "))
            .map(PathBuf::from)
            .filter_map(|path| path.canonicalize().ok())
            .any(|path| path == worktree_path);
        if !registered {
            return Err(GitError(
                "The path is not a registered worktree for this repository.".into(),
            ));
        }
        run_git(
            &root_path,
            Vec::<std::ffi::OsString>::from([
                "worktree".into(),
                "remove".into(),
                git_argument_path(&worktree_path),
            ]),
        )?;
        Ok(())
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

fn git_argument_path(path: &Path) -> std::ffi::OsString {
    #[cfg(windows)]
    {
        let path = path.to_string_lossy();
        if let Some(path) = path.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{path}").into();
        }
        return path.strip_prefix(r"\\?\").unwrap_or(&path).into();
    }

    #[cfg(not(windows))]
    {
        path.as_os_str().to_owned()
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

    Ok(parse_commits(&output))
}

fn commits_between(path: &Path, base_branch: &str) -> Result<Vec<CommitSummary>> {
    let Some(output) = run_git_if_success(
        path,
        [
            "log",
            "--format=%H%x1f%h%x1f%an%x1f%aI%x1f%s",
            &format!("{base_branch}..HEAD"),
        ],
    )?
    else {
        return Ok(Vec::new());
    };
    Ok(parse_commits(&output))
}

fn parse_commits(output: &str) -> Vec<CommitSummary> {
    output
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
        .collect()
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
        assert!(GitService::create_task_worktree(
            &directory,
            &std::env::temp_dir().join(format!("orchestr-git-empty-worktree-{nonce}")),
            "task/empty-repository",
            &repository.default_branch,
        )
        .expect_err("empty repository cannot create an isolated worktree")
        .to_string()
        .contains("no commits yet"));

        let initialized =
            GitService::create_initial_commit(&directory).expect("initial project commit creates");
        assert_eq!(
            initialized
                .latest_commit
                .as_ref()
                .map(|commit| commit.subject.as_str()),
            Some("chore: initialize project")
        );
        assert!(initialized.is_clean);

        fs::remove_dir_all(directory).expect("temporary directory removes");
    }

    #[cfg(windows)]
    #[test]
    fn converts_windows_extended_paths_before_passing_them_to_git() {
        assert_eq!(
            super::git_argument_path(std::path::Path::new(
                r"\\?\C:\Users\konta\Projects\worktree"
            )),
            std::ffi::OsString::from(r"C:\Users\konta\Projects\worktree")
        );
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

        let worktree_path = std::env::temp_dir().join(format!("orchestr-git-worktree-{nonce}"));
        let worktree = GitService::create_task_worktree(
            &directory,
            &worktree_path,
            "task/test-worktree",
            &details.summary.default_branch,
        )
        .expect("task worktree creates");
        assert!(worktree.created_branch);
        assert_eq!(
            GitService::inspect_repository(&worktree.path)
                .expect("worktree inspects")
                .current_branch
                .as_deref(),
            Some("task/test-worktree")
        );
        GitService::remove_task_worktree(&directory, &worktree.path)
            .expect("task worktree removes");
        assert!(!worktree_path.exists());

        fs::remove_dir_all(directory).expect("temporary directory removes");
    }
}
