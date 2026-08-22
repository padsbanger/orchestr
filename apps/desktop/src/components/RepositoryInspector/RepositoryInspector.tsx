import { FileCode2, GitCommitHorizontal, RefreshCw, X } from "lucide-react";
import { useState } from "react";
import { getRepositoryDiff, getRepositoryFilePreview, type ChangedFile, type FilePreview, type RepositoryDetails } from "../../services/projects";
import "./RepositoryInspector.css";

type RepositoryInspectorProps = {
  projectId: string;
  repository?: RepositoryDetails;
  error?: string;
  isLoading: boolean;
  onClose: () => void;
  onRefresh: () => void;
};

export function RepositoryInspector({
  projectId,
  repository,
  error,
  isLoading,
  onClose,
  onRefresh,
}: RepositoryInspectorProps) {
  const [selectedFile, setSelectedFile] = useState<ChangedFile>();
  const [diff, setDiff] = useState<string | null>();
  const [preview, setPreview] = useState<FilePreview | null>();
  const [diffError, setDiffError] = useState<string>();
  const [previewError, setPreviewError] = useState<string>();
  const [isLoadingDiff, setIsLoadingDiff] = useState(false);
  const [fileView, setFileView] = useState<"preview" | "diff">("preview");

  const selectFile = async (file: ChangedFile) => {
    setSelectedFile(file);
    setDiff(undefined);
    setPreview(undefined);
    setDiffError(undefined);
    setPreviewError(undefined);
    setFileView("preview");
    setIsLoadingDiff(true);
    const [diffResult, previewResult] = await Promise.allSettled([getRepositoryDiff(projectId, file.path), getRepositoryFilePreview(projectId, file.path)]);
    if (diffResult.status === "fulfilled") setDiff(diffResult.value); else setDiffError(diffResult.reason instanceof Error ? diffResult.reason.message : "Unable to load the file diff.");
    if (previewResult.status === "fulfilled") setPreview(previewResult.value); else setPreviewError(previewResult.reason instanceof Error ? previewResult.reason.message : "Unable to preview the file.");
    setIsLoadingDiff(false);
  };

  return (
    <aside className="repository-inspector" aria-label="Repository inspector">
      <header className="repository-inspector-header">
        <div>
          <p className="eyebrow">Repository awareness</p>
          <h2>Repository activity</h2>
        </div>
        <div className="repository-inspector-actions">
          <button className="icon-button" type="button" onClick={onRefresh} disabled={isLoading} aria-label="Refresh repository activity">
            <RefreshCw size={16} className={isLoading ? "spin" : undefined} />
          </button>
          <button className="icon-button" type="button" onClick={onClose} aria-label="Close repository inspector"><X size={16} /></button>
        </div>
      </header>

      {error && <p className="repository-error" role="alert">{error}</p>}
      {isLoading && !repository ? <p className="repository-loading">Inspecting local repositoryâ€¦</p> : repository && (
        <div className="repository-inspector-content">
          <section className="repository-overview" aria-label="Repository summary">
            <div><span>Branch</span><code>{repository.summary.currentBranch ?? repository.summary.defaultBranch}</code></div>
            <div><span>Working tree</span><strong className={repository.summary.isClean ? "repository-clean" : "repository-dirty"}>{repository.summary.isClean ? "Clean" : `${repository.summary.changedFileCount} changed`}</strong></div>
            <div><span>Latest commit</span><code>{repository.summary.latestCommit?.shortHash ?? "No commits"}</code></div>
          </section>

          <section className="repository-section">
            <h3><FileCode2 size={14} /> Changed files <span>{repository.changedFiles.length}</span></h3>
            {repository.changedFiles.length === 0 ? <p className="repository-empty">Working tree is clean.</p> : (
              <div className="changed-file-list">
                {repository.changedFiles.map((file) => (
                  <button key={`${file.status}:${file.path}`} type="button" className={selectedFile?.path === file.path ? "changed-file active" : "changed-file"} onClick={() => void selectFile(file)}>
                    <span>{describeStatus(file.status)}</span><code>{file.path}</code>
                  </button>
                ))}
              </div>
            )}
          </section>

          {selectedFile && <section className="repository-section diff-section">
            <h3><FileCode2 size={14} /> {selectedFile.path}</h3>
            <div className="file-view-tabs"><button type="button" className={fileView === "preview" ? "active" : ""} onClick={() => setFileView("preview")}>Preview</button><button type="button" className={fileView === "diff" ? "active" : ""} onClick={() => setFileView("diff")}>Diff</button></div>
            {isLoadingDiff ? <p className="repository-empty">Loading file activity...</p> : fileView === "preview" ? previewError ? <p className="repository-error" role="alert">{previewError}</p> : preview ? preview.kind === "image" ? <img className="file-image-preview" src={`data:${preview.mimeType};base64,${preview.data}`} alt={`Preview of ${selectedFile.path}`} /> : <pre className="diff-output file-preview-output">{preview.content}</pre> : <p className="repository-empty">This file is no longer available in the working tree.</p> : diffError ? <p className="repository-error" role="alert">{diffError}</p> : diff ? <pre className="diff-output">{diff}</pre> : <p className="repository-empty">No Git diff is available for this file yet.</p>}
          </section>}

          <section className="repository-section">
            <h3><GitCommitHorizontal size={14} /> Recent commits <span>{repository.recentCommits.length}</span></h3>
            {repository.recentCommits.length === 0 ? <p className="repository-empty">No commits yet.</p> : <ol className="commit-list">
              {repository.recentCommits.map((commit) => <li key={commit.hash}><code>{commit.shortHash}</code><div><strong>{commit.subject}</strong><span>{commit.author} / {formatCommitDate(commit.authoredAt)}</span></div></li>)}
            </ol>}
          </section>
        </div>
      )}
    </aside>
  );
}

function describeStatus(status: string) {
  if (status === "??") return "Untracked";
  if (status.includes("A")) return "Added";
  if (status.includes("D")) return "Deleted";
  if (status.includes("R")) return "Renamed";
  return "Modified";
}

function formatCommitDate(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(date);
}
