import { Activity, Play, Plus, RotateCcw, Trash2, X } from "lucide-react";
import { FormEvent, useState } from "react";
import type { ProjectHealth, ValidationAttempt, ValidationCommand, ValidationStage } from "../../services/quality";
import "./QualityGatesPanel.css";

type QualityGatesPanelProps = {
  health?: ProjectHealth;
  implementationCommands: ValidationCommand[];
  integrationCommands: ValidationCommand[];
  attempts: ValidationAttempt[];
  isLoading: boolean;
  isRunning: boolean;
  onClose: () => void;
  onRefresh: () => void;
  onAddCommand: (input: { stage: ValidationStage; name: string; program: string; arguments: string[] }) => Promise<void>;
  onDeleteCommand: (id: string) => void;
  onRerunIntegration: () => void;
};

export function QualityGatesPanel({ health, implementationCommands, integrationCommands, attempts, isLoading, isRunning, onClose, onRefresh, onAddCommand, onDeleteCommand, onRerunIntegration }: QualityGatesPanelProps) {
  const [stage, setStage] = useState<ValidationStage>("implementation");
  const [name, setName] = useState("");
  const [program, setProgram] = useState("");
  const [argumentsText, setArgumentsText] = useState("");
  const [isSaving, setIsSaving] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setIsSaving(true);
    try {
      await onAddCommand({ stage, name, program, arguments: argumentsText.split("\n").map((argument) => argument.trim()).filter(Boolean) });
      setName(""); setProgram(""); setArgumentsText("");
    } finally { setIsSaving(false); }
  };

  return <aside className="quality-gates-panel" aria-label="Quality gates and project health">
    <header className="quality-gates-header"><div><p className="eyebrow">Delivery safety</p><h2>Quality gates</h2></div><button className="icon-button" type="button" onClick={onClose} aria-label="Close quality gates"><X size={16} /></button></header>
    <div className="quality-gates-content">
      <section className={`project-health ${health?.status ?? "unknown"}`}>
        <div><Activity size={17} /><span>Integration branch health</span></div><strong>{health?.status ?? "unknown"}</strong>
        <p>{health?.failingGate ? `Failing gate: ${health.failingGate}` : health?.lastSuccessfulValidationAt ? `Last green: ${formatTime(health.lastSuccessfulValidationAt)}` : "No completed integration validation yet."}</p>
        <button className="secondary-button" type="button" disabled={isRunning} onClick={onRerunIntegration}><RotateCcw size={14} /> {isRunning ? "Running checks..." : "Re-run integration checks"}</button>
      </section>
      <p className="quality-gates-hint">Commands run as executable plus argument arrays in the selected workspace. Add one argument per line; no shell is involved.</p>
      <GateList title="Implementation checks" commands={implementationCommands} onDelete={onDeleteCommand} />
      <GateList title="Integration checks" commands={integrationCommands} onDelete={onDeleteCommand} />
      <form className="quality-command-form" onSubmit={(event) => void submit(event)}>
        <h3>Add a check</h3>
        <label>Stage<select value={stage} onChange={(event) => setStage(event.target.value as ValidationStage)}><option value="implementation">Implementation</option><option value="integration">Integration</option></select></label>
        <label>Name<input value={name} onChange={(event) => setName(event.target.value)} placeholder="Typecheck" required /></label>
        <label>Program<input value={program} onChange={(event) => setProgram(event.target.value)} placeholder="npm" required /></label>
        <label>Arguments <span>one per line</span><textarea value={argumentsText} onChange={(event) => setArgumentsText(event.target.value)} placeholder={"run\ntypecheck"} rows={3} /></label>
        <button className="primary-button" type="submit" disabled={isSaving}><Plus size={14} /> {isSaving ? "Saving..." : "Add check"}</button>
      </form>
      <section className="validation-history"><div className="quality-section-heading"><h3>Recent results</h3><button className="icon-button" type="button" onClick={onRefresh} disabled={isLoading} aria-label="Refresh validation results"><RotateCcw size={14} /></button></div>
        {attempts.length === 0 ? <p className="quality-empty">No validation has run yet.</p> : <ol>{attempts.slice(0, 8).map((attempt) => <li key={attempt.id}><div><span className={`validation-status ${attempt.status}`}>{attempt.status}</span><strong>{attempt.stage}</strong><time>{formatTime(attempt.startedAt)}</time></div>{attempt.error && <p>{attempt.error}</p>}{attempt.events.length > 0 && <pre>{attempt.events.slice(-12).map((event) => event.message).join("")}</pre>}</li>)}</ol>}
      </section>
    </div>
  </aside>;
}

function GateList({ title, commands, onDelete }: { title: string; commands: ValidationCommand[]; onDelete: (id: string) => void }) {
  return <section className="quality-gate-list"><h3>{title}</h3>{commands.length === 0 ? <p className="quality-empty">No required checks configured.</p> : <ul>{commands.map((command) => <li key={command.id}><div><strong>{command.name}</strong><code>{[command.program, ...command.arguments].join(" ")}</code></div><button className="icon-button" type="button" onClick={() => onDelete(command.id)} aria-label={`Delete ${command.name}`}><Trash2 size={14} /></button></li>)}</ul>}</section>;
}

function formatTime(value: string) { return new Date(`${value.replace(" ", "T")}Z`).toLocaleString(); }
