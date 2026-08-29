import { ArrowLeft, CirclePause, CircleStop, Play, RefreshCw, Save, StepForward } from "lucide-react";
import { useCallback, useEffect, useState, type FormEvent } from "react";
import { Link, useParams } from "react-router-dom";
import { listAgents, type Agent } from "../../services/agents";
import {
  advanceProjectAutonomy,
  getProjectAutonomy,
  listenToAutonomyEvents,
  pauseProjectAutonomy,
  startProjectAutonomy,
  stopProjectAutonomy,
  updateProjectAutonomy,
  type AutonomyConfiguration,
  type ProjectAutonomySnapshot,
} from "../../services/autonomy";
import { listPlanningProposals, type PlanningProposal } from "../../services/planning";
import { getProject, type Project } from "../../services/projects";
import "./ProjectAutonomyPage.css";

const defaultConfiguration: AutonomyConfiguration = {
  planningProposalId: null,
  reviewerAgentId: null,
  autoSchedule: true,
  autoReview: true,
  autoIntegrate: true,
  maxTasksPerCycle: 2,
  maxAutoRetries: 1,
  pauseOnFailure: true,
  pauseOnNeedsInput: true,
};

export function ProjectAutonomyPage() {
  const { projectId } = useParams();
  const [project, setProject] = useState<Project | null>();
  const [snapshot, setSnapshot] = useState<ProjectAutonomySnapshot>();
  const [proposals, setProposals] = useState<PlanningProposal[]>([]);
  const [agents, setAgents] = useState<Agent[]>([]);
  const [configuration, setConfiguration] = useState(defaultConfiguration);
  const [error, setError] = useState<string>();
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);

  const load = useCallback(async () => {
    if (!projectId) return;
    setError(undefined);
    try {
      const [loadedProject, loadedSnapshot, loadedProposals, loadedAgents] = await Promise.all([
        getProject(projectId), getProjectAutonomy(projectId), listPlanningProposals(projectId), listAgents(),
      ]);
      setProject(loadedProject);
      setSnapshot(loadedSnapshot);
      setProposals(loadedProposals.filter((proposal) => proposal.status === "approved"));
      setAgents(loadedAgents.filter((agent) => agent.provider === "codex"));
      setConfiguration(configurationFrom(loadedSnapshot));
    } catch (loadError) {
      setError(messageFrom(loadError, "Unable to load autonomous project mode."));
    } finally {
      setIsLoading(false);
    }
  }, [projectId]);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    if (!projectId) return;
    let unlisten: (() => void) | undefined;
    void listenToAutonomyEvents((changedProjectId) => {
      if (changedProjectId === projectId) void load();
    }).then((stop) => { unlisten = stop; });
    return () => { unlisten?.(); };
  }, [load, projectId]);

  const runAction = async (action: () => Promise<ProjectAutonomySnapshot>) => {
    setIsSaving(true); setError(undefined);
    try {
      const updated = await action();
      setSnapshot(updated);
      setConfiguration(configurationFrom(updated));
    } catch (actionError) {
      setError(messageFrom(actionError, "Unable to change autonomous project mode."));
    } finally { setIsSaving(false); }
  };

  const save = async (event: FormEvent) => {
    event.preventDefault();
    if (!projectId) return;
    await runAction(() => updateProjectAutonomy(projectId, configuration));
  };

  if (isLoading && !snapshot) return <LoadingState />;
  if (!project || !snapshot || !projectId) return <NotFoundState />;
  const autonomy = snapshot.autonomy;
  const isRunning = autonomy.status === "running";

  return <section className="page autonomy-page">
    <header className="page-header autonomy-header">
      <div>
        <Link className="back-link" to={`/projects/${project.id}`}><ArrowLeft size={15} /> Board</Link>
        <p className="eyebrow">Supervised control loop / {project.defaultBranch}</p>
        <h1>{project.name} autonomy</h1>
        <p className="muted">Advance only the human-approved plan through existing readiness, review, integration, and health gates.</p>
      </div>
      <div className="autonomy-actions">
        <span className={`autonomy-status ${autonomy.status}`}>{autonomy.status}</span>
        <button className="icon-button" type="button" onClick={() => void load()} aria-label="Refresh autonomy"><RefreshCw size={16} /></button>
        {isRunning ? <>
          <button className="secondary-button" type="button" disabled={isSaving} onClick={() => void runAction(() => advanceProjectAutonomy(projectId))}><StepForward size={15} /> Step</button>
          <button className="secondary-button" type="button" disabled={isSaving} onClick={() => void runAction(() => pauseProjectAutonomy(projectId))}><CirclePause size={15} /> Pause</button>
          <button className="danger-button" type="button" disabled={isSaving} onClick={() => void runAction(() => stopProjectAutonomy(projectId))}><CircleStop size={15} /> Stop</button>
        </> : <button className="primary-button" type="button" disabled={isSaving} onClick={() => void runAction(() => startProjectAutonomy(projectId))}><Play size={15} /> {autonomy.status === "paused" ? "Resume" : "Start"}</button>}
      </div>
    </header>

    {error && <div className="inline-error" role="alert">{error}</div>}
    {autonomy.pauseReason && <div className="autonomy-notice"><strong>Operator attention</strong><span>{autonomy.pauseReason}</span></div>}

    <section className="autonomy-summary" aria-label="Approved plan progress">
      <div><span>Approved goal</span><strong>{snapshot.goal ?? "No plan selected"}</strong></div>
      <Metric label="Done" value={snapshot.counts.done} tone="done" />
      <Metric label="Ready" value={snapshot.counts.ready} tone="ready" />
      <Metric label="Active" value={snapshot.counts.inProgress} tone="active" />
      <Metric label="Review" value={snapshot.counts.review} tone="review" />
      <Metric label="Blocked / input" value={snapshot.counts.blocked + snapshot.counts.needsInput} tone="blocked" />
    </section>

    <div className="autonomy-layout">
      <main className="autonomy-main">
        <section className="autonomy-section">
          <div className="autonomy-section-heading"><div><p className="eyebrow">Durable activity</p><h2>Control cycles</h2></div><span>{snapshot.cycles.length} recent</span></div>
          {snapshot.cycles.length === 0 ? <Empty text="No autonomous cycle has run." /> : <div className="cycle-list">{snapshot.cycles.map((cycle) => <article key={cycle.id}>
            <div><span className={`cycle-state ${cycle.status}`}>{cycle.status}</span><strong>{cycle.outcome ?? "Cycle in progress"}</strong><time>{formatTimestamp(cycle.startedAt)} · {cycle.triggerKind}</time></div>
            <code>{cycle.scheduledCount} scheduled / {cycle.reviewCount} reviewed / {cycle.retryCount} retried / {cycle.integrationCount} integrated</code>
          </article>)}</div>}
        </section>
        <section className="autonomy-section">
          <div className="autonomy-section-heading"><div><p className="eyebrow">Action provenance</p><h2>Audit trail</h2></div></div>
          {snapshot.events.length === 0 ? <Empty text="Task-level autonomous actions will be recorded here." /> : <div className="autonomy-event-list">{snapshot.events.map((event) => <article key={event.id}><code>{event.kind}</code><p>{event.message}</p><span>{formatTimestamp(event.createdAt)}{event.taskId ? ` · task ${event.taskId.slice(0, 8)}` : ""}{event.runId ? ` · run ${event.runId.slice(0, 8)}` : ""}</span></article>)}</div>}
        </section>
      </main>

      <form className="autonomy-config" onSubmit={(event) => void save(event)}>
        <div><p className="eyebrow">Operator policy</p><h2>Safety envelope</h2><p>Configuration is locked while the control loop is running.</p></div>
        <label>Approved plan<select value={configuration.planningProposalId ?? ""} disabled={isRunning || isSaving} required onChange={(event) => setConfiguration((current) => ({ ...current, planningProposalId: event.target.value || null }))}><option value="">Select approved plan...</option>{proposals.map((proposal) => <option key={proposal.id} value={proposal.id}>{proposal.goal}</option>)}</select></label>
        <label>Read-only reviewer<select value={configuration.reviewerAgentId ?? ""} disabled={isRunning || isSaving || !configuration.autoReview} required={configuration.autoReview} onChange={(event) => setConfiguration((current) => ({ ...current, reviewerAgentId: event.target.value || null }))}><option value="">Select reviewer...</option>{agents.map((agent) => <option key={agent.id} value={agent.id}>{agent.name} · {agent.model ?? "default"}</option>)}</select></label>
        <div className="autonomy-toggle-list">
          <Toggle label="Schedule Ready work" checked={configuration.autoSchedule} disabled={isRunning || isSaving} onChange={(autoSchedule) => setConfiguration((current) => ({ ...current, autoSchedule }))} />
          <Toggle label="Start architect review" checked={configuration.autoReview} disabled={isRunning || isSaving} onChange={(autoReview) => setConfiguration((current) => ({ ...current, autoReview }))} />
          <Toggle label="Integrate approved work" checked={configuration.autoIntegrate} disabled={isRunning || isSaving} onChange={(autoIntegrate) => setConfiguration((current) => ({ ...current, autoIntegrate }))} />
          <Toggle label="Pause on failure" checked={configuration.pauseOnFailure} disabled={isRunning || isSaving} onChange={(pauseOnFailure) => setConfiguration((current) => ({ ...current, pauseOnFailure }))} />
          <Toggle label="Pause when input is needed" checked={configuration.pauseOnNeedsInput} disabled={isRunning || isSaving} onChange={(pauseOnNeedsInput) => setConfiguration((current) => ({ ...current, pauseOnNeedsInput }))} />
        </div>
        <div className="autonomy-number-grid">
          <label>Tasks per cycle<input type="number" min="1" max="20" value={configuration.maxTasksPerCycle} disabled={isRunning || isSaving} onChange={(event) => setConfiguration((current) => ({ ...current, maxTasksPerCycle: Number(event.target.value) }))} /></label>
          <label>Automatic retries<input type="number" min="0" max="3" value={configuration.maxAutoRetries} disabled={isRunning || isSaving} onChange={(event) => setConfiguration((current) => ({ ...current, maxAutoRetries: Number(event.target.value) }))} /></label>
        </div>
        <button className="secondary-button" type="submit" disabled={isRunning || isSaving}><Save size={14} /> Save policy</button>
        <p className="autonomy-safety-note">Stop prevents new autonomous actions. Existing worker runs and branches remain recoverable. Human approval is still required for every planning proposal; merge health validation is never bypassed.</p>
      </form>
    </div>
  </section>;
}

function configurationFrom(snapshot: ProjectAutonomySnapshot): AutonomyConfiguration {
  const autonomy = snapshot.autonomy;
  return {
    planningProposalId: autonomy.planningProposalId, reviewerAgentId: autonomy.reviewerAgentId,
    autoSchedule: autonomy.autoSchedule, autoReview: autonomy.autoReview, autoIntegrate: autonomy.autoIntegrate,
    maxTasksPerCycle: autonomy.maxTasksPerCycle, maxAutoRetries: autonomy.maxAutoRetries,
    pauseOnFailure: autonomy.pauseOnFailure, pauseOnNeedsInput: autonomy.pauseOnNeedsInput,
  };
}

function Toggle({ label, checked, disabled, onChange }: { label: string; checked: boolean; disabled: boolean; onChange: (checked: boolean) => void }) {
  return <label><input type="checkbox" checked={checked} disabled={disabled} onChange={(event) => onChange(event.target.checked)} /><span>{label}</span></label>;
}

function Metric({ label, value, tone }: { label: string; value: number; tone: string }) { return <div className={`autonomy-metric ${tone}`}><span>{label}</span><strong>{value}</strong></div>; }
function Empty({ text }: { text: string }) { return <p className="autonomy-empty">{text}</p>; }
function formatTimestamp(value: string) { return value.replace("T", " ").replace("Z", "").slice(0, 19); }
function messageFrom(error: unknown, fallback: string) { return error instanceof Error ? error.message : typeof error === "string" ? error : fallback; }
function LoadingState() { return <section className="page"><div className="empty-state"><span className="empty-index">LOOP</span><h2>Loading autonomy controls</h2></div></section>; }
function NotFoundState() { return <section className="page"><div className="empty-state"><h2>Project not found</h2><Link className="secondary-button" to="/projects">Return to projects</Link></div></section>; }
