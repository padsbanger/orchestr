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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrationResult {
    Merged { commit: String },
    Conflict { paths: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrationPreparation {
    Ready,
    Conflict { paths: Vec<String> },
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

        commit_with_fallback_identity(&root_path, "chore: initialize project", true)?;
        Self::inspect_repository(&root_path)
    }

    pub fn inspect_repository(path: &Path) -> Result<RepositorySummary> {
        let root_path = repository_root(path)?;
        repository_summary(&root_path)
    }

    /// Returns every Git metadata directory that must be writable for commands
    /// executed from this checkout.
    ///
    /// Linked worktrees keep their index and HEAD locks in a private directory
    /// under the primary repository's common `.git` database. Windows sandbox
    /// ACLs require that private directory to be granted explicitly, even when
    /// its parent common directory is already writable.
    pub fn writable_git_directories(path: &Path) -> Result<Vec<PathBuf>> {
        let root_path = repository_root(path)?;
        let worktree_directory = resolve_git_directory(&root_path, "--git-dir")?;
        let common_directory = resolve_git_directory(&root_path, "--git-common-dir")?;
        let mut directories = vec![worktree_directory];
        if !directories.contains(&common_directory) {
            directories.push(common_directory);
        }
        Ok(directories)
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

    pub fn squash_integrate_task(
        repository_path: &Path,
        task_worktree_path: &Path,
        source_branch: &str,
        target_branch: &str,
        message: &str,
    ) -> Result<IntegrationResult> {
        let repository_path = repository_root(repository_path)?;
        let task_worktree_path = repository_root(task_worktree_path)?;
        if let IntegrationPreparation::Conflict { paths } = Self::prepare_task_for_integration(
            &repository_path,
            &task_worktree_path,
            source_branch,
            target_branch,
        )? {
            return Ok(IntegrationResult::Conflict { paths });
        }

        if let Err(error) = run_git(
            &repository_path,
            ["merge", "--squash", "--no-commit", source_branch],
        ) {
            let paths = conflicted_paths(&repository_path)?;
            if !paths.is_empty() {
                run_git(&repository_path, ["reset", "--merge"])?;
                return Ok(IntegrationResult::Conflict { paths });
            }
            return Err(error);
        }
        if run_git_if_success(&repository_path, ["diff", "--cached", "--quiet"])?.is_some() {
            return Err(GitError(
                "The task branch has no changes to integrate into the current integration branch."
                    .into(),
            ));
        }
        if let Err(error) = commit_with_fallback_identity(&repository_path, message, false) {
            run_git(&repository_path, ["reset", "--merge"])?;
            return Err(error);
        }
        let commit = run_git(&repository_path, ["rev-parse", "HEAD"])?;
        Ok(IntegrationResult::Merged {
            commit: commit.trim().to_owned(),
        })
    }

    /// Rebase a task branch onto the current integration branch before its
    /// integration quality gates execute. This is intentionally separate from
    /// the squash merge so validation observes the exact branch to be merged.
    pub fn prepare_task_for_integration(
        repository_path: &Path,
        task_worktree_path: &Path,
        source_branch: &str,
        target_branch: &str,
    ) -> Result<IntegrationPreparation> {
        let repository_path = repository_root(repository_path)?;
        let task_worktree_path = repository_root(task_worktree_path)?;
        run_git(
            &repository_path,
            ["check-ref-format", "--branch", source_branch],
        )?;
        run_git(
            &repository_path,
            ["check-ref-format", "--branch", target_branch],
        )?;
        if source_branch == target_branch {
            return Err(GitError(
                "A task branch cannot integrate into itself.".into(),
            ));
        }
        ensure_clean_integration_workspace(&repository_path, target_branch)?;
        ensure_task_worktree_ready(&task_worktree_path, source_branch)?;
        if run_git_if_success(
            &repository_path,
            ["merge-base", "--is-ancestor", target_branch, source_branch],
        )?
        .is_none()
        {
            if let Err(error) = run_git(&task_worktree_path, ["rebase", target_branch]) {
                let paths = conflicted_paths(&task_worktree_path)?;
                if !paths.is_empty() {
                    return Ok(IntegrationPreparation::Conflict { paths });
                }
                return Err(error);
            }
        }
        Ok(IntegrationPreparation::Ready)
    }

    pub fn delete_integrated_task_branch(repository_path: &Path, branch: &str) -> Result<()> {
        let repository_path = repository_root(repository_path)?;
        run_git(&repository_path, ["check-ref-format", "--branch", branch])?;
        let current_branch = non_empty(run_git(&repository_path, ["branch", "--show-current"])?);
        if current_branch.as_deref() == Some(branch) {
            return Err(GitError(
                "The current integration workspace cannot delete its checked-out branch.".into(),
            ));
        }
        run_git(&repository_path, ["branch", "--delete", "--force", branch])?;
        Ok(())
    }

    pub fn delete_task_branch_if_exists(repository_path: &Path, branch: &str) -> Result<()> {
        let repository_path = repository_root(repository_path)?;
        let branch_ref = format!("refs/heads/{branch}");
        if run_git_if_success(
            &repository_path,
            ["show-ref", "--verify", "--quiet", branch_ref.as_str()],
        )?
        .is_none()
        {
            return Ok(());
        }
        Self::delete_integrated_task_branch(&repository_path, branch)
    }

    pub fn revert_integration_commit(
        repository_path: &Path,
        target_branch: &str,
        commit: &str,
    ) -> Result<String> {
        let repository_path = repository_root(repository_path)?;
        ensure_clean_integration_workspace(&repository_path, target_branch)?;
        run_git(
            &repository_path,
            ["rev-parse", "--verify", &format!("{commit}^{{commit}}")],
        )?;
        if run_git_if_success(
            &repository_path,
            ["merge-base", "--is-ancestor", commit, target_branch],
        )?
        .is_none()
        {
            return Err(GitError(
                "The integration commit is not part of the configured integration branch.".into(),
            ));
        }
        let mut arguments = git_identity_arguments(&repository_path)?;
        arguments.extend(["revert".into(), "--no-edit".into(), commit.into()]);
        if let Err(error) = run_git(&repository_path, arguments) {
            let _ = run_git(&repository_path, ["revert", "--abort"]);
            return Err(error);
        }
        Ok(run_git(&repository_path, ["rev-parse", "HEAD"])?
            .trim()
            .to_owned())
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

fn ensure_clean_integration_workspace(path: &Path, target_branch: &str) -> Result<()> {
    let current_branch = non_empty(run_git(path, ["branch", "--show-current"])?);
    if current_branch.as_deref() != Some(target_branch) {
        return Err(GitError(format!(
            "The primary workspace must be checked out on {target_branch} before integration."
        )));
    }
    if !run_git(path, ["status", "--porcelain=v1"])?
        .trim()
        .is_empty()
    {
        return Err(GitError(
            "The primary workspace has uncommitted changes. Commit, stash, or discard them before integration."
                .into(),
        ));
    }
    Ok(())
}

fn ensure_task_worktree_ready(path: &Path, source_branch: &str) -> Result<()> {
    let current_branch = non_empty(run_git(path, ["branch", "--show-current"])?);
    if current_branch.as_deref() != Some(source_branch) {
        return Err(GitError(
            "The task worktree is no longer checked out on its recorded task branch.".into(),
        ));
    }
    if !run_git(path, ["status", "--porcelain=v1"])?
        .trim()
        .is_empty()
    {
        return Err(GitError(
            "The task worktree has uncommitted changes. Commit, stash, or discard them before integration."
                .into(),
        ));
    }
    Ok(())
}

fn conflicted_paths(path: &Path) -> Result<Vec<String>> {
    Ok(run_git(path, ["diff", "--name-only", "--diff-filter=U"])?
        .lines()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn commit_with_fallback_identity(path: &Path, message: &str, allow_empty: bool) -> Result<()> {
    let mut arguments = git_identity_arguments(path)?;
    arguments.push("commit".into());
    if allow_empty {
        arguments.push("--allow-empty".into());
    }
    arguments.extend(["--message".into(), message.into()]);
    run_git(path, arguments).map(|_| ())
}

fn git_identity_arguments(path: &Path) -> Result<Vec<String>> {
    let mut arguments = Vec::new();
    if run_git_if_success(path, ["config", "--get", "user.name"])?
        .and_then(non_empty)
        .is_none()
    {
        arguments.extend(["-c".into(), "user.name=Orchestr".into()]);
    }
    if run_git_if_success(path, ["config", "--get", "user.email"])?
        .and_then(non_empty)
        .is_none()
    {
        arguments.extend(["-c".into(), "user.email=orchestr@local".into()]);
    }
    Ok(arguments)
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

fn resolve_git_directory(root_path: &Path, argument: &str) -> Result<PathBuf> {
    let directory = non_empty(run_git(root_path, ["rev-parse", argument])?).ok_or_else(|| {
        GitError(format!(
            "Git did not report its {argument} metadata directory for this repository."
        ))
    })?;
    let directory = PathBuf::from(directory);
    let directory = if directory.is_absolute() {
        directory
    } else {
        root_path.join(directory)
    };
    canonical_directory(&directory)
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
    use super::{FilePreview, GitService, IntegrationResult};
    use std::{
        fs,
        path::Path,
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
        let writable_git_directories = GitService::writable_git_directories(&worktree.path)
            .expect("worktree metadata directories resolve");
        assert_eq!(writable_git_directories.len(), 2);
        assert!(writable_git_directories
            .iter()
            .any(|path| path.ends_with(".git")));
        assert!(writable_git_directories.iter().any(|path| path
            .parent()
            .is_some_and(|parent| parent.ends_with(Path::new(".git/worktrees")))));
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

    #[test]
    fn squash_integrates_a_task_branch_and_retains_a_single_main_commit() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("orchestr-git-integrate-{nonce}"));
        fs::create_dir(&directory).expect("temporary directory creates");
        let repository =
            GitService::initialize_repository(&directory).expect("repository initializes");
        GitService::create_initial_commit(&directory).expect("initial commit creates");
        let worktree_path =
            std::env::temp_dir().join(format!("orchestr-git-integrate-worktree-{nonce}"));
        let worktree = GitService::create_task_worktree(
            &directory,
            &worktree_path,
            "task/integration-test",
            &repository.default_branch,
        )
        .expect("task worktree creates");

        fs::write(worktree.path.join("feature.txt"), "integrated\n").expect("feature writes");
        run_git(&worktree.path, &["add", "feature.txt"]);
        run_git(
            &worktree.path,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.test",
                "commit",
                "-m",
                "Implement feature",
            ],
        );

        let result = GitService::squash_integrate_task(
            &directory,
            &worktree.path,
            "task/integration-test",
            &repository.default_branch,
            "task: integrate feature",
        )
        .expect("integration completes");
        assert!(matches!(result, IntegrationResult::Merged { .. }));
        assert_eq!(
            fs::read_to_string(directory.join("feature.txt"))
                .expect("integrated file reads")
                .replace("\r\n", "\n"),
            "integrated\n"
        );
        assert_eq!(
            GitService::repository_details(&directory)
                .expect("repository details load")
                .recent_commits[0]
                .subject,
            "task: integrate feature"
        );

        GitService::remove_task_worktree(&directory, &worktree.path)
            .expect("task worktree removes");
        GitService::delete_integrated_task_branch(&directory, "task/integration-test")
            .expect("task branch removes");
        GitService::delete_task_branch_if_exists(&directory, "task/integration-test")
            .expect("already removed task branch is safe to clean again");
        fs::remove_dir_all(directory).expect("temporary directory removes");
    }

    #[test]
    fn revert_integration_commit_creates_normal_history() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("orchestr-git-revert-{nonce}"));
        fs::create_dir(&directory).expect("temporary directory creates");
        let repository =
            GitService::initialize_repository(&directory).expect("repository initializes");
        GitService::create_initial_commit(&directory).expect("initial commit creates");
        fs::write(directory.join("regression.txt"), "bad change\n").expect("regression writes");
        run_git(&directory, &["add", "regression.txt"]);
        run_git(
            &directory,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.test",
                "commit",
                "-m",
                "Regressing integration",
            ],
        );
        let original_commit = GitService::repository_details(&directory)
            .expect("details load")
            .recent_commits[0]
            .hash
            .clone();

        let revert_commit = GitService::revert_integration_commit(
            &directory,
            &repository.default_branch,
            &original_commit,
        )
        .expect("integration reverts");

        assert!(!directory.join("regression.txt").exists());
        assert_ne!(revert_commit, original_commit);
        let history = GitService::repository_details(&directory)
            .expect("history loads")
            .recent_commits;
        assert_eq!(history.len(), 3);
        assert!(history[0].subject.starts_with("Revert"));
        fs::remove_dir_all(directory).expect("temporary directory removes");
    }

    #[test]
    fn integration_conflicts_preserve_the_task_worktree_and_primary_workspace() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("orchestr-git-conflict-{nonce}"));
        fs::create_dir(&directory).expect("temporary directory creates");
        let repository =
            GitService::initialize_repository(&directory).expect("repository initializes");
        fs::write(directory.join("shared.txt"), "base\n").expect("base file writes");
        run_git(&directory, &["add", "shared.txt"]);
        run_git(
            &directory,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.test",
                "commit",
                "-m",
                "Base",
            ],
        );
        let worktree_path =
            std::env::temp_dir().join(format!("orchestr-git-conflict-worktree-{nonce}"));
        let worktree = GitService::create_task_worktree(
            &directory,
            &worktree_path,
            "task/conflict-test",
            &repository.default_branch,
        )
        .expect("task worktree creates");
        fs::write(worktree.path.join("shared.txt"), "task change\n").expect("task file writes");
        run_git(&worktree.path, &["add", "shared.txt"]);
        run_git(
            &worktree.path,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.test",
                "commit",
                "-m",
                "Task change",
            ],
        );
        fs::write(directory.join("shared.txt"), "main change\n").expect("main file writes");
        run_git(&directory, &["add", "shared.txt"]);
        run_git(
            &directory,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.test",
                "commit",
                "-m",
                "Main change",
            ],
        );

        let result = GitService::squash_integrate_task(
            &directory,
            &worktree.path,
            "task/conflict-test",
            &repository.default_branch,
            "task: conflict test",
        )
        .expect("conflict is an integration result");
        assert!(matches!(result, IntegrationResult::Conflict { paths } if paths == ["shared.txt"]));
        assert_eq!(
            fs::read_to_string(directory.join("shared.txt")).expect("primary file reads"),
            "main change\n"
        );
        run_git(&worktree.path, &["rebase", "--abort"]);
        GitService::remove_task_worktree(&directory, &worktree.path)
            .expect("task worktree removes");
        fs::remove_dir_all(directory).expect("temporary directory removes");
    }
}
