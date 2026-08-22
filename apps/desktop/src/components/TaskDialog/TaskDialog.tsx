import { X } from "lucide-react";
import { FormEvent, useState } from "react";
import type { Task } from "../../services/tasks";
import "./TaskDialog.css";

type TaskDialogProps = {
  task?: Task;
  onClose: () => void;
  onSave: (input: { title: string; description: string }) => Promise<void>;
};

export function TaskDialog({ task, onClose, onSave }: TaskDialogProps) {
  const [title, setTitle] = useState(task?.title ?? "");
  const [description, setDescription] = useState(task?.description ?? "");
  const [error, setError] = useState<string>();
  const [isSaving, setIsSaving] = useState(false);

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError(undefined);
    setIsSaving(true);
    try {
      await onSave({ title, description });
      onClose();
    } catch (saveError) {
      setError(saveError instanceof Error ? saveError.message : "Unable to save task.");
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="task-dialog" role="dialog" aria-modal="true" aria-labelledby="task-dialog-title" onMouseDown={(event) => event.stopPropagation()}>
        <header className="dialog-header">
          <div>
            <p className="eyebrow">Project task</p>
            <h2 id="task-dialog-title">{task ? "Edit task" : "New task"}</h2>
          </div>
          <button className="icon-button" type="button" onClick={onClose} aria-label="Close dialog"><X size={16} /></button>
        </header>
        <form onSubmit={submit}>
          <label>
            Title
            <input value={title} onChange={(event) => setTitle(event.target.value)} required autoFocus maxLength={200} placeholder="Describe the work" />
          </label>
          <label>
            Description <span className="field-optional">optional</span>
            <textarea value={description} onChange={(event) => setDescription(event.target.value)} rows={6} placeholder="Implementation notes, context, or expected outcome" />
          </label>
          {error && <p className="form-error" role="alert">{error}</p>}
          <footer className="dialog-actions">
            <button type="button" className="secondary-button" onClick={onClose}>Cancel</button>
            <button type="submit" className="primary-button" disabled={isSaving}>{isSaving ? "Saving…" : task ? "Save task" : "Create task"}</button>
          </footer>
        </form>
      </section>
    </div>
  );
}
