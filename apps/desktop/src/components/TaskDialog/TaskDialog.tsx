import { X } from "lucide-react";
import { FormEvent, useState } from "react";
import type { Task, TaskInput } from "../../services/tasks";
import type { Agent } from "../../services/agents";
import "./TaskDialog.css";

type TaskDialogProps = {
  task?: Task;
  agents: Agent[];
  onClose: () => void;
  onSave: (input: TaskInput) => Promise<void>;
};

export function TaskDialog({ task, agents, onClose, onSave }: TaskDialogProps) {
  const [title, setTitle] = useState(task?.title ?? "");
  const [description, setDescription] = useState(task?.description ?? "");
  const [acceptanceCriteria, setAcceptanceCriteria] = useState(task ? task.acceptanceCriteria.join("\n") : "");
  const [implementationNotes, setImplementationNotes] = useState(task?.implementationNotes ?? "");
  const [relevantPaths, setRelevantPaths] = useState(task ? task.relevantPaths.join("\n") : "");
  const [dependencyIds, setDependencyIds] = useState(task ? task.dependencyIds.join("\n") : "");
  const [assignedAgentId, setAssignedAgentId] = useState(task?.assignedAgentId ?? "");
  const [error, setError] = useState<string>();
  const [isSaving, setIsSaving] = useState(false);

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError(undefined);
    setIsSaving(true);
    try {
      await onSave({
        title,
        description,
        acceptanceCriteria: lines(acceptanceCriteria),
        implementationNotes,
        relevantPaths: lines(relevantPaths),
        dependencyIds: lines(dependencyIds),
        assignedAgentId,
      });
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
            <textarea value={description} onChange={(event) => setDescription(event.target.value)} rows={3} placeholder="Problem, context, or intended outcome" />
          </label>
          <fieldset className="task-specification-fields">
            <legend>Task specification</legend>
            <label>
              Acceptance criteria <span className="field-optional">one item per line</span>
              <textarea value={acceptanceCriteria} onChange={(event) => setAcceptanceCriteria(event.target.value)} rows={4} placeholder={"Successful callback creates a session\nInvalid callback shows an error\nTests pass"} />
            </label>
            <label>
              Implementation notes <span className="field-optional">optional</span>
              <textarea value={implementationNotes} onChange={(event) => setImplementationNotes(event.target.value)} rows={3} placeholder="Technical approach, constraints, or review notes" />
            </label>
            <label>
              Relevant paths / context <span className="field-optional">one per line</span>
              <textarea value={relevantPaths} onChange={(event) => setRelevantPaths(event.target.value)} rows={3} placeholder={"src/auth\ndocs/architecture.md"} />
            </label>
            <label>
              Dependencies <span className="field-optional">task IDs, one per line</span>
              <textarea value={dependencyIds} onChange={(event) => setDependencyIds(event.target.value)} rows={2} placeholder="TASK-12 or task UUID" />
              <span className="field-hint">Dependencies are recorded now; execution blocking arrives in a later milestone.</span>
            </label>
          </fieldset>
          <label>
            Assigned agent <span className="field-optional">optional</span>
            <select value={assignedAgentId} onChange={(event) => setAssignedAgentId(event.target.value)}>
              <option value="">No agent assigned</option>
              {agents.map((agent) => <option key={agent.id} value={agent.id}>{agent.name} / {agent.role}</option>)}
            </select>
            {agents.length === 0 && <span className="field-hint">Create an agent from the Agents page before assigning work.</span>}
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

function lines(value: string) {
  return value.split("\n").map((line) => line.trim()).filter(Boolean);
}
