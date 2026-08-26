import { BookOpenCheck, Bot, CheckCircle2, CheckSquare, CircleAlert, Code2, Download, FileCode2, FolderOpen, GitBranch, LoaderCircle, MessageSquareText, Pencil, Play, RotateCcw, Square, Terminal, X } from "lucide-react";
import { save } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState, type ReactNode } from "react";
import type { Agent } from "../../services/agents";
import { exportTaskRunLog, type RunEvent, type TaskRun } from "../../services/runs";
import type { Task } from "../../services/tasks";
import type { TaskReview } from "../../services/reviews";
import type { AgentReview } from "../../services/agentReviews";
import type { TaskInputRequest } from "../../services/interruptions";
import type { ArchitectureDecision } from "../../services/knowledge";
import "./TaskDetailPanel.css";

type TaskDetailPanelProps = {
  task: Task;
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

export function TaskDetailPanel({ task, assignedAgent, recoveryAgents, reviewerAgents, agentReviews, inputRequests, architectureDecisions, isAgentReviewStarting, runs, isStartingRun, runRecoveryAction, inputAction, cancellingRunId, isCleaningWorktree, isOpeningWorktree, review, reviewError, isReviewLoading, isReviewActionPending, onClose, onEdit, onStartRun, onCancelRun, onRecoverRun, onResolveRunFailure, onRequestInput, onAnswerInput, onCleanupWorktree, onOpenWorktree, onApproveReview, onRequestChanges, onStartAgentReview }: TaskDetailPanelProps) {
  const activeRun = runs.find((run) => run.status === "queued" || run.status === "running");
  const activeAgentReview = agentReviews.find((review) => review.status === "running");
  const latestRun = activeRun ?? runs[0];
  const [now, setNow] = useState(() => Date.now());
  const [reviewerId, setReviewerId] = useState("");
  const [recoveryAgentId, setRecoveryAgentId] = useState("");

  useEffect(() => {
    if (!activeRun && !activeAgentReview) return undefined;
    const interval = window.setInterval(() => setNow(Date.now()), 1_000);
    return () => window.clearInterval(interval);
  }, [activeAgentReview, activeRun]);

  useEffect(() => {
    if (!reviewerAgents.some((agent) => agent.id === reviewerId)) setReviewerId(reviewerAgents[0]?.id ?? "");
  }, [reviewerAgents, reviewerId]);

  useEffect(() => {
    const alternative = recoveryAgents.find((agent) => agent.id !== latestRun?.agentId);
    if (!recoveryAgents.some((agent) => agent.id === recoveryAgentId && agent.id !== latestRun?.agentId)) setRecoveryAgentId(alternative?.id ?? "");
  }, [latestRun?.agentId, recoveryAgentId, recoveryAgents]);

  const canStart = Boolean(assignedAgent) && task.status === "ready" && !activeRun;
  return (
    <aside className="board-inspector-panel task-detail-panel" aria-label={`Task details for ${task.title}`}>
      <header className="task-detail-header">
        <div><p className="eyebrow">Task specification</p><h2>{task.title}</h2><code>{task.id}</code></div>
        <div className="task-detail-actions">
          <button className="icon-button" type="button" onClick={() => onEdit(task)} aria-label={`Edit ${task.title}`}><Pencil size={16} /></button>
          <button className="icon-button" type="button" onClick={onClose} aria-label="Close task details"><X size={16} /></button>
        </div>
      </header>
      <div className="task-detail-content">
        <TaskSection title="Context">
          <p className="task-detail-copy">{task.description || "No additional context has been recorded."}</p>
        </TaskSection>
        <TaskSection title="Acceptance criteria" icon={<CheckSquare size={14} />} count={task.acceptanceCriteria.length}>
          {task.acceptanceCriteria.length === 0 ? <p className="task-detail-empty">No acceptance criteria recorded.</p> : <ul className="criteria-list">{task.acceptanceCriteria.map((criterion) => <li key={criterion}>{criterion}</li>)}</ul>}
        </TaskSection>
        <TaskSection title="Implementation notes" icon={<Code2 size={14} />}>
          <p className="task-detail-copy">{task.implementationNotes || "No implementation notes recorded."}</p>
        </TaskSection>
        <TaskSection title="Relevant paths / context" icon={<FileCode2 size={14} />} count={task.relevantPaths.length}>
          {task.relevantPaths.length === 0 ? <p className="task-detail-empty">No relevant paths recorded.</p> : <ul className="token-list">{task.relevantPaths.map((path) => <li key={path}><code>{path}</code></li>)}</ul>}
        </TaskSection>
        <TaskSection title="Architecture context" icon={<BookOpenCheck size={14} />} count={architectureDecisions.length}>
          {architectureDecisions.length === 0 ? <p className="task-detail-empty">No accepted managed ADRs apply. Repository instructions and architecture docs still apply.</p> : <div className="task-architecture-context">{architectureDecisions.map((item) => <article key={item.id}><header><code>ADR-{String(item.decisionNumber).padStart(3, "0")}</code><strong>{item.title}</strong></header><p>{item.decision}</p>{item.consequences && <small>{item.consequences}</small>}</article>)}</div>}
          <p className="task-detail-hint">This is the managed decision context injected into implementation and architect-review runs.</p>
        </TaskSection>
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
        {(task.status === "in_progress" || task.status === "needs_input" || inputRequests.length > 0) && <InputRequestSection task={task} requests={inputRequests} latestRun={latestRun} activeRun={activeRun} inputAction={inputAction} onRequest={onRequestInput} onAnswer={onAnswerInput} />}
        {(task.branch || task.worktreePath) && <TaskSection title="Isolation" icon={<GitBranch size={14} />}>
          {task.branch && <p className="task-detail-copy"><span className="task-detail-label">Branch</span><code>{task.branch}</code></p>}
          {task.worktreePath ? <><p className="task-detail-copy"><span className="task-detail-label">Worktree</span><code className="task-worktree-path">{task.worktreePath}</code></p><div className="task-worktree-actions"><button className="secondary-button" type="button" disabled={isOpeningWorktree} onClick={onOpenWorktree}><FolderOpen size={14} /> {isOpeningWorktree ? "Opening..." : "Open folder"}</button><button className="secondary-button" type="button" disabled={Boolean(activeRun) || isCleaningWorktree} onClick={onCleanupWorktree}>{isCleaningWorktree ? "Removing..." : "Remove worktree"}</button></div><p className="task-detail-hint">Open the isolated checkout to inspect the agent's files. Removing it retains the task branch for review.</p></> : <p className="task-detail-hint">The task branch is retained; its isolated checkout has been removed.</p>}
        </TaskSection>}
        {(task.status === "review" || agentReviews.length > 0) && <TaskSection title="Architect review" icon={<Bot size={14} />} count={agentReviews.length || undefined}>
          {agentReviews.length > 0 && <ArchitectReviewHistory reviews={agentReviews} agents={reviewerAgents} now={now} cancellingRunId={cancellingRunId} onCancel={onCancelRun} />}
          {task.status === "review" && (reviewerAgents.length === 0 ? <p className="task-detail-hint">Create a separate Codex agent to run an architect review. The implementation agent cannot review its own task.</p> : <div className="agent-review-controls"><select value={reviewerId} onChange={(event) => setReviewerId(event.target.value)}>{reviewerAgents.map((agent) => <option key={agent.id} value={agent.id}>{agent.name} / {agent.role}</option>)}</select><button className="secondary-button" type="button" disabled={!reviewerId || isAgentReviewStarting || Boolean(activeAgentReview)} onClick={() => onStartAgentReview(reviewerId)}>{isAgentReviewStarting ? "Starting architect..." : activeAgentReview ? "Architect reviewing..." : agentReviews.length > 0 ? "Run another review" : "Run architect review"}</button></div>)}
          {task.status === "review" && <p className="task-detail-hint">The architect inspects the branch in read-only mode. Its decision is persisted here even after the task moves to Approved or back to In Progress.</p>}
        </TaskSection>}
        {task.status === "review" && <TaskSection title="Branch review" icon={<GitBranch size={14} />}>
          {isReviewLoading ? <p className="task-detail-empty">Loading task branch changes...</p> : reviewError ? <p className="task-run-error">{reviewError}</p> : review && <><p className="task-detail-hint">{review.branch} compared with {review.baseBranch}</p><div className="review-actions"><button className="primary-button" type="button" disabled={isReviewActionPending || Boolean(activeAgentReview)} onClick={onApproveReview}>Approve for integration</button><button className="secondary-button" type="button" disabled={isReviewActionPending || Boolean(activeAgentReview)} onClick={onRequestChanges}>Request changes</button></div><p className="task-detail-hint">Approval queues a serialized squash merge; it does not mark the task Done.</p><h4>Commits <span>{review.commits.length}</span></h4>{review.commits.length === 0 ? <p className="task-detail-empty">No commits on the task branch yet.</p> : <ul className="review-commit-list">{review.commits.map((commit) => <li key={commit.hash}><code>{commit.shortHash}</code><span>{commit.subject}</span></li>)}</ul>}<h4>Diff</h4>{review.diff ? <pre className="review-diff">{review.diff}</pre> : <p className="task-detail-empty">No tracked changes are available yet.</p>}{review.changedFiles.length > 0 && <p className="task-detail-hint">Uncommitted files: {review.changedFiles.map((file) => file.path).join(", ")}</p>}</>}
        </TaskSection>}
        <TaskSection title="Execution" icon={<Terminal size={14} />}>
          <div className="task-run-actions">
            <button className="primary-button" type="button" disabled={!canStart || isStartingRun} onClick={onStartRun}><Play size={15} /> {isStartingRun ? "Queuing..." : "Queue with Codex"}</button>
            {activeRun && <button className="secondary-button" type="button" disabled={cancellingRunId === activeRun.id} onClick={() => onCancelRun(activeRun.id)}><Square size={14} /> {cancellingRunId === activeRun.id ? "Cancelling..." : activeRun.status === "queued" ? "Remove from queue" : "Cancel"}</button>}
          </div>
          {!assignedAgent ? <p className="task-detail-hint">Assign a Codex agent before starting this task.</p> : task.status === "blocked" ? <p className="task-detail-hint">Resolve the blocked requirement before starting this task.</p> : activeRun?.status === "queued" ? <p className="task-detail-hint">Waiting for worker, agent, and downstream WIP capacity.</p> : task.status !== "ready" && !activeRun ? <p className="task-detail-hint">Only Ready tasks can be queued. Successful runs are sent to Review for human approval.</p> : <p className="task-detail-hint">Codex runs in an isolated task worktree. Successful runs move the task to Review.</p>}
          {latestRun && task.status === "in_progress" && (latestRun.status === "failed" || latestRun.status === "cancelled") && <div className="run-recovery-panel">
            <div className="run-recovery-heading"><CircleAlert size={15} /><div><strong>Run needs recovery</strong><p>The branch, worktree, output, and timeline remain available.</p></div></div>
            <div className="run-recovery-actions">
              <button className="primary-button" type="button" disabled={Boolean(runRecoveryAction)} onClick={() => onRecoverRun(latestRun.id, "resume")}><RotateCcw size={14} /> {runRecoveryAction === "resume" ? "Resuming..." : "Resume worktree"}</button>
              <button className="secondary-button" type="button" disabled={Boolean(runRecoveryAction)} onClick={() => onRecoverRun(latestRun.id, "restart_clean")}>{runRecoveryAction === "restart_clean" ? "Restarting..." : "Restart clean"}</button>
            </div>
            {recoveryAgents.some((agent) => agent.id !== latestRun.agentId) && <div className="run-recovery-reassign"><select value={recoveryAgentId} onChange={(event) => setRecoveryAgentId(event.target.value)}>{recoveryAgents.filter((agent) => agent.id !== latestRun.agentId).map((agent) => <option key={agent.id} value={agent.id}>{agent.name} / {agent.role}</option>)}</select><button className="secondary-button" type="button" disabled={!recoveryAgentId || Boolean(runRecoveryAction)} onClick={() => onRecoverRun(latestRun.id, "resume", recoveryAgentId)}>{runRecoveryAction?.startsWith("reassign:") ? "Reassigning..." : "Retry with agent"}</button></div>}
            <div className="run-recovery-secondary"><button type="button" disabled={Boolean(runRecoveryAction)} onClick={() => onResolveRunFailure(latestRun.id, "escalate")}>Escalate as blocked</button><button type="button" disabled={Boolean(runRecoveryAction)} onClick={() => onResolveRunFailure(latestRun.id, "abandon")}>Abandon recovery</button></div>
          </div>}
          {latestRun ? <RunSummary run={latestRun} now={now} /> : <p className="task-detail-empty">No runs recorded for this task.</p>}
        </TaskSection>
      </div>
    </aside>
  );
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

function commandActivity(command: string, kind: string) {
  const normalized = command.toLowerCase();
  const action = kind.startsWith("validation") ? "Checking" : kind === "command.started" ? "Running" : "Finished";
  if (/\b(git status|git diff|git log|git show|git branch)\b/.test(normalized)) return `${action} repository inspection`;
  if (/\b(rg|find|fd|ls|dir|cat|type|get-content|select-string)\b/.test(normalized)) return `${action} file inspection`;
  if (/\bgit (add|commit|restore|checkout|rebase|merge)\b/.test(normalized)) return `${action} Git update`;
  if (/\b(npm|pnpm|yarn|bun)\b/.test(normalized)) return `${action} JavaScript task`;
  if (/\bcargo\b/.test(normalized)) return `${action} Rust task`;
  if (/\b(pytest|python)\b/.test(normalized)) return `${action} Python task`;
  if (/\b(gradle|mvn|java)\b/.test(normalized)) return `${action} JVM task`;
  if (/\b(write|set-content|out-file|copy-item|move-item|mkdir|new-item)\b/.test(normalized)) return `${action} file update`;
  return `${action} command`;
}

function timestamp(value: string) { return Date.parse(value.endsWith("Z") ? value : `${value}Z`); }

function time(value: string) { return new Date(timestamp(value)).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" }); }

function dateTime(value: string) { return new Date(timestamp(value)).toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" }); }

function formatDuration(milliseconds: number) {
  const seconds = Math.floor(milliseconds / 1_000);
  return `${Math.floor(seconds / 60)}m ${String(seconds % 60).padStart(2, "0")}s`;
}
