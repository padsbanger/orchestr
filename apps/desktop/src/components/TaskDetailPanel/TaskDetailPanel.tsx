import { BookOpenCheck, Bot, CheckCircle2, CheckSquare, CircleAlert, Code2, Download, FileCode2, FolderOpen, GitBranch, LoaderCircle, MessageSquareText, Pencil, Play, RotateCcw, Square, Terminal, X } from "lucide-react";
import { save } from "@tauri-apps/plugin-dialog";
import { useEffect, useId, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type ReactNode } from "react";
import type { Agent } from "../../services/agents";
import { exportTaskRunLog, type RunEvent, type TaskRun } from "../../services/runs";
import type { Task } from "../../services/tasks";
import type { TaskReview } from "../../services/reviews";
import type { AgentReview } from "../../services/agentReviews";
import type { TaskInputRequest } from "../../services/interruptions";
import type { ArchitectureDecision } from "../../services/knowledge";
import type { IntegrationAttempt, RevertAttempt } from "../../services/integrations";
import type { ValidationAttempt } from "../../services/quality";
import type { WorkflowTaskView } from "../../services/workflow";
import "./TaskDetailPanel.css";

export type TaskDetailPanelProps = {
  task: Task;
  workflowView: WorkflowTaskView;
  assignedAgent?: Agent;
  recoveryAgents: Agent[];
  reviewerAgents: Agent[];
  agentReviews: AgentReview[];
  inputRequests: TaskInputRequest[];
  architectureDecisions: ArchitectureDecision[];
  isAgentReviewStarting: boolean;
  runs: TaskRun[];
  isStartingRun: boolean;
  runRecoveryAction?: string;
  inputAction?: "request" | "answer";
  cancellingRunId?: string;
  isCleaningWorktree: boolean;
  isOpeningWorktree: boolean;
  review?: TaskReview;
  reviewError?: string;
  isReviewLoading: boolean;
  isReviewActionPending: boolean;
  integrationAttempts?: IntegrationAttempt[];
  revertAttempts?: RevertAttempt[];
  validationAttempts?: ValidationAttempt[];
  onTabChange?: (tab: DetailTab) => void;
  onClose: () => void;
  onEdit: (task: Task) => void;
  onStartRun: () => void;
  onCancelRun: (runId: string) => void;
  onRecoverRun: (runId: string, mode: "resume" | "restart_clean", agentId?: string) => void;
  onResolveRunFailure: (runId: string, action: "abandon" | "escalate") => void;
  onRequestInput: (question: string, runId?: string) => void;
  onAnswerInput: (requestId: string, answer: string) => void;
  onCleanupWorktree: () => void;
  onOpenWorktree: () => void;
  onApproveReview: () => void;
  onRequestChanges: () => void;
  onStartAgentReview: (agentId: string) => void;
};

const DETAIL_TABS = [
  { id: "work", label: "Work" },
  { id: "activity", label: "Activity" },
  { id: "review", label: "Review & Land" },
] as const;

export type DetailTab = (typeof DETAIL_TABS)[number]["id"];

export function TaskDetailPanel(props: TaskDetailPanelProps) {
  const { task, workflowView, agentReviews, runs, onTabChange, onClose, onEdit } = props;
  const hasLiveActivity = runs.some((run) => run.status === "queued" || run.status === "running")
    || agentReviews.some((review) => review.status === "running");
  const [now, setNow] = useState(() => Date.now());
  const [activeTab, setActiveTab] = useState<DetailTab>(() => defaultDetailTab(workflowView.stage));
  const tabGroupId = useId();
  const tabRefs = useRef<Record<DetailTab, HTMLButtonElement | null>>({ work: null, activity: null, review: null });

  useEffect(() => {
    const interval = window.setInterval(() => setNow(Date.now()), hasLiveActivity ? 1_000 : 60_000);
    return () => window.clearInterval(interval);
  }, [hasLiveActivity]);

  useEffect(() => {
    setActiveTab(defaultDetailTab(workflowView.stage));
  }, [task.id, workflowView.stage]);

  useEffect(() => { onTabChange?.(activeTab); }, [activeTab, onTabChange]);

  const phase = workflowPhaseSummary(workflowView);
  const selectTabFromKeyboard = (event: ReactKeyboardEvent<HTMLButtonElement>, currentTab: DetailTab) => {
    const nextTab = detailTabForKey(currentTab, event.key);
    if (!nextTab) return;
    event.preventDefault();
    setActiveTab(nextTab);
    tabRefs.current[nextTab]?.focus();
  };

  return (
    <aside className="board-inspector-panel task-detail-panel" aria-label={`Task details for ${task.title}`}>
      <header className="task-detail-header">
        <div><p className="eyebrow">Task specification</p><h2>{task.title}</h2><code>{task.id}</code></div>
        <div className="task-detail-actions">
          <button className="icon-button" type="button" onClick={() => onEdit(task)} aria-label={`Edit ${task.title}`}><Pencil size={16} /></button>
          <button className="icon-button" type="button" onClick={onClose} aria-label="Close task details"><X size={16} /></button>
        </div>
      </header>
      <div className="task-detail-body">
        <section className={`task-phase-summary ${task.status}`} aria-label="Current task phase" aria-live="polite">
          <div className="task-phase-summary-row">
            <span className={`task-phase-status ${task.status}`}>{statusLabel(task.status)}</span>
            <span className="task-phase-stage">{phase.stage}</span>
            <time dateTime={workflowView.statusChangedAt} title={`Entered this state: ${dateTime(workflowView.statusChangedAt)}`}>In state {relativeAge(workflowView.statusChangedAt, now)}</time>
          </div>
          <div className="task-phase-guidance">
            <span className={`task-phase-actor ${phase.actorKind}`}><span aria-hidden="true" />{phase.actor}</span>
            <p><strong>Next</strong>{phase.nextAction}</p>
          </div>
        </section>
        <div className="task-detail-tabs" role="tablist" aria-label="Task detail sections">
          {DETAIL_TABS.map((tab) => <button
            key={tab.id}
            ref={(node) => { tabRefs.current[tab.id] = node; }}
            id={`${tabGroupId}-${tab.id}-tab`}
            role="tab"
            type="button"
            aria-selected={activeTab === tab.id}
            aria-controls={`${tabGroupId}-${tab.id}-panel`}
            tabIndex={activeTab === tab.id ? 0 : -1}
            onClick={() => setActiveTab(tab.id)}
            onKeyDown={(event) => selectTabFromKeyboard(event, tab.id)}
          >{tab.label}</button>)}
        </div>
        <div
          className="task-detail-content"
          id={`${tabGroupId}-work-panel`}
          role="tabpanel"
          aria-labelledby={`${tabGroupId}-work-tab`}
          hidden={activeTab !== "work"}
          tabIndex={0}
        >
          <WorkTab {...props} />
        </div>
        <div
          className="task-detail-content"
          id={`${tabGroupId}-activity-panel`}
          role="tabpanel"
          aria-labelledby={`${tabGroupId}-activity-tab`}
          hidden={activeTab !== "activity"}
          tabIndex={0}
        >
          <ActivityTab {...props} now={now} />
        </div>
        <div
          className="task-detail-content"
          id={`${tabGroupId}-review-panel`}
          role="tabpanel"
          aria-labelledby={`${tabGroupId}-review-tab`}
          hidden={activeTab !== "review"}
          tabIndex={0}
        >
          <ReviewLandTab {...props} now={now} />
        </div>
      </div>
    </aside>
  );
}

function WorkTab({ task, workflowView, assignedAgent, architectureDecisions, runs, isCleaningWorktree, isOpeningWorktree, onCleanupWorktree, onOpenWorktree }: TaskDetailPanelProps) {
  const activeRun = runs.find((run) => run.status === "queued" || run.status === "running");
  return <>
    <TaskSection title="Context"><p className="task-detail-copy">{task.description || "No additional context has been recorded."}</p></TaskSection>
    <ReadinessSection task={task} workflowView={workflowView} />
    <TaskSection title="Acceptance criteria" icon={<CheckSquare size={14} />} count={task.acceptanceCriteria.length}>
      {task.acceptanceCriteria.length === 0 ? <p className="task-detail-empty">No acceptance criteria recorded.</p> : <ul className="criteria-list">{task.acceptanceCriteria.map((criterion) => <li key={criterion}>{criterion}</li>)}</ul>}
    </TaskSection>
    <TaskSection title="Implementation notes" icon={<Code2 size={14} />}><p className="task-detail-copy">{task.implementationNotes || "No implementation notes recorded."}</p></TaskSection>
    <TaskSection title="Relevant paths / context" icon={<FileCode2 size={14} />} count={task.relevantPaths.length}>
      {task.relevantPaths.length === 0 ? <p className="task-detail-empty">No relevant paths recorded.</p> : <ul className="token-list">{task.relevantPaths.map((path) => <li key={path}><code>{path}</code></li>)}</ul>}
    </TaskSection>
    <ArchitectureContext decisions={architectureDecisions} />
    <TaskSection title="Dependencies" icon={<GitBranch size={14} />} count={task.dependencyIds.length}>
      {task.dependencyIds.length === 0 ? <p className="task-detail-empty">No task dependencies recorded.</p> : <><ul className="token-list">{task.dependencyIds.map((dependency) => <li key={dependency}><code>{dependency}</code></li>)}</ul><p className="task-detail-hint">Dependencies must be Done before this task can run.</p></>}
    </TaskSection>
    <TaskSection title="Worker capabilities" icon={<Terminal size={14} />} count={task.requiredCapabilities.length}>
      {task.requiredCapabilities.length === 0 ? <p className="task-detail-empty">Any provider-ready project worker may execute this task.</p> : <><ul className="token-list">{task.requiredCapabilities.map((capability) => <li key={capability}><code>{capability}</code></li>)}</ul><p className="task-detail-hint">The scheduler matches these against tools, labels, OS, and architecture.</p></>}
    </TaskSection>
    <TaskSection title="Priority"><p className="task-detail-copy"><span className={`task-priority ${task.priority}`}>{task.priority}</span></p></TaskSection>
    {task.status === "blocked" && <TaskSection title="Blocked"><p className="task-blocked-reason">{task.blockedReason || "This task is waiting for workflow requirements."}</p></TaskSection>}
    <TaskSection title="Assigned agent" icon={<Bot size={14} />}>
      {assignedAgent ? <p className="task-detail-copy"><strong>{assignedAgent.name}</strong><br />{assignedAgent.role}{assignedAgent.model ? ` / ${assignedAgent.model}` : ""}</p> : <p className="task-detail-empty">No agent assigned.</p>}
    </TaskSection>
    <IsolationSection task={task} hasActiveRun={Boolean(activeRun)} isCleaning={isCleaningWorktree} isOpening={isOpeningWorktree} onCleanup={onCleanupWorktree} onOpen={onOpenWorktree} />
  </>;
}

function ReadinessSection({ task, workflowView }: { task: Task; workflowView: WorkflowTaskView }) {
  const readiness = workflowView.readiness;
  if (!readiness) return null;
  const summary = readiness.reason ?? (readiness.ready ? "All current readiness requirements are satisfied." : "Complete the remaining workflow requirements.");
  return <TaskSection title="Readiness" icon={<CheckCircle2 size={14} />}>
    <div className={`task-readiness-summary ${readiness.ready ? "ready" : "waiting"}`}><strong>{readiness.ready ? "Ready for scheduling" : "Waiting"}</strong><span>{summary}</span></div>
    <ul className="task-readiness-checklist"><li className={task.acceptanceCriteria.length > 0 ? "complete" : "incomplete"}>{task.acceptanceCriteria.length > 0 ? "Acceptance criteria recorded" : "Acceptance criteria required"}</li><li className={readiness.ready ? "complete" : "incomplete"}>Readiness service: {readiness.ready ? "eligible" : "not eligible"}</li></ul>
  </TaskSection>;
}

function ArchitectureContext({ decisions }: { decisions: ArchitectureDecision[] }) {
  return <TaskSection title="Architecture context" icon={<BookOpenCheck size={14} />} count={decisions.length}>
    {decisions.length === 0 ? <p className="task-detail-empty">No accepted managed ADRs apply. Repository instructions and architecture docs still apply.</p> : <div className="task-architecture-context">{decisions.map((item) => <article key={item.id}><header><code>ADR-{String(item.decisionNumber).padStart(3, "0")}</code><strong>{item.title}</strong></header><p>{item.decision}</p>{item.consequences && <small>{item.consequences}</small>}</article>)}</div>}
    <p className="task-detail-hint">This is the managed decision context injected into implementation and architect-review runs.</p>
  </TaskSection>;
}

function IsolationSection({ task, hasActiveRun, isCleaning, isOpening, onCleanup, onOpen }: { task: Task; hasActiveRun: boolean; isCleaning: boolean; isOpening: boolean; onCleanup: () => void; onOpen: () => void }) {
  if (!task.branch && !task.worktreePath) return null;
  return <TaskSection title="Isolation" icon={<GitBranch size={14} />}>
    {task.branch && <p className="task-detail-copy"><span className="task-detail-label">Branch</span><code>{task.branch}</code></p>}
    {task.worktreePath ? <><p className="task-detail-copy"><span className="task-detail-label">Worktree</span><code className="task-worktree-path">{task.worktreePath}</code></p><div className="task-worktree-actions"><button className="secondary-button" type="button" disabled={isOpening} onClick={onOpen}><FolderOpen size={14} /> {isOpening ? "Opening..." : "Open folder"}</button><button className="secondary-button" type="button" disabled={hasActiveRun || isCleaning} onClick={onCleanup}>{isCleaning ? "Removing..." : "Remove worktree"}</button></div><p className="task-detail-hint">Open the isolated checkout to inspect the agent's files. Removing it retains the task branch for review.</p></> : <p className="task-detail-hint">The task branch is retained; its isolated checkout has been removed.</p>}
  </TaskSection>;
}

function ActivityTab({ task, assignedAgent, recoveryAgents, inputRequests, runs, isStartingRun, runRecoveryAction, inputAction, cancellingRunId, onStartRun, onCancelRun, onRecoverRun, onResolveRunFailure, onRequestInput, onAnswerInput, now }: TaskDetailPanelProps & { now: number }) {
  const activeRun = runs.find((run) => run.status === "queued" || run.status === "running");
  const latestRun = activeRun ?? runs[0];
  const canStart = Boolean(assignedAgent) && task.status === "ready" && !activeRun;
  const showInput = task.status === "in_progress" || task.status === "needs_input" || inputRequests.length > 0;
  return <>
    {showInput && <InputRequestSection task={task} requests={inputRequests} latestRun={latestRun} activeRun={activeRun} inputAction={inputAction} onRequest={onRequestInput} onAnswer={onAnswerInput} />}
    <TaskSection title="Execution" icon={<Terminal size={14} />}>
      <div className="task-run-actions">
        <button className="primary-button" type="button" disabled={!canStart || isStartingRun} onClick={onStartRun}><Play size={15} /> {isStartingRun ? "Queuing..." : "Queue with Codex"}</button>
        {activeRun && <button className="secondary-button" type="button" disabled={cancellingRunId === activeRun.id} onClick={() => onCancelRun(activeRun.id)}><Square size={14} /> {cancellingRunId === activeRun.id ? "Cancelling..." : activeRun.status === "queued" ? "Remove from queue" : "Cancel"}</button>}
      </div>
      <p className="task-detail-hint">{executionHint(task, assignedAgent, activeRun)}</p>
      {isRecoverableRun(task, latestRun) && <RunRecoveryPanel run={latestRun} agents={recoveryAgents} action={runRecoveryAction} onRecover={onRecoverRun} onResolve={onResolveRunFailure} />}
      {latestRun ? <RunSummary run={latestRun} now={now} /> : <p className="task-detail-empty">No runs recorded for this task.</p>}
    </TaskSection>
  </>;
}

function executionHint(task: Task, assignedAgent: Agent | undefined, activeRun: TaskRun | undefined): string {
  if (!assignedAgent) return "Assign a Codex agent before starting this task.";
  if (task.status === "blocked") return "Resolve the blocked requirement before starting this task.";
  if (activeRun?.status === "queued") return "Waiting for worker, agent, and downstream WIP capacity.";
  if (task.status !== "ready" && !activeRun) return "Only Ready tasks can be queued. Successful runs are sent to Review for human approval.";
  return "Codex runs in an isolated task worktree. Successful runs move the task to Review.";
}

function isRecoverableRun(task: Task, run: TaskRun | undefined): run is TaskRun {
  return task.status === "in_progress" && Boolean(run && (run.status === "failed" || run.status === "cancelled"));
}

function RunRecoveryPanel({ run, agents, action, onRecover, onResolve }: { run: TaskRun; agents: Agent[]; action?: string; onRecover: TaskDetailPanelProps["onRecoverRun"]; onResolve: TaskDetailPanelProps["onResolveRunFailure"] }) {
  const alternatives = agents.filter((agent) => agent.id !== run.agentId);
  const [agentId, setAgentId] = useState(alternatives[0]?.id ?? "");
  useEffect(() => {
    if (!alternatives.some((agent) => agent.id === agentId)) setAgentId(alternatives[0]?.id ?? "");
  }, [agentId, alternatives]);
  return <div className="run-recovery-panel">
    <div className="run-recovery-heading"><CircleAlert size={15} /><div><strong>Run needs recovery</strong><p>The branch, worktree, output, and timeline remain available.</p></div></div>
    <div className="run-recovery-actions"><button className="primary-button" type="button" disabled={Boolean(action)} onClick={() => onRecover(run.id, "resume")}><RotateCcw size={14} /> {action === "resume" ? "Resuming..." : "Resume worktree"}</button><button className="secondary-button" type="button" disabled={Boolean(action)} onClick={() => onRecover(run.id, "restart_clean")}>{action === "restart_clean" ? "Restarting..." : "Restart clean"}</button></div>
    {alternatives.length > 0 && <div className="run-recovery-reassign"><select value={agentId} onChange={(event) => setAgentId(event.target.value)}>{alternatives.map((agent) => <option key={agent.id} value={agent.id}>{agent.name} / {agent.role}</option>)}</select><button className="secondary-button" type="button" disabled={!agentId || Boolean(action)} onClick={() => onRecover(run.id, "resume", agentId)}>{action?.startsWith("reassign:") ? "Reassigning..." : "Retry with agent"}</button></div>}
    <div className="run-recovery-secondary"><button type="button" disabled={Boolean(action)} onClick={() => onResolve(run.id, "escalate")}>Escalate as blocked</button><button type="button" disabled={Boolean(action)} onClick={() => onResolve(run.id, "abandon")}>Abandon recovery</button></div>
  </div>;
}

function ReviewLandTab({ task, reviewerAgents, agentReviews, isAgentReviewStarting, cancellingRunId, review, reviewError, isReviewLoading, isReviewActionPending, integrationAttempts = [], revertAttempts = [], validationAttempts = [], onCancelRun, onApproveReview, onRequestChanges, onStartAgentReview, now }: TaskDetailPanelProps & { now: number }) {
  const activeReview = agentReviews.find((item) => item.status === "running");
  const [reviewerId, setReviewerId] = useState(reviewerAgents[0]?.id ?? "");
  useEffect(() => {
    if (!reviewerAgents.some((agent) => agent.id === reviewerId)) setReviewerId(reviewerAgents[0]?.id ?? "");
  }, [reviewerAgents, reviewerId]);
  return <>
    <ArchitectReviewSection task={task} agents={reviewerAgents} reviews={agentReviews} activeReview={activeReview} reviewerId={reviewerId} isStarting={isAgentReviewStarting} cancellingRunId={cancellingRunId} now={now} onReviewerChange={setReviewerId} onCancel={onCancelRun} onStart={onStartAgentReview} />
    {task.status === "review" ? <BranchReview review={review} error={reviewError} isLoading={isReviewLoading} isPending={isReviewActionPending} hasActiveAgentReview={Boolean(activeReview)} onApprove={onApproveReview} onRequestChanges={onRequestChanges} /> : <TaskSection title="Landing status" icon={<GitBranch size={14} />}><p className="task-detail-copy">{landingStatusGuidance(task)}</p></TaskSection>}
    <DeliveryEvidence task={task} integrations={integrationAttempts} reverts={revertAttempts} validations={validationAttempts} />
  </>;
}

function ArchitectReviewSection({ task, agents, reviews, activeReview, reviewerId, isStarting, cancellingRunId, now, onReviewerChange, onCancel, onStart }: { task: Task; agents: Agent[]; reviews: AgentReview[]; activeReview?: AgentReview; reviewerId: string; isStarting: boolean; cancellingRunId?: string; now: number; onReviewerChange: (id: string) => void; onCancel: (runId: string) => void; onStart: (agentId: string) => void }) {
  if (task.status !== "review" && reviews.length === 0) return null;
  return <TaskSection title="Architect review" icon={<Bot size={14} />} count={reviews.length || undefined}>
    {reviews.length > 0 && <ArchitectReviewHistory reviews={reviews} agents={agents} now={now} cancellingRunId={cancellingRunId} onCancel={onCancel} />}
    {task.status === "review" && (agents.length === 0 ? <p className="task-detail-hint">Create a separate Codex agent to run an architect review. The implementation agent cannot review its own task.</p> : <div className="agent-review-controls"><select value={reviewerId} onChange={(event) => onReviewerChange(event.target.value)}>{agents.map((agent) => <option key={agent.id} value={agent.id}>{agent.name} / {agent.role}</option>)}</select><button className="secondary-button" type="button" disabled={!reviewerId || isStarting || Boolean(activeReview)} onClick={() => onStart(reviewerId)}>{isStarting ? "Starting architect..." : activeReview ? "Architect reviewing..." : reviews.length > 0 ? "Run another review" : "Run architect review"}</button></div>)}
    {task.status === "review" && <p className="task-detail-hint">The architect inspects the branch in read-only mode. Its decision is persisted here even after the task moves to Approved or back to In Progress.</p>}
  </TaskSection>;
}

function BranchReview({ review, error, isLoading, isPending, hasActiveAgentReview, onApprove, onRequestChanges }: { review?: TaskReview; error?: string; isLoading: boolean; isPending: boolean; hasActiveAgentReview: boolean; onApprove: () => void; onRequestChanges: () => void }) {
  if (isLoading) return <TaskSection title="Branch review" icon={<GitBranch size={14} />}><p className="task-detail-empty">Loading task branch changes...</p></TaskSection>;
  if (error) return <TaskSection title="Branch review" icon={<GitBranch size={14} />}><p className="task-run-error">{error}</p></TaskSection>;
  if (!review) return <TaskSection title="Branch review" icon={<GitBranch size={14} />}><p className="task-detail-empty">No branch review is available.</p></TaskSection>;
  return <TaskSection title="Branch review" icon={<GitBranch size={14} />}>
    <p className="task-detail-hint">{review.branch} compared with {review.baseBranch}</p><div className="review-actions"><button className="primary-button" type="button" disabled={isPending || hasActiveAgentReview} onClick={onApprove}>Approve for integration</button><button className="secondary-button" type="button" disabled={isPending || hasActiveAgentReview} onClick={onRequestChanges}>Request changes</button></div><p className="task-detail-hint">Approval queues a serialized squash merge; it does not mark the task Done.</p>
    <h4>Commits <span>{review.commits.length}</span></h4>{review.commits.length === 0 ? <p className="task-detail-empty">No commits on the task branch yet.</p> : <ul className="review-commit-list">{review.commits.map((commit) => <li key={commit.hash}><code>{commit.shortHash}</code><span>{commit.subject}</span></li>)}</ul>}
    <h4>Diff</h4>{review.diff ? <pre className="review-diff">{review.diff}</pre> : <p className="task-detail-empty">No tracked changes are available yet.</p>}{review.changedFiles.length > 0 && <p className="task-detail-hint">Uncommitted files: {review.changedFiles.map((file) => file.path).join(", ")}</p>}
  </TaskSection>;
}

function defaultDetailTab(workflowStage: WorkflowTaskView["stage"]): DetailTab {
  if (workflowStage === "verify" || workflowStage === "done") return "review";
  return workflowStage === "build" ? "activity" : "work";
}

function detailTabForKey(currentTab: DetailTab, key: string): DetailTab | undefined {
  const currentIndex = DETAIL_TABS.findIndex((tab) => tab.id === currentTab);
  if (key === "ArrowRight") return DETAIL_TABS[(currentIndex + 1) % DETAIL_TABS.length].id;
  if (key === "ArrowLeft") return DETAIL_TABS[(currentIndex - 1 + DETAIL_TABS.length) % DETAIL_TABS.length].id;
  if (key === "Home") return DETAIL_TABS[0].id;
  if (key === "End") return DETAIL_TABS[DETAIL_TABS.length - 1].id;
  return undefined;
}

function workflowPhaseSummary(view: WorkflowTaskView) {
  const stage = { queue: "Queue", build: "Build", verify: "Verify & Land", done: "Done" }[view.stage];
  const actor = view.currentActor?.label ?? "Unassigned";
  const actorKind = view.currentActor?.kind ?? "system";
  const nextAction = view.nextAction.reason ? `${view.nextAction.label}: ${view.nextAction.reason}` : view.nextAction.label;
  return { stage, actor, actorKind, nextAction };
}

function DeliveryEvidence({ task, integrations, reverts, validations }: { task: Task; integrations: IntegrationAttempt[]; reverts: RevertAttempt[]; validations: ValidationAttempt[] }) {
  const taskIntegrations = integrations.filter((attempt) => attempt.taskId === task.id);
  const taskReverts = reverts.filter((attempt) => attempt.originalTaskId === task.id);
  const taskValidations = validations.filter((attempt) => attempt.taskId === task.id);
  const evidenceCount = taskIntegrations.length + taskReverts.length + taskValidations.length;
  return <TaskSection title="Delivery evidence" icon={<CheckCircle2 size={14} />} count={evidenceCount}>
    {evidenceCount === 0 ? <p className="task-detail-empty">No validation, integration, cleanup, or revert attempts recorded for this task.</p> : <div className="delivery-evidence-list">
      {taskValidations.map((attempt) => <article key={attempt.id}><header><strong>{attempt.stage} validation</strong><span className={`run-status ${attempt.status}`}>{attempt.status}</span></header><small>{dateTime(attempt.completedAt ?? attempt.startedAt)}</small>{attempt.error && <p>{attempt.error}</p>}</article>)}
      {taskIntegrations.map((attempt) => <article key={attempt.id}><header><strong>{attempt.sourceBranch} → {attempt.targetBranch}</strong><span className={`run-status ${attempt.status}`}>{attempt.status}</span></header><small>{dateTime(attempt.completedAt ?? attempt.startedAt ?? attempt.createdAt)}</small>{attempt.mergeCommit && <code>{attempt.mergeCommit}</code>}{attempt.status === "merged" && <p className={attempt.error ? "evidence-error" : "evidence-success"}>{attempt.error ? `Cleanup needs recovery: ${attempt.error}` : "Integration and cleanup completed."}</p>}{attempt.status !== "merged" && attempt.error && <p className="evidence-error">{attempt.error}</p>}</article>)}
      {taskReverts.map((attempt) => <article key={attempt.id}><header><strong>Revert</strong><span className={`run-status ${attempt.status}`}>{attempt.status.replace("_", " ")}</span></header><small>{dateTime(attempt.completedAt ?? attempt.startedAt)}</small>{attempt.revertCommit && <code>{attempt.revertCommit}</code>}{attempt.error && <p className="evidence-error">{attempt.error}</p>}</article>)}
    </div>}
  </TaskSection>;
}

function landingStatusGuidance(task: Task) {
  switch (task.status) {
    case "approved": return "Approved and waiting in the serialized integration queue.";
    case "integrating": return "Updating and validating the task against the latest integration branch.";
    case "done": return "Accepted changes are integrated and the integration branch is healthy.";
    case "blocked": return task.blockedReason || "Landing is blocked. Resolve the recorded conflict or validation failure before retrying.";
    default: return "Review and integration evidence will appear after implementation validation succeeds.";
  }
}

function statusLabel(status: Task["status"]) {
  return status.replaceAll("_", " ");
}

function relativeAge(value: string, now: number) {
  const elapsed = Math.max(0, now - timestamp(value));
  const minutes = Math.floor(elapsed / 60_000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

function InputRequestSection({ task, requests, latestRun, activeRun, inputAction, onRequest, onAnswer }: { task: Task; requests: TaskInputRequest[]; latestRun?: TaskRun; activeRun?: TaskRun; inputAction?: "request" | "answer"; onRequest: (question: string, runId?: string) => void; onAnswer: (requestId: string, answer: string) => void }) {
  const [question, setQuestion] = useState("");
  const [answer, setAnswer] = useState("");
  const openRequest = requests.find((request) => request.status === "open");
  const answered = requests.filter((request) => request.status === "answered");
  return <TaskSection title="Human input" icon={<MessageSquareText size={14} />} count={requests.length || undefined}>
    {openRequest ? <div className="task-input-open">
      <span>Waiting for answer</span><strong>{openRequest.question}</strong>
      <small>{openRequest.requestingRunId ? `Requested by run ${openRequest.requestingRunId}` : "Task-level request"} · {dateTime(openRequest.requestedAt)}</small>
      {activeRun?.status === "running" ? <p className="task-detail-hint">The worker is stopping safely. Answering becomes available when the run is paused.</p> : <><textarea rows={3} value={answer} onChange={(event) => setAnswer(event.target.value)} placeholder="Record the decision or missing information..." /><button className="primary-button" type="button" disabled={!answer.trim() || Boolean(inputAction)} onClick={() => onAnswer(openRequest.id, answer)}>{inputAction === "answer" ? "Saving answer..." : "Answer and resume"}</button></>}
    </div> : task.status === "in_progress" && <div className="task-input-form"><p className="task-detail-hint">Stop the active implementation and ask instead of allowing the agent to guess.</p><textarea rows={3} value={question} onChange={(event) => setQuestion(event.target.value)} placeholder="What decision or information is required?" /><button className="secondary-button" type="button" disabled={!question.trim() || Boolean(inputAction)} onClick={() => onRequest(question, latestRun?.id)}>{inputAction === "request" ? "Pausing..." : "Request human input"}</button></div>}
    {answered.length > 0 && <details className="task-input-history"><summary>Answered requests · {answered.length}</summary>{answered.map((request) => <article key={request.id}><strong>{request.question}</strong><p>{request.answer}</p><small>{dateTime(request.answeredAt ?? request.requestedAt)}</small></article>)}</details>}
  </TaskSection>;
}

function ArchitectReviewHistory({ reviews, agents, now, cancellingRunId, onCancel }: { reviews: AgentReview[]; agents: Agent[]; now: number; cancellingRunId?: string; onCancel: (runId: string) => void }) {
  return <div className="agent-review-list" aria-live="polite">{reviews.map((review, index) => {
    const outcome = architectReviewOutcome(review);
    const runtimeEnd = review.completedAt ? timestamp(review.completedAt) : now;
    const runtime = formatDuration(Math.max(0, runtimeEnd - timestamp(review.startedAt)));
    const outputEvents = review.rawOutput.trim() ? review.rawOutput.trim().split("\n").length : 0;
    const reviewer = agents.find((agent) => agent.id === review.agentId)?.name ?? "Removed agent";
    return <article className={`agent-review-card ${outcome.tone}`} key={review.id}>
      <header className="agent-review-card-header">
        <span className={`agent-review-icon ${outcome.tone}`}>{outcome.icon}</span>
        <div><strong>{outcome.title}</strong><p>{outcome.detail}</p></div>
        <span className={`agent-review-status ${outcome.tone}`}>{outcome.label}</span>
      </header>
      {review.status === "running" && <div className="agent-review-progress" aria-label="Architect review in progress"><span /><span /><span /></div>}
      <div className="agent-review-meta"><span>{reviewer}</span><span>{runtime}</span><span>{dateTime(review.completedAt ?? review.startedAt)}</span>{reviews.length > 1 && <span>Attempt {reviews.length - index}</span>}</div>
      {review.status === "running" && <p className="agent-review-activity">Inspecting acceptance criteria, commits, diff, and validation evidence{outputEvents > 0 ? ` · ${outputEvents} provider events received` : ""}.</p>}
      {review.notes && <div className="agent-review-notes"><span>Architect notes</span><p>{review.notes}</p></div>}
      {review.error && <p className="task-run-error">{review.error}</p>}
      <div className="agent-review-card-actions">
        {review.status === "running" && <button className="secondary-button" type="button" disabled={cancellingRunId === review.id} onClick={() => onCancel(review.id)}>{cancellingRunId === review.id ? "Cancelling..." : "Cancel architect"}</button>}
        {review.rawOutput && <details className="agent-review-output"><summary>Provider transcript · {outputEvents} events</summary><pre>{review.rawOutput}</pre></details>}
      </div>
    </article>;
  })}</div>;
}

function architectReviewOutcome(review: AgentReview): { tone: string; label: string; title: string; detail: string; icon: ReactNode } {
  if (review.status === "running") return { tone: "running", label: "Reviewing", title: "Architect review in progress", detail: "The task stays in Review until a durable decision is recorded.", icon: <LoaderCircle size={15} className="spin" /> };
  if (review.status === "failed") return { tone: "failed", label: "Failed", title: "Architect review failed", detail: "No workflow decision was applied. The task remains available for another review.", icon: <CircleAlert size={15} /> };
  if (review.status === "cancelled") return { tone: "cancelled", label: "Cancelled", title: "Architect review cancelled", detail: "No workflow decision was applied.", icon: <Square size={13} /> };
  if (review.decision === "approve") return { tone: "approve", label: "Approved", title: "Architect approved the implementation", detail: "The task moved to Approved and entered the serialized integration queue.", icon: <CheckCircle2 size={15} /> };
  if (review.decision === "request_changes") return { tone: "request_changes", label: "Changes", title: "Architect requested changes", detail: "The task returned to In Progress with the architect's notes retained below.", icon: <RotateCcw size={15} /> };
  return { tone: "completed", label: "Completed", title: "Architect review completed", detail: "The review finished without a workflow decision.", icon: <CheckCircle2 size={15} /> };
}

function TaskSection({ title, icon, count, children }: { title: string; icon?: ReactNode; count?: number; children: ReactNode }) {
  return <section className="task-detail-section"><h3>{icon}{title}{count !== undefined && <span>{count}</span>}</h3>{children}</section>;
}

function RunSummary({ run, now }: { run: TaskRun; now: number }) {
  const runtimeEnd = run.completedAt ? timestamp(run.completedAt) : now;
  const runtime = Math.max(0, runtimeEnd - timestamp(run.startedAt));
  const timelineRef = useRef<HTMLDivElement>(null);
  const [isExporting, setIsExporting] = useState(false);
  const [exportError, setExportError] = useState<string>();

  useEffect(() => {
    timelineRef.current?.scrollTo({ top: timelineRef.current.scrollHeight });
  }, [run.id, run.events.length]);

  const exportRawLog = async () => {
    setExportError(undefined);
    try {
      const destination = await save({
        title: "Save raw execution log",
        defaultPath: `orchestr-run-${run.id}.txt`,
        filters: [{ name: "Text log", extensions: ["txt"] }],
      });
      if (!destination) return;
      setIsExporting(true);
      await exportTaskRunLog(run.id, destination);
    } catch (error) {
      setExportError(error instanceof Error ? error.message : "Unable to export the raw execution log.");
    } finally {
      setIsExporting(false);
    }
  };

  return <div className="task-run-summary">
    <div className="task-run-meta"><span className={`run-status ${run.status}`}>{run.status}</span><span>{run.workerId === "local" ? "local worker" : run.workerId}</span><span>{formatDuration(runtime)}</span>{run.exitCode !== null && <span>exit {run.exitCode}</span>}<button className="task-run-log-link" type="button" disabled={isExporting} onClick={() => void exportRawLog()}><Download size={13} /> {isExporting ? "Saving..." : "Log.txt"}</button></div>
    {run.error && <p className="task-run-error">{run.error}</p>}
    {exportError && <p className="task-run-error">{exportError}</p>}
    <div ref={timelineRef} className="task-run-timeline" aria-live="polite">{run.events.length === 0 ? <p>Waiting for Codex events...</p> : run.events.map((event) => <TimelineEvent event={event} key={event.id} />)}</div>
  </div>;
}

function TimelineEvent({ event }: { event: RunEvent }) {
  const command = event.command ?? commandFromProviderStatus(event.message);
  const activity = command ? commandActivity(command, commandActivityKind(event)) : undefined;
  const message = shouldShowEventMessage(event, command) ? event.message : undefined;
  return <div className={`timeline-event ${event.kind.replaceAll(".", "-")}`}>
    <time>{time(event.createdAt)}</time><span className="timeline-marker" /><div><strong>{activity ?? eventLabel(event.kind)}</strong>{message && <EventMessage text={message} failed={event.exitCode !== null && event.exitCode !== 0} />}{event.filePath && <code>{event.filePath}</code>}{event.exitCode !== null && <span className="timeline-exit">exit {event.exitCode}</span>}</div>
  </div>;
}

function EventMessage({ text, failed }: { text: string; failed: boolean }) {
  const lineCount = text.split("\n").length;
  if (lineCount > 4) return <details className="timeline-output" open={failed}><summary>Output · {lineCount} lines</summary><pre>{text}</pre></details>;
  return <p>{text}</p>;
}

function shouldShowEventMessage(event: RunEvent, command?: string) {
  if (!command) return true;
  const normalized = event.message.trim();
  return normalized !== `in_progress: ${command}` && normalized !== `completed: ${command}` && normalized !== `failed: ${command}`;
}

function commandFromProviderStatus(message: string) {
  const match = /^(?:in_progress|completed|failed):\s+(.+)$/s.exec(message.trim());
  return match?.[1];
}

function commandActivityKind(event: RunEvent) {
  if (event.kind !== "command.output") return event.kind;
  if (event.message.startsWith("in_progress:")) return "command.started";
  return "command.completed";
}

function eventLabel(kind: string) {
  const labels: Record<string, string> = {
    "agent.session_started": "Codex session started",
    "agent.started": "Codex started working",
    "agent.completed": "Codex finished working",
    "agent.reasoning": "Thinking",
    "agent.message": "Codex",
    "file.modified": "Changed files",
    "provider.error": "Provider error",
    "run.queued": "Run queued",
    "run.completed": "Run completed",
    "run.failed": "Run failed",
  };
  return labels[kind] ?? kind.replaceAll(".", " · ");
}

const COMMAND_ACTIVITY_RULES = [
  { pattern: /\b(git status|git diff|git log|git show|git branch)\b/, label: "repository inspection" },
  { pattern: /\b(rg|find|fd|ls|dir|cat|type|get-content|select-string)\b/, label: "file inspection" },
  { pattern: /\bgit (add|commit|restore|checkout|rebase|merge)\b/, label: "Git update" },
  { pattern: /\b(npm|pnpm|yarn|bun)\b/, label: "JavaScript task" },
  { pattern: /\bcargo\b/, label: "Rust task" },
  { pattern: /\b(pytest|python)\b/, label: "Python task" },
  { pattern: /\b(gradle|mvn|java)\b/, label: "JVM task" },
  { pattern: /\b(write|set-content|out-file|copy-item|move-item|mkdir|new-item)\b/, label: "file update" },
] as const;

function commandActivity(command: string, kind: string) {
  const action = kind.startsWith("validation") ? "Checking" : kind === "command.started" ? "Running" : "Finished";
  const normalized = command.toLowerCase();
  const rule = COMMAND_ACTIVITY_RULES.find(({ pattern }) => pattern.test(normalized));
  return `${action} ${rule?.label ?? "command"}`;
}

function timestamp(value: string) { return Date.parse(value.endsWith("Z") ? value : `${value}Z`); }

function time(value: string) { return new Date(timestamp(value)).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" }); }

function dateTime(value: string) { return new Date(timestamp(value)).toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" }); }

function formatDuration(milliseconds: number) {
  const seconds = Math.floor(milliseconds / 1_000);
  return `${Math.floor(seconds / 60)}m ${String(seconds % 60).padStart(2, "0")}s`;
}
