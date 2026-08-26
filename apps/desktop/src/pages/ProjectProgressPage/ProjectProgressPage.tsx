import { ArrowLeft, Flag, Layers3, Plus, RefreshCw } from "lucide-react";
import { FormEvent, useCallback, useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { createEpic, createMilestone, getProjectProgress, listEpics, listMilestones, updateEpicStatus, updateMilestoneStatus, type Epic, type Milestone, type OutcomeStatus, type ProjectProgress } from "../../services/outcomes";
import { getProject, type Project } from "../../services/projects";
import { listIntegrationAttempts, type IntegrationAttempt } from "../../services/integrations";
import { getProjectHealth, type ProjectHealth } from "../../services/quality";
import "./ProjectProgressPage.css";

const statuses: OutcomeStatus[] = ["planned", "active", "completed", "blocked"];

export function ProjectProgressPage() {
  const { projectId } = useParams();
  const [project, setProject] = useState<Project | null>();
  const [progress, setProgress] = useState<ProjectProgress>();
  const [milestones, setMilestones] = useState<Milestone[]>([]);
  const [epics, setEpics] = useState<Epic[]>([]);
  const [health, setHealth] = useState<ProjectHealth>();
  const [integrations, setIntegrations] = useState<IntegrationAttempt[]>([]);
  const [error, setError] = useState<string>();
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);

  const load = useCallback(async () => {
    if (!projectId) return;
    setIsLoading(true); setError(undefined);
    try {
      const [loadedProject, loadedProgress, loadedMilestones, loadedEpics, loadedHealth, loadedIntegrations] = await Promise.all([getProject(projectId), getProjectProgress(projectId), listMilestones(projectId), listEpics(projectId), getProjectHealth(projectId), listIntegrationAttempts(projectId)]);
      setProject(loadedProject); setProgress(loadedProgress); setMilestones(loadedMilestones); setEpics(loadedEpics); setHealth(loadedHealth); setIntegrations(loadedIntegrations);
    } catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Unable to load project progress."); }
    finally { setIsLoading(false); }
  }, [projectId]);

  useEffect(() => { void load(); }, [load]);

  const addMilestone = async (input: OutcomeInput) => {
    if (!projectId) return;
    setIsSaving(true);
    try { await createMilestone({ projectId, ...input, targetDate: input.targetDate || undefined }); await load(); }
    catch (saveError) { setError(saveError instanceof Error ? saveError.message : "Unable to create milestone."); }
    finally { setIsSaving(false); }
  };
  const addEpic = async (input: Omit<OutcomeInput, "targetDate"> & { milestoneId: string }) => {
    if (!projectId) return;
    setIsSaving(true);
    try { await createEpic({ projectId, ...input, milestoneId: input.milestoneId || undefined }); await load(); }
    catch (saveError) { setError(saveError instanceof Error ? saveError.message : "Unable to create epic."); }
    finally { setIsSaving(false); }
  };
  const changeMilestoneStatus = async (id: string, status: OutcomeStatus) => { setIsSaving(true); try { await updateMilestoneStatus(id, status); await load(); } catch (saveError) { setError(saveError instanceof Error ? saveError.message : "Unable to update milestone status."); } finally { setIsSaving(false); } };
  const changeEpicStatus = async (id: string, status: OutcomeStatus) => { setIsSaving(true); try { await updateEpicStatus(id, status); await load(); } catch (saveError) { setError(saveError instanceof Error ? saveError.message : "Unable to update epic status."); } finally { setIsSaving(false); } };

  if (isLoading) return <section className="page"><div className="empty-state"><span className="empty-index">SYNC</span><h2>Loading project progress</h2></div></section>;
  if (!project) return <section className="page"><div className="empty-state"><h2>Project not found</h2><Link className="secondary-button" to="/projects">Return to projects</Link></div></section>;
  const counts = progress?.counts ?? { total: 0, done: 0, ready: 0, inProgress: 0, needsInput: 0, review: 0, blocked: 0, backlog: 0 };
  const queuedIntegrations = integrations.filter((attempt) => attempt.status === "queued" || attempt.status === "integrating").length;
  return <section className="page project-progress-page">
    <header className="page-header"><div><Link className="back-link" to={`/projects/${project.id}`}><ArrowLeft size={15} /> Board</Link><p className="eyebrow">Outcome progress / {project.defaultBranch}</p><h1>{project.name}</h1><p className="muted">Integrated outcomes, not agent activity.</p></div><button className="icon-button" type="button" onClick={() => void load()} aria-label="Refresh progress"><RefreshCw size={16} /></button></header>
    {error && <div className="inline-error" role="alert">{error}</div>}
    <section className="progress-system-state" aria-label="Integration branch state"><div><span>Integration branch</span><strong>{project.defaultBranch}</strong></div><div><span>Main health</span><strong className={`health-value ${health?.status ?? "unknown"}`}>{health?.status ?? "unknown"}</strong></div><div><span>Integration queue</span><strong>{queuedIntegrations}</strong></div></section>
    <section className="progress-overview"><Metric label="Done" value={counts.done} tone="done" /><Metric label="Ready" value={counts.ready} tone="ready" /><Metric label="In progress" value={counts.inProgress} tone="progress" /><Metric label="Needs input" value={counts.needsInput} tone="input" /><Metric label="Review" value={counts.review} tone="review" /><Metric label="Blocked" value={counts.blocked} tone="blocked" /><Metric label="Total" value={counts.total} tone="total" /></section>
    <div className="outcome-layout"><div className="outcome-main"><section className="outcome-section"><div className="outcome-section-heading"><div><p className="eyebrow">Project hierarchy</p><h2>Milestones</h2></div><Flag size={18} /></div>{progress?.milestones.length === 0 ? <p className="outcome-empty">Create a milestone to group project outcomes.</p> : <div className="milestone-progress-list">{progress?.milestones.map(({ milestone, counts: milestoneCounts, epics: milestoneEpics }) => <article key={milestone.id} className="milestone-progress-card"><div className="milestone-progress-heading"><div><select className={`outcome-status ${milestone.status}`} value={milestone.status} disabled={isSaving} aria-label={`${milestone.title} status`} onChange={(event) => void changeMilestoneStatus(milestone.id, event.target.value as OutcomeStatus)}>{statuses.map((status) => <option key={status} value={status}>{status}</option>)}</select><h3>{milestone.title}</h3></div><strong>{milestoneCounts.done} / {milestoneCounts.total} Done</strong></div>{milestone.description && <p>{milestone.description}</p>}<div className="milestone-bar"><i style={{ width: `${milestoneCounts.total === 0 ? 0 : (milestoneCounts.done / milestoneCounts.total) * 100}%` }} /></div><div className="milestone-counts"><span>{milestoneCounts.ready} Ready</span><span>{milestoneCounts.inProgress} In progress</span><span>{milestoneCounts.needsInput} Needs input</span><span>{milestoneCounts.review} Review</span><span>{milestoneCounts.blocked} Blocked</span></div>{milestoneEpics.length > 0 && <div className="milestone-epics"><Layers3 size={14} /> {milestoneEpics.map((epic) => <label key={epic.id}><span>{epic.title}</span><select value={epic.status} disabled={isSaving} aria-label={`${epic.title} status`} onChange={(event) => void changeEpicStatus(epic.id, event.target.value as OutcomeStatus)}>{statuses.map((status) => <option key={status} value={status}>{status}</option>)}</select></label>)}</div>}</article>)}</div>}</section></div><aside className="outcome-forms"><OutcomeForm title="New milestone" submitLabel="Add milestone" isSaving={isSaving} onSubmit={addMilestone} /><EpicForm milestones={milestones} isSaving={isSaving} onSubmit={addEpic} /></aside></div>
  </section>;
}

type OutcomeInput = { title: string; description?: string; status: OutcomeStatus; targetDate?: string };
function OutcomeForm({ title, submitLabel, isSaving, onSubmit }: { title: string; submitLabel: string; isSaving: boolean; onSubmit: (input: OutcomeInput) => Promise<void> }) { const [name, setName] = useState(""); const [description, setDescription] = useState(""); const [status, setStatus] = useState<OutcomeStatus>("planned"); const [targetDate, setTargetDate] = useState(""); const submit = async (event: FormEvent) => { event.preventDefault(); await onSubmit({ title: name, description, status, targetDate }); setName(""); setDescription(""); setTargetDate(""); }; return <form className="outcome-form" onSubmit={(event) => void submit(event)}><h2>{title}</h2><label>Title<input value={name} onChange={(event) => setName(event.target.value)} required /></label><label>Description<textarea value={description} onChange={(event) => setDescription(event.target.value)} rows={2} /></label><label>Status<select value={status} onChange={(event) => setStatus(event.target.value as OutcomeStatus)}>{statuses.map((value) => <option key={value}>{value}</option>)}</select></label><label>Target date <span>optional</span><input type="date" value={targetDate} onChange={(event) => setTargetDate(event.target.value)} /></label><button className="primary-button" type="submit" disabled={isSaving}><Plus size={14} /> {submitLabel}</button></form>; }
function EpicForm({ milestones, isSaving, onSubmit }: { milestones: Milestone[]; isSaving: boolean; onSubmit: (input: Omit<OutcomeInput, "targetDate"> & { milestoneId: string }) => Promise<void> }) { const [name, setName] = useState(""); const [description, setDescription] = useState(""); const [status, setStatus] = useState<OutcomeStatus>("planned"); const [milestoneId, setMilestoneId] = useState(""); const submit = async (event: FormEvent) => { event.preventDefault(); await onSubmit({ title: name, description, status, milestoneId }); setName(""); setDescription(""); }; return <form className="outcome-form" onSubmit={(event) => void submit(event)}><h2>New epic</h2><label>Title<input value={name} onChange={(event) => setName(event.target.value)} required /></label><label>Milestone<select value={milestoneId} onChange={(event) => setMilestoneId(event.target.value)}><option value="">No milestone</option>{milestones.map((milestone) => <option key={milestone.id} value={milestone.id}>{milestone.title}</option>)}</select></label><label>Status<select value={status} onChange={(event) => setStatus(event.target.value as OutcomeStatus)}>{statuses.map((value) => <option key={value}>{value}</option>)}</select></label><label>Description<textarea value={description} onChange={(event) => setDescription(event.target.value)} rows={2} /></label><button className="secondary-button" type="submit" disabled={isSaving}><Plus size={14} /> Add epic</button></form>; }
function Metric({ label, value, tone }: { label: string; value: number; tone: string }) { return <div className={`progress-metric ${tone}`}><span>{label}</span><strong>{value}</strong></div>; }
