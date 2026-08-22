import { X } from "lucide-react";
import { FormEvent, useState } from "react";
import type { Agent, AgentInput } from "../../services/agents";
import "./AgentDialog.css";

const codexModels = [
  { id: "gpt-5.6-sol", name: "GPT-5.6 Sol", description: "Highest capability" },
  { id: "gpt-5.6-terra", name: "GPT-5.6 Terra", description: "Balanced capability and cost" },
  { id: "gpt-5.6-luna", name: "GPT-5.6 Luna", description: "Fast, cost-efficient tasks" },
] as const;

type AgentDialogProps = {
  agent?: Agent;
  onClose: () => void;
  onSave: (input: AgentInput) => Promise<void>;
};

export function AgentDialog({ agent, onClose, onSave }: AgentDialogProps) {
  const [name, setName] = useState(agent?.name ?? "");
  const [role, setRole] = useState(agent?.role ?? "");
  const [model, setModel] = useState(agent?.model ?? "");
  const [systemPrompt, setSystemPrompt] = useState(agent?.systemPrompt ?? "");
  const [skills, setSkills] = useState(agent ? agent.skills.join("\n") : "");
  const [maxConcurrentTasks, setMaxConcurrentTasks] = useState(String(agent?.maxConcurrentTasks ?? 1));
  const [error, setError] = useState<string>();
  const [isSaving, setIsSaving] = useState(false);
  const isSavedModelUnavailable = model !== "" && !codexModels.some((option) => option.id === model);

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError(undefined);
    setIsSaving(true);
    try {
      await onSave({ name, provider: "codex", role, model, systemPrompt, skills: lines(skills), maxConcurrentTasks: Number(maxConcurrentTasks) });
      onClose();
    } catch (saveError) {
      setError(saveError instanceof Error ? saveError.message : "Unable to save agent.");
    } finally {
      setIsSaving(false);
    }
  };

  return <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
    <section className="agent-dialog" role="dialog" aria-modal="true" aria-labelledby="agent-dialog-title" onMouseDown={(event) => event.stopPropagation()}>
      <header className="dialog-header"><div><p className="eyebrow">Agent configuration</p><h2 id="agent-dialog-title">{agent ? "Edit agent" : "New agent"}</h2></div><button className="icon-button" type="button" onClick={onClose} aria-label="Close dialog"><X size={16} /></button></header>
      <form onSubmit={submit}>
        <label>Name<input value={name} onChange={(event) => setName(event.target.value)} required autoFocus maxLength={120} placeholder="Codex Terra" /></label>
        <label>Role<input value={role} onChange={(event) => setRole(event.target.value)} required maxLength={120} placeholder="Frontend engineer" /></label>
        <label>Provider<select value="codex" disabled><option value="codex">Codex</option></select><span className="field-hint">Codex is the currently available local provider.</span></label>
        <label>Model <span className="field-optional">optional</span><select value={model} onChange={(event) => setModel(event.target.value)}><option value="">Provider default</option>{isSavedModelUnavailable && <option value={model}>Current unavailable model: {model}</option>}{codexModels.map((option) => <option key={option.id} value={option.id}>{option.name} — {option.description}</option>)}</select></label>
        <label>System instructions <span className="field-optional">optional</span><textarea value={systemPrompt} onChange={(event) => setSystemPrompt(event.target.value)} rows={4} placeholder="Review the project conventions before changing code." /></label>
        <label>Skills <span className="field-optional">one per line</span><textarea value={skills} onChange={(event) => setSkills(event.target.value)} rows={3} placeholder={"react\ntypescript\ntesting"} /></label>
        <label>Maximum concurrent tasks<input type="number" min="1" max="32" value={maxConcurrentTasks} onChange={(event) => setMaxConcurrentTasks(event.target.value)} required /></label>
        {error && <p className="form-error" role="alert">{error}</p>}
        <footer className="dialog-actions"><button type="button" className="secondary-button" onClick={onClose}>Cancel</button><button type="submit" className="primary-button" disabled={isSaving}>{isSaving ? "Saving..." : agent ? "Save agent" : "Create agent"}</button></footer>
      </form>
    </section>
  </div>;
}

function lines(value: string) { return value.split("\n").map((line) => line.trim()).filter(Boolean); }
