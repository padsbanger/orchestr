import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, GitFork, HardDriveDownload, X } from "lucide-react";
import { FormEvent, useState } from "react";
import { createProject, registerProject } from "../../services/projects";
import "./NewProjectDialog.css";

type ProjectMode = "new" | "existing";

type NewProjectDialogProps = {
  onCreated: () => void;
  onClose: () => void;
};

export function NewProjectDialog({ onCreated, onClose }: NewProjectDialogProps) {
  const [mode, setMode] = useState<ProjectMode>("new");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [parentPath, setParentPath] = useState("");
  const [directoryName, setDirectoryName] = useState("");
  const [repositoryPath, setRepositoryPath] = useState("");
  const [error, setError] = useState<string>();
  const [isSubmitting, setIsSubmitting] = useState(false);

  const chooseDirectory = async (target: "parent" | "repository") => {
    try {
      const selection = await open({ directory: true, multiple: false, title: target === "parent" ? "Choose parent directory" : "Choose Git repository" });
      if (typeof selection === "string") {
        if (target === "parent") setParentPath(selection);
        else setRepositoryPath(selection);
      }
    } catch (dialogError) {
      setError(dialogError instanceof Error ? dialogError.message : "Unable to open the directory picker.");
    }
  };

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError(undefined);
    setIsSubmitting(true);
    try {
      if (mode === "new") {
        await createProject({ name, description, parentPath, directoryName: directoryName || slugify(name) });
      } else {
        await registerProject({ name, description, path: repositoryPath });
      }
      onCreated();
    } catch (projectError) {
      setError(projectError instanceof Error ? projectError.message : "Unable to save the project.");
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="project-dialog" role="dialog" aria-modal="true" aria-labelledby="new-project-title" onMouseDown={(event) => event.stopPropagation()}>
        <header className="dialog-header">
          <div>
            <p className="eyebrow">Project registry</p>
            <h2 id="new-project-title">Add project</h2>
          </div>
          <button className="icon-button" type="button" onClick={onClose} aria-label="Close dialog"><X size={16} /></button>
        </header>

        <div className="mode-switch" role="tablist" aria-label="Project source">
          <button type="button" role="tab" aria-selected={mode === "new"} className={mode === "new" ? "active" : ""} onClick={() => setMode("new")}>
            <GitFork size={15} /> New repository
          </button>
          <button type="button" role="tab" aria-selected={mode === "existing"} className={mode === "existing" ? "active" : ""} onClick={() => setMode("existing")}>
            <HardDriveDownload size={15} /> Existing repository
          </button>
        </div>

        <form onSubmit={submit}>
          <label>
            Project name
            <input value={name} onChange={(event) => setName(event.target.value)} autoFocus required maxLength={120} placeholder="Trading Bot" />
          </label>
          <label>
            Description <span className="field-optional">optional</span>
            <textarea value={description} onChange={(event) => setDescription(event.target.value)} rows={3} placeholder="What this project is responsible for" />
          </label>

          {mode === "new" ? (
            <>
              <label>
                Parent directory
                <div className="path-input"><input value={parentPath} onChange={(event) => setParentPath(event.target.value)} required placeholder="/projects" /><button type="button" onClick={() => void chooseDirectory("parent")} aria-label="Choose parent directory"><FolderOpen size={15} /></button></div>
              </label>
              <label>
                Repository folder
                <input value={directoryName} onChange={(event) => setDirectoryName(event.target.value)} placeholder={slugify(name) || "trading-bot"} />
                <span className="field-hint">A new empty folder and Git repository will be created here.</span>
              </label>
            </>
          ) : (
            <label>
              Local Git repository
              <div className="path-input"><input value={repositoryPath} onChange={(event) => setRepositoryPath(event.target.value)} required placeholder="/projects/trading-bot" /><button type="button" onClick={() => void chooseDirectory("repository")} aria-label="Choose Git repository"><FolderOpen size={15} /></button></div>
              <span className="field-hint">Orchestr validates the selected repository before registering it.</span>
            </label>
          )}

          {error && <p className="form-error" role="alert">{error}</p>}
          <footer className="dialog-actions">
            <button type="button" className="secondary-button" onClick={onClose}>Cancel</button>
            <button type="submit" className="primary-button" disabled={isSubmitting}>{isSubmitting ? "Saving…" : mode === "new" ? "Create project" : "Register project"}</button>
          </footer>
        </form>
      </section>
    </div>
  );
}

function slugify(value: string) {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}
