import { closestCorners, DndContext, DragOverlay, KeyboardSensor, pointerWithin, PointerSensor, useDroppable, useSensor, useSensors, type DragEndEvent } from "@dnd-kit/core";
import { SortableContext, sortableKeyboardCoordinates, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { Activity, ArrowLeft, BookOpenCheck, ChartNoAxesCombined, CheckCircle2, Gauge, GitBranch, GitMerge, LayoutGrid, ListTree, MessagesSquare, MoreHorizontal, PanelRightOpen, PlayCircle, Plus, ShieldAlert, Sparkles } from "lucide-react";
import { type ComponentProps, type ReactNode, type RefObject } from "react";
import { Link } from "react-router-dom";
import { CollaborationPanel } from "../../components/CollaborationPanel/CollaborationPanel";
import { FlowControlPanel } from "../../components/FlowControlPanel/FlowControlPanel";
import { IntegrationQueuePanel } from "../../components/IntegrationQueuePanel/IntegrationQueuePanel";
import { PlanningPanel } from "../../components/PlanningPanel/PlanningPanel";
import { ProjectBlockersPanel } from "../../components/ProjectBlockersPanel/ProjectBlockersPanel";
import { ProjectKnowledgePanel } from "../../components/ProjectKnowledgePanel/ProjectKnowledgePanel";
import { QualityGatesPanel } from "../../components/QualityGatesPanel/QualityGatesPanel";
import { RepositoryInspector } from "../../components/RepositoryInspector/RepositoryInspector";
import { TaskDetailPanel } from "../../components/TaskDetailPanel/TaskDetailPanel";
import { TaskDialog } from "../../components/TaskDialog/TaskDialog";
import { AgentActivityRail, AttentionTray, FlowStageColumn, WorkflowTaskCard, WorkflowTaskDragPreview, type WorkflowTaskActions } from "../../components/WorkflowCockpit/WorkflowCockpit";
import type { Agent } from "../../services/agents";
import type { Project, RepositoryDetails } from "../../services/projects";
import type { ProjectHealth } from "../../services/quality";
import { TASK_STATUSES, type Task, type TaskStatus } from "../../services/tasks";
import type { ProjectWorkflowSnapshot, WorkflowBoardView, WorkflowStage, WorkflowTaskView } from "../../services/workflow";

export type BoardSidePanel = "task" | "repository" | "integration" | "quality" | "flow" | "blockers" | "knowledge" | "planning" | "collaboration";

export function BoardTaskDialog({ isCreating, editingTask, ...props }: Omit<ComponentProps<typeof TaskDialog>, "task"> & { isCreating: boolean; editingTask?: Task | null }) {
  if (!isCreating && !editingTask) return null;
  return <TaskDialog {...props} task={editingTask ?? undefined} />;
}

type BoardHeaderProps = {
  project: Project;
  workflow?: ProjectWorkflowSnapshot;
  health?: ProjectHealth;
  repository?: RepositoryDetails;
  isRepositoryLoading: boolean;
  activeBlockerCount: number;
  queuedIntegrationCount: number;
  proposedPlanCount: number;
  openCollaborationCount: number;
  acceptedDecisionCount: number;
  activeFlowCount: number;
  queuedFlowCount: number;
  flowCapacity: number;
  isProjectToolsOpen: boolean;
  projectToolsRef: RefObject<HTMLDivElement | null>;
  onToggleProjectTools: () => void;
  onCloseProjectTools: () => void;
  onOpenSidePanel: (panel: Exclude<BoardSidePanel, "task">) => void;
  onOpenProjectToolPanel: (panel: Exclude<BoardSidePanel, "task">) => void;
  onNewTask: () => void;
};

export function BoardHeader({ project, workflow, health, repository, isRepositoryLoading, activeBlockerCount, queuedIntegrationCount, proposedPlanCount, openCollaborationCount, acceptedDecisionCount, activeFlowCount, queuedFlowCount, flowCapacity, isProjectToolsOpen, projectToolsRef, onToggleProjectTools, onCloseProjectTools, onOpenSidePanel, onOpenProjectToolPanel, onNewTask }: BoardHeaderProps) {
  return <header className="board-header">
    <Link className="back-link" to="/projects"><ArrowLeft size={15} /> Projects</Link>
    <div className="board-title-row">
      <ProjectTitle project={project} />
      <div className="board-header-actions">
        <ProjectStatusCluster project={project} workflow={workflow} health={health} repository={repository} isRepositoryLoading={isRepositoryLoading} activeFlowCount={activeFlowCount} queuedFlowCount={queuedFlowCount} flowCapacity={flowCapacity} onOpen={onOpenSidePanel} />
        <ProjectAlerts blockerCount={activeBlockerCount} integrationCount={queuedIntegrationCount} onOpen={onOpenSidePanel} />
        <ProjectControlsHost project={project} queuedIntegrationCount={queuedIntegrationCount} activeBlockerCount={activeBlockerCount} proposedPlanCount={proposedPlanCount} openCollaborationCount={openCollaborationCount} acceptedDecisionCount={acceptedDecisionCount} isOpen={isProjectToolsOpen} hostRef={projectToolsRef} onToggle={onToggleProjectTools} onClose={onCloseProjectTools} onOpen={onOpenProjectToolPanel} />
        <button className="primary-button" type="button" onClick={onNewTask}><Plus size={16} /> New task</button>
      </div>
    </div>
  </header>;
}

function ProjectTitle({ project }: { project: Project }) {
  return <div><p className="eyebrow">{project.defaultBranch} / local workspace</p><h1>{project.name}</h1><p className="muted">{project.description ? project.description : "Project task board"}</p></div>;
}

function ProjectStatusCluster({ project, workflow, health, repository, isRepositoryLoading, activeFlowCount, queuedFlowCount, flowCapacity, onOpen }: Pick<BoardHeaderProps, "project" | "workflow" | "health" | "repository" | "isRepositoryLoading" | "activeFlowCount" | "queuedFlowCount" | "flowCapacity"> & { onOpen: BoardHeaderProps["onOpenSidePanel"] }) {
  const projectHealth = workflowHealth(workflow, health);
  const repositoryState = repositoryStatus(repository, isRepositoryLoading);
  return <div className="project-status-cluster" aria-label="Project status">
    <button className="project-status-button" type="button" onClick={() => onOpen("flow")}><Gauge size={14} /> Flow <strong>{activeFlowCount}/{flowCapacity}</strong><QueuedFlowCount count={queuedFlowCount} /></button>
    <button className={`project-status-button project-health-button ${projectHealth}`} type="button" onClick={() => onOpen("quality")}><Activity size={14} /> {projectHealth}</button>
    <button className="project-status-button repository-status" type="button" onClick={() => onOpen("repository")} title="Inspect repository activity"><GitBranch size={14} /><span>{repositoryBranch(repository, project.defaultBranch)}</span><strong className={repositoryState.className}>{repositoryState.label}</strong></button>
  </div>;
}

function QueuedFlowCount({ count }: { count: number }) { return count > 0 ? <span>+{count}</span> : null; }

function workflowHealth(workflow: ProjectWorkflowSnapshot | undefined, health: ProjectHealth | undefined) {
  if (workflow) return workflow.health.status;
  return health ? health.status : "unknown";
}

function repositoryBranch(repository: RepositoryDetails | undefined, defaultBranch: string): string {
  return repository && repository.summary.currentBranch ? repository.summary.currentBranch : defaultBranch;
}

function repositoryStatus(repository: RepositoryDetails | undefined, isLoading: boolean) {
  if (repository && !repository.summary.isClean) return { className: "repository-dirty", label: `${repository.summary.changedFileCount} changed` };
  if (repository) return { className: "repository-clean", label: "Clean" };
  return { className: "repository-pending", label: repositoryPendingLabel(isLoading) };
}

function repositoryPendingLabel(isLoading: boolean): string { return isLoading ? "Checking" : "Inspect"; }

function ProjectAlerts({ blockerCount, integrationCount, onOpen }: { blockerCount: number; integrationCount: number; onOpen: BoardHeaderProps["onOpenSidePanel"] }) {
  return <>{blockerCount > 0 && <button className="project-alert-button" type="button" onClick={() => onOpen("blockers")}><ShieldAlert size={15} /> Blockers <span>{blockerCount}</span></button>}{integrationCount > 0 && <button className="secondary-button integration-action" type="button" onClick={() => onOpen("integration")}><GitMerge size={16} /> Integrate <span>{integrationCount}</span></button>}</>;
}

function ProjectControlsHost({ project, queuedIntegrationCount, activeBlockerCount, proposedPlanCount, openCollaborationCount, acceptedDecisionCount, isOpen, hostRef, onToggle, onClose, onOpen }: Pick<BoardHeaderProps, "project" | "queuedIntegrationCount" | "activeBlockerCount" | "proposedPlanCount" | "openCollaborationCount" | "acceptedDecisionCount"> & { isOpen: boolean; hostRef: BoardHeaderProps["projectToolsRef"]; onToggle: () => void; onClose: () => void; onOpen: BoardHeaderProps["onOpenProjectToolPanel"] }) {
  return <div className="project-tools" ref={hostRef}><button className="secondary-button" type="button" aria-haspopup="menu" aria-expanded={isOpen} aria-controls="project-tools-menu" onClick={onToggle}><MoreHorizontal size={16} /> Project controls</button>{isOpen && <ProjectControlsMenu project={project} queuedIntegrationCount={queuedIntegrationCount} activeBlockerCount={activeBlockerCount} proposedPlanCount={proposedPlanCount} openCollaborationCount={openCollaborationCount} acceptedDecisionCount={acceptedDecisionCount} onClose={onClose} onOpen={onOpen} />}</div>;
}

function ProjectControlsMenu({ project, queuedIntegrationCount, activeBlockerCount, proposedPlanCount, openCollaborationCount, acceptedDecisionCount, onClose, onOpen }: Pick<BoardHeaderProps, "project" | "queuedIntegrationCount" | "activeBlockerCount" | "proposedPlanCount" | "openCollaborationCount" | "acceptedDecisionCount"> & { onClose: () => void; onOpen: BoardHeaderProps["onOpenProjectToolPanel"] }) {
  return <div className="project-tools-menu" id="project-tools-menu" role="menu">
    <Link role="menuitem" to={`/projects/${project.id}/progress`} onClick={onClose}><ChartNoAxesCombined size={15} /> Progress</Link>
    <Link role="menuitem" to={`/projects/${project.id}/metrics`} onClick={onClose}><Activity size={15} /> Metrics & cost</Link>
    <Link role="menuitem" to={`/projects/${project.id}/autonomy`} onClick={onClose}><PlayCircle size={15} /> Autonomy</Link>
    <button type="button" role="menuitem" onClick={() => onOpen("repository")}><GitBranch size={15} /> Repository</button>
    <button type="button" role="menuitem" onClick={() => onOpen("quality")}><CheckCircle2 size={15} /> Quality & health</button>
    <button type="button" role="menuitem" onClick={() => onOpen("flow")}><Gauge size={15} /> Flow & capacity</button>
    <button type="button" role="menuitem" onClick={() => onOpen("integration")}><GitMerge size={15} /> Integration queue <span>{queuedIntegrationCount}</span></button>
    <button type="button" role="menuitem" onClick={() => onOpen("blockers")}><ShieldAlert size={15} /> Blockers <span>{activeBlockerCount}</span></button>
    <button type="button" role="menuitem" onClick={() => onOpen("planning")}><Sparkles size={15} /> Plan <span>{proposedPlanCount}</span></button>
    <button type="button" role="menuitem" onClick={() => onOpen("collaboration")}><MessagesSquare size={15} /> Collaborate <span>{openCollaborationCount}</span></button>
    <button type="button" role="menuitem" onClick={() => onOpen("knowledge")}><BookOpenCheck size={15} /> Knowledge <span>{acceptedDecisionCount}</span></button>
  </div>;
}

type BoardCockpitProps = WorkflowTaskActions & {
  error?: string;
  boardView: WorkflowBoardView;
  workflow?: ProjectWorkflowSnapshot;
  workflowSnapshot?: ProjectWorkflowSnapshot;
  workflowSnapshotError?: string;
  isWorkflowSnapshotLoading: boolean;
  isAttentionExpanded: boolean;
  isAgentRailOpen: boolean;
  isAgentRailDrawer: boolean;
  showIdleAgents: boolean;
  showAllDone: boolean;
  activeMobileStage: WorkflowStage;
  activeTaskId?: string;
  tasks: Task[];
  tasksByStatus: Record<TaskStatus, Task[]>;
  tasksById: ReadonlyMap<string, Task>;
  taskViewsById: ReadonlyMap<string, WorkflowTaskView>;
  idleAgents: Agent[];
  recentlyTransitionedTaskIds: string[];
  now: number;
  onChangeView: (view: WorkflowBoardView) => void;
  onToggleAttention: () => void;
  onOpenAttention: ComponentProps<typeof AttentionTray>["onOpen"];
  onToggleAgentRail: () => void;
  onCloseAgentRail: () => void;
  onToggleIdle: () => void;
  onToggleDone: () => void;
  onChangeMobileStage: (stage: WorkflowStage) => void;
  onOpenActivity: ComponentProps<typeof AgentActivityRail>["onOpen"];
  onDragStart: (taskId: string) => void;
  onDragCancel: () => void;
  onDragEnd: (event: DragEndEvent) => void;
};

export function BoardCockpit(props: BoardCockpitProps) {
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }), useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }));
  return <div className="board-body">
    <BoardNotice error={props.error} />
    <CockpitToolbar {...props} />
    <BoardAttention {...props} />
    <DndContext sensors={sensors} collisionDetection={columnCollisionDetection} onDragStart={({ active }) => props.onDragStart(String(active.id))} onDragCancel={props.onDragCancel} onDragEnd={props.onDragEnd}>
      <BoardSurface {...props} />
      <TaskDragOverlay {...props} />
    </DndContext>
  </div>;
}

function BoardNotice({ error }: { error?: string }) { return <div className="board-notice">{error ? <div className="inline-error" role="alert">{error}</div> : null}</div>; }

function BoardAttention(props: BoardCockpitProps) {
  if (props.boardView !== "flow") return null;
  return <AttentionTray items={workflowAttention(props.workflow)} expanded={props.isAttentionExpanded} isLoading={workflowIsInitiallyLoading(props)} snapshotError={props.workflowSnapshotError} onToggle={props.onToggleAttention} onOpen={props.onOpenAttention} />;
}

function workflowAttention(workflow: ProjectWorkflowSnapshot | undefined) { return workflow ? workflow.attention : []; }

function workflowIsInitiallyLoading(props: Pick<BoardCockpitProps, "isWorkflowSnapshotLoading" | "workflowSnapshot">): boolean { return props.isWorkflowSnapshotLoading && !props.workflowSnapshot; }

function BoardSurface(props: BoardCockpitProps) { return props.boardView === "flow" ? <FlowWorkspace {...props} /> : <LifecycleBoard {...props} />; }

function TaskDragOverlay(props: BoardCockpitProps) {
  if (!props.activeTaskId) return <DragOverlay dropAnimation={null} />;
  return <DragOverlay dropAnimation={null}><WorkflowTaskDragPreview task={props.tasks.find((task) => task.id === props.activeTaskId)} view={props.taskViewsById.get(props.activeTaskId)} /></DragOverlay>;
}

function CockpitToolbar({ boardView, workflow, workflowSnapshot, workflowSnapshotError, isWorkflowSnapshotLoading, isAgentRailOpen, onChangeView, onToggleAgentRail }: BoardCockpitProps) {
  return <div className="cockpit-toolbar"><div className="cockpit-toolbar-primary"><BoardViewSwitch boardView={boardView} onChange={onChangeView} /><SnapshotState error={workflowSnapshotError} isLoading={isWorkflowSnapshotLoading} snapshot={workflowSnapshot} /></div><AgentRailToggle boardView={boardView} activityCount={workflowActivityCount(workflow)} isOpen={isAgentRailOpen} onToggle={onToggleAgentRail} /></div>;
}

export function BoardViewSwitch({ boardView, onChange }: { boardView: WorkflowBoardView; onChange: (view: WorkflowBoardView) => void }) {
  return <div className="board-view-switch" role="group" aria-label="Board view"><button className={viewButtonClass(boardView, "flow")} type="button" aria-pressed={boardView === "flow"} onClick={() => onChange("flow")}><LayoutGrid size={14} /> Flow</button><button className={viewButtonClass(boardView, "lifecycle")} type="button" aria-pressed={boardView === "lifecycle"} onClick={() => onChange("lifecycle")}><ListTree size={14} /> Full lifecycle</button></div>;
}

function viewButtonClass(current: WorkflowBoardView, target: WorkflowBoardView): string { return activeClass(current === target); }

function activeClass(active: boolean): string { return active ? "is-active" : ""; }

function SnapshotState({ error, isLoading, snapshot }: { error?: string; isLoading: boolean; snapshot?: ProjectWorkflowSnapshot }) {
  if (error) return <span className="workflow-snapshot-state is-stale">Workflow status stale</span>;
  if (isLoading && !snapshot) return <span className="workflow-snapshot-state is-loading">Refreshing workflow</span>;
  return null;
}

function AgentRailToggle({ boardView, activityCount, isOpen, onToggle }: { boardView: WorkflowBoardView; activityCount: number; isOpen: boolean; onToggle: () => void }) {
  if (boardView !== "flow") return null;
  return <button className="agent-rail-toggle" type="button" aria-expanded={isOpen} onClick={onToggle}><PanelRightOpen size={15} /> Agent activity <span>{activityCount}</span></button>;
}

function workflowActivityCount(workflow: ProjectWorkflowSnapshot | undefined): number { return workflow ? workflow.agentActivity.length : 0; }

function FlowWorkspace(props: BoardCockpitProps) {
  return <div className="cockpit-shell"><main className="flow-workspace"><MobileStageTabs stages={workflowStages(props.workflow)} activeStage={props.activeMobileStage} onChange={props.onChangeMobileStage} /><StageGrid {...props} /></main><WorkflowAgentRail {...props} /></div>;
}

function workflowStages(workflow: ProjectWorkflowSnapshot | undefined) { return workflow ? workflow.stages : []; }

function MobileStageTabs({ stages, activeStage, onChange }: { stages: ProjectWorkflowSnapshot["stages"]; activeStage: WorkflowStage; onChange: (stage: WorkflowStage) => void }) {
  return <nav className="mobile-stage-tabs" aria-label="Workflow stage">{stages.map((stage) => <button key={stage.id} className={activeClass(activeStage === stage.id)} type="button" aria-pressed={activeStage === stage.id} onClick={() => onChange(stage.id)}>{stage.label}<span>{stage.totalCount}</span></button>)}</nav>;
}

function StageGrid(props: BoardCockpitProps) {
  return <div className="flow-board">{workflowStages(props.workflow).map((stage) => <FlowStageColumn key={stage.id} stage={stage.id} label={stage.label} totalCount={stage.totalCount} taskViews={stage.tasks} tasksById={props.tasksById} activeOnMobile={props.activeMobileStage === stage.id} showAllDone={props.showAllDone} now={props.now} onToggleDone={props.onToggleDone} recentlyTransitionedTaskIds={props.recentlyTransitionedTaskIds} onInspect={props.onInspect} onEdit={props.onEdit} onDelete={props.onDelete} onPlanningState={props.onPlanningState} />)}</div>;
}

function WorkflowAgentRail(props: BoardCockpitProps) {
  const activity = props.workflow ? props.workflow.agentActivity : [];
  const idleCount = props.workflow ? props.workflow.idleAgentCount : props.idleAgents.length;
  return <AgentActivityRail activities={activity} idleAgents={props.idleAgents} idleCount={idleCount} isOpen={props.isAgentRailOpen} isDrawer={props.isAgentRailDrawer} showIdle={props.showIdleAgents} now={props.now} onClose={props.onCloseAgentRail} onToggleIdle={props.onToggleIdle} onOpen={props.onOpenActivity} />;
}

function LifecycleBoard(props: BoardCockpitProps) {
  return <div className="kanban-board lifecycle-board">{TASK_STATUSES.map((status) => <TaskColumn key={status} status={status} tasks={props.tasksByStatus[status]} taskViewsById={props.taskViewsById} now={props.now} recentlyTransitionedTaskIds={props.recentlyTransitionedTaskIds} onInspect={props.onInspect} onEdit={props.onEdit} onDelete={props.onDelete} onPlanningState={props.onPlanningState} />)}</div>;
}

const columns: Record<TaskStatus, { label: string; tone: string }> = {
  backlog: { label: "Backlog", tone: "neutral" }, ready: { label: "Ready", tone: "blue" }, in_progress: { label: "In Progress", tone: "amber" }, needs_input: { label: "Needs Input", tone: "yellow" }, review: { label: "Review", tone: "violet" }, approved: { label: "Approved", tone: "indigo" }, integrating: { label: "Integrating", tone: "cyan" }, blocked: { label: "Blocked", tone: "orange" }, done: { label: "Done", tone: "green" },
};

type TaskColumnProps = { status: TaskStatus; tasks: Task[]; taskViewsById: ReadonlyMap<string, WorkflowTaskView>; now: number; recentlyTransitionedTaskIds: string[] } & WorkflowTaskActions;

function TaskColumn({ status, tasks, taskViewsById, now, recentlyTransitionedTaskIds, ...actions }: TaskColumnProps) {
  const planningStatus = isPlanningStatus(status);
  const { setNodeRef, isOver } = useDroppable({ id: `column:${status}`, disabled: !planningStatus });
  return <section ref={setNodeRef} className={`kanban-column ${isOver ? "is-over" : ""}`}><header className="column-header"><div><span className={`status-dot ${columns[status].tone}`} /><h2>{columns[status].label}</h2></div><span>{tasks.length}</span></header><TaskColumnContents tasks={tasks} taskViewsById={taskViewsById} now={now} recentlyTransitionedTaskIds={recentlyTransitionedTaskIds} planningStatus={planningStatus} {...actions} /></section>;
}

function isPlanningStatus(status: TaskStatus): boolean { return status === "backlog" || status === "ready"; }

function TaskColumnContents({ tasks, taskViewsById, now, recentlyTransitionedTaskIds, planningStatus, ...actions }: Omit<TaskColumnProps, "status"> & { planningStatus: boolean }) {
  return <div className="task-list"><SortableContext items={tasks.map((task) => task.id)} strategy={verticalListSortingStrategy}>{tasks.map((task) => <WorkflowTaskCard key={task.id} task={task} view={taskViewsById.get(task.id)} now={now} isRecentlyTransitioned={recentlyTransitionedTaskIds.includes(task.id)} {...actions} />)}</SortableContext><EmptyTaskColumn isEmpty={tasks.length === 0} planningStatus={planningStatus} /></div>;
}

function EmptyTaskColumn({ isEmpty, planningStatus }: { isEmpty: boolean; planningStatus: boolean }) { return isEmpty ? <p className="empty-column">{planningStatus ? "No tasks" : "Workflow managed"}</p> : null; }

function columnCollisionDetection(args: Parameters<typeof pointerWithin>[0]) {
  const collisions = pointerWithin(args);
  return collisions.length > 0 ? collisions : closestCorners(args);
}

type BoardSidePanelsProps = {
  activePanel?: BoardSidePanel;
  task?: ComponentProps<typeof TaskDetailPanel>;
  repository: ComponentProps<typeof RepositoryInspector>;
  integration: ComponentProps<typeof IntegrationQueuePanel>;
  quality: ComponentProps<typeof QualityGatesPanel>;
  flow: ComponentProps<typeof FlowControlPanel>;
  blockers: ComponentProps<typeof ProjectBlockersPanel>;
  knowledge: ComponentProps<typeof ProjectKnowledgePanel>;
  planning: ComponentProps<typeof PlanningPanel>;
  collaboration: ComponentProps<typeof CollaborationPanel>;
};

export function BoardSidePanels(props: BoardSidePanelsProps) {
  if (!props.activePanel) return null;
  return sidePanelRenderers[props.activePanel](props);
}

const sidePanelRenderers: Record<BoardSidePanel, (props: BoardSidePanelsProps) => ReactNode> = {
  task: (props) => props.task ? <TaskDetailPanel key={props.task.task.id} {...props.task} /> : null,
  repository: (props) => <RepositoryInspector {...props.repository} />,
  integration: (props) => <IntegrationQueuePanel {...props.integration} />,
  quality: (props) => <QualityGatesPanel {...props.quality} />,
  flow: (props) => <FlowControlPanel {...props.flow} />,
  blockers: (props) => <ProjectBlockersPanel {...props.blockers} />,
  knowledge: (props) => <ProjectKnowledgePanel {...props.knowledge} />,
  planning: (props) => <PlanningPanel {...props.planning} />,
  collaboration: (props) => <CollaborationPanel {...props.collaboration} />,
};
