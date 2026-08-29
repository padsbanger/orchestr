import { closestCorners, DndContext, DragEndEvent, DragOverlay, KeyboardSensor, pointerWithin, PointerSensor, useSensor, useSensors, useDroppable } from "@dnd-kit/core";
import { arrayMove, SortableContext, sortableKeyboardCoordinates, useSortable, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { Activity, ArrowLeft, BookOpenCheck, Bot, ChartNoAxesCombined, Gauge, GitBranch, GitMerge, GripVertical, MessagesSquare, MoreHorizontal, Pencil, PlayCircle, Plus, SearchCode, ShieldAlert, Sparkles, Trash2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { RepositoryInspector } from "../../components/RepositoryInspector/RepositoryInspector";
import { IntegrationQueuePanel } from "../../components/IntegrationQueuePanel/IntegrationQueuePanel";
import { QualityGatesPanel } from "../../components/QualityGatesPanel/QualityGatesPanel";
import { FlowControlPanel } from "../../components/FlowControlPanel/FlowControlPanel";
import { ProjectBlockersPanel } from "../../components/ProjectBlockersPanel/ProjectBlockersPanel";
import { ProjectKnowledgePanel } from "../../components/ProjectKnowledgePanel/ProjectKnowledgePanel";
import { PlanningPanel } from "../../components/PlanningPanel/PlanningPanel";
import { CollaborationPanel } from "../../components/CollaborationPanel/CollaborationPanel";
import { TaskDetailPanel } from "../../components/TaskDetailPanel/TaskDetailPanel";
import { TaskDialog } from "../../components/TaskDialog/TaskDialog";
import { listAgents, type Agent } from "../../services/agents";
import { errorMessage, runConfirmedDestructiveAction } from "../../services/confirmations";
import { getProject, getRepositoryDetails, type Project, type RepositoryDetails } from "../../services/projects";
import { cancelQueuedTaskRun, cancelTaskRun, listTaskRuns, recoverTaskRun, resolveFailedRun, startTaskRun, type TaskRun } from "../../services/runs";
import { approveTaskReview, getTaskReview, requestTaskChanges, type TaskReview } from "../../services/reviews";
import { listAgentReviews, listenToAgentReviewEvents, startAgentReview, type AgentReview } from "../../services/agentReviews";
import { integrateNextTask, listIntegrationAttempts, listRevertAttempts, retryIntegrationAttempt, retryIntegrationCleanup, revertIntegration, type IntegrationAttempt, type RevertAttempt } from "../../services/integrations";
import { createValidationCommand, deleteValidationCommand, getProjectHealth, listValidationAttempts, listValidationCommands, listenToValidationEvents, rerunIntegrationValidation, type ProjectHealth, type ValidationAttempt, type ValidationCommand, type ValidationStage } from "../../services/quality";
import { listEpics, listMilestones, type Epic, type Milestone } from "../../services/outcomes";
import { getFlowState, listenToFlowChanges, scheduleReadyTasks, updateFlowLimits, type FlowLimitInput, type FlowState } from "../../services/flow";
import { answerTaskInput, createProjectBlocker, listProjectBlockers, listTaskInputRequests, requestTaskInput, resolveProjectBlocker, type ProjectBlocker, type TaskInputRequest } from "../../services/interruptions";
import { createArchitectureDecision, decideArchitectureDecision, listArchitectureDecisions, listRelevantArchitectureDecisions, type ArchitectureDecision, type ArchitectureDecisionInput } from "../../services/knowledge";
import { approvePlanningProposal, cancelPlanningProposal, listPlanningProposals, listenToPlanningEvents, rejectPlanningProposal, startPlanningProposal, type PlanningProposal } from "../../services/planning";
import { createCollaborationEntry, listCollaborationEntries, resolveCollaborationEntry, type CollaborationEntry, type CollaborationKind } from "../../services/collaboration";
import { cleanupTaskWorktree, createTask, deleteTask, listTasks, moveTask, openTaskWorktree, TASK_STATUSES, type Task, type TaskInput, type TaskStatus, updateTask } from "../../services/tasks";
import { listenToWorkerRunEvents } from "../../services/workers";
import "./BoardPage.css";

const columns: Record<TaskStatus, { label: string; tone: string }> = {
  backlog: { label: "Backlog", tone: "neutral" },
  ready: { label: "Ready", tone: "blue" },
  in_progress: { label: "In Progress", tone: "amber" },
  needs_input: { label: "Needs Input", tone: "yellow" },
  review: { label: "Review", tone: "violet" },
  approved: { label: "Approved", tone: "indigo" },
  integrating: { label: "Integrating", tone: "cyan" },
  blocked: { label: "Blocked", tone: "orange" },
  done: { label: "Done", tone: "green" },
};

type BoardSidePanel = "task" | "repository" | "integration" | "quality" | "flow" | "blockers" | "knowledge" | "planning" | "collaboration";

export function BoardPage() {
  const { projectId } = useParams();
  const [project, setProject] = useState<Project | null>();
  const [tasks, setTasks] = useState<Task[]>([]);
  const [agents, setAgents] = useState<Agent[]>([]);
  const [error, setError] = useState<string>();
  const [isLoading, setIsLoading] = useState(true);
  const [editingTask, setEditingTask] = useState<Task | null>();
  const [inspectedTask, setInspectedTask] = useState<Task | null>();
  const [isCreating, setIsCreating] = useState(false);
  const [activeTaskId, setActiveTaskId] = useState<string>();
  const [recentlyTransitionedTaskIds, setRecentlyTransitionedTaskIds] = useState<string[]>([]);
  const [runs, setRuns] = useState<TaskRun[]>([]);
  const [isStartingRun, setIsStartingRun] = useState(false);
  const [cancellingRunId, setCancellingRunId] = useState<string>();
  const [isCleaningWorktree, setIsCleaningWorktree] = useState(false);
  const [isOpeningWorktree, setIsOpeningWorktree] = useState(false);
  const [review, setReview] = useState<TaskReview>();
  const [agentReviews, setAgentReviews] = useState<AgentReview[]>([]);
  const [isAgentReviewStarting, setIsAgentReviewStarting] = useState(false);
  const [reviewError, setReviewError] = useState<string>();
  const [isReviewLoading, setIsReviewLoading] = useState(false);
  const [isReviewActionPending, setIsReviewActionPending] = useState(false);
  const [integrationAttempts, setIntegrationAttempts] = useState<IntegrationAttempt[]>([]);
  const [revertAttempts, setRevertAttempts] = useState<RevertAttempt[]>([]);
  const [isIntegrationQueueLoading, setIsIntegrationQueueLoading] = useState(false);
  const [isIntegrating, setIsIntegrating] = useState(false);
  const [recoveringIntegrationId, setRecoveringIntegrationId] = useState<string>();
  const [revertingIntegrationId, setRevertingIntegrationId] = useState<string>();
  const [runRecoveryAction, setRunRecoveryAction] = useState<string>();
  const [inputRequests, setInputRequests] = useState<TaskInputRequest[]>([]);
  const [inputAction, setInputAction] = useState<"request" | "answer">();
  const [projectBlockers, setProjectBlockers] = useState<ProjectBlocker[]>([]);
  const [isBlockersLoading, setIsBlockersLoading] = useState(false);
  const [isBlockerSaving, setIsBlockerSaving] = useState(false);
  const [resolvingBlockerId, setResolvingBlockerId] = useState<string>();
  const [architectureDecisions, setArchitectureDecisions] = useState<ArchitectureDecision[]>([]);
  const [taskArchitectureDecisions, setTaskArchitectureDecisions] = useState<ArchitectureDecision[]>([]);
  const [knowledgePreviewTaskId, setKnowledgePreviewTaskId] = useState<string>();
  const [knowledgePreviewDecisions, setKnowledgePreviewDecisions] = useState<ArchitectureDecision[]>([]);
  const [isKnowledgeLoading, setIsKnowledgeLoading] = useState(false);
  const [isKnowledgePreviewLoading, setIsKnowledgePreviewLoading] = useState(false);
  const [isKnowledgeSaving, setIsKnowledgeSaving] = useState(false);
  const [decidingArchitectureId, setDecidingArchitectureId] = useState<string>();
  const [planningProposals, setPlanningProposals] = useState<PlanningProposal[]>([]);
  const [isPlanningLoading, setIsPlanningLoading] = useState(false);
  const [isPlanningStarting, setIsPlanningStarting] = useState(false);
  const [planningActionId, setPlanningActionId] = useState<string>();
  const [collaborationEntries, setCollaborationEntries] = useState<CollaborationEntry[]>([]);
  const [isCollaborationLoading, setIsCollaborationLoading] = useState(false);
  const [isCollaborationSaving, setIsCollaborationSaving] = useState(false);
  const [collaborationActionId, setCollaborationActionId] = useState<string>();
  const [health, setHealth] = useState<ProjectHealth>();
  const [implementationCommands, setImplementationCommands] = useState<ValidationCommand[]>([]);
  const [integrationCommands, setIntegrationCommands] = useState<ValidationCommand[]>([]);
  const [validationAttempts, setValidationAttempts] = useState<ValidationAttempt[]>([]);
  const [isQualityLoading, setIsQualityLoading] = useState(false);
  const [flow, setFlow] = useState<FlowState>();
  const [isFlowLoading, setIsFlowLoading] = useState(false);
  const [isFlowSaving, setIsFlowSaving] = useState(false);
  const [isScheduling, setIsScheduling] = useState(false);
  const [isRerunningIntegrationValidation, setIsRerunningIntegrationValidation] = useState(false);
  const [milestones, setMilestones] = useState<Milestone[]>([]);
  const [epics, setEpics] = useState<Epic[]>([]);
  const runIds = useRef(new Set<string>());
  const taskTransitionTimers = useRef(new Map<string, number>());
  const previousTaskStatuses = useRef(new Map<string, TaskStatus>());
  const [repository, setRepository] = useState<RepositoryDetails>();
  const [repositoryError, setRepositoryError] = useState<string>();
  const [isRepositoryLoading, setIsRepositoryLoading] = useState(false);
  const [activeSidePanel, setActiveSidePanel] = useState<BoardSidePanel>();
  const [isProjectToolsOpen, setIsProjectToolsOpen] = useState(false);
  const projectToolsRef = useRef<HTMLDivElement>(null);
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  const flashTaskTransition = useCallback((taskId: string) => {
    const existingTimer = taskTransitionTimers.current.get(taskId);
    if (existingTimer) window.clearTimeout(existingTimer);
    setRecentlyTransitionedTaskIds((current) => current.includes(taskId) ? current : [...current, taskId]);
    taskTransitionTimers.current.set(taskId, window.setTimeout(() => {
      setRecentlyTransitionedTaskIds((current) => current.filter((id) => id !== taskId));
      taskTransitionTimers.current.delete(taskId);
    }, 700));
  }, []);

  useEffect(() => () => {
    taskTransitionTimers.current.forEach((timer) => window.clearTimeout(timer));
  }, []);

  useEffect(() => {
    previousTaskStatuses.current.clear();
    setRecentlyTransitionedTaskIds([]);
  }, [projectId]);

  useEffect(() => {
    if (!isProjectToolsOpen) return;
    const closeWhenOutside = (event: PointerEvent) => {
      if (event.target instanceof Node && !projectToolsRef.current?.contains(event.target)) setIsProjectToolsOpen(false);
    };
    const closeWhenEscaped = (event: KeyboardEvent) => {
      if (event.key === "Escape") setIsProjectToolsOpen(false);
    };
    document.addEventListener("pointerdown", closeWhenOutside);
    document.addEventListener("keydown", closeWhenEscaped);
    return () => {
      document.removeEventListener("pointerdown", closeWhenOutside);
      document.removeEventListener("keydown", closeWhenEscaped);
    };
  }, [isProjectToolsOpen]);

  const openSidePanel = (panel: Exclude<BoardSidePanel, "task">) => {
    setInspectedTask(null);
    setActiveSidePanel(panel);
  };

  const openProjectToolPanel = (panel: Exclude<BoardSidePanel, "task">) => {
    setIsProjectToolsOpen(false);
    openSidePanel(panel);
  };

  const inspectTask = (task: Task) => {
    setInspectedTask(task);
    setActiveSidePanel("task");
  };

  const closeSidePanel = () => {
    setActiveSidePanel(undefined);
    setInspectedTask(null);
  };

  const loadRepository = useCallback(async () => {
    if (!projectId) return;
    setIsRepositoryLoading(true);
    setRepositoryError(undefined);
    try {
      setRepository(await getRepositoryDetails(projectId));
    } catch (loadError) {
      setRepositoryError(loadError instanceof Error ? loadError.message : "Unable to inspect the repository.");
    } finally {
      setIsRepositoryLoading(false);
    }
  }, [projectId]);

  const loadIntegrationQueue = useCallback(async () => {
    if (!projectId) return;
    setIsIntegrationQueueLoading(true);
    try {
      const [attempts, reverts] = await Promise.all([listIntegrationAttempts(projectId), listRevertAttempts(projectId)]);
      setIntegrationAttempts(attempts);
      setRevertAttempts(reverts);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Unable to load the integration queue.");
    } finally {
      setIsIntegrationQueueLoading(false);
    }
  }, [projectId]);

  const loadQualityGates = useCallback(async () => {
    if (!projectId) return;
    setIsQualityLoading(true);
    try {
      const [loadedHealth, loadedImplementation, loadedIntegration, loadedAttempts] = await Promise.all([
        getProjectHealth(projectId),
        listValidationCommands(projectId, "implementation"),
        listValidationCommands(projectId, "integration"),
        listValidationAttempts(projectId),
      ]);
      setHealth(loadedHealth);
      setImplementationCommands(loadedImplementation);
      setIntegrationCommands(loadedIntegration);
      setValidationAttempts(loadedAttempts);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Unable to load project quality gates.");
    } finally {
      setIsQualityLoading(false);
    }
  }, [projectId]);

  const loadOutcomes = useCallback(async () => {
    if (!projectId) return;
    try {
      const [loadedMilestones, loadedEpics] = await Promise.all([listMilestones(projectId), listEpics(projectId)]);
      setMilestones(loadedMilestones); setEpics(loadedEpics);
    } catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Unable to load project outcomes."); }
  }, [projectId]);

  const loadFlowControl = useCallback(async () => {
    if (!projectId) return;
    setIsFlowLoading(true);
    try {
      setFlow(await getFlowState(projectId));
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Unable to load execution flow control.");
    } finally {
      setIsFlowLoading(false);
    }
  }, [projectId]);

  const loadProjectBlockers = useCallback(async () => {
    if (!projectId) return;
    setIsBlockersLoading(true);
    try {
      setProjectBlockers(await listProjectBlockers(projectId));
    } catch (loadError) {
      setError(errorMessage(loadError, "Unable to load project blockers."));
    } finally {
      setIsBlockersLoading(false);
    }
  }, [projectId]);

  const loadArchitectureDecisions = useCallback(async () => {
    if (!projectId) return;
    setIsKnowledgeLoading(true);
    try {
      setArchitectureDecisions(await listArchitectureDecisions(projectId));
    } catch (loadError) {
      setError(errorMessage(loadError, "Unable to load architecture decisions."));
    } finally {
      setIsKnowledgeLoading(false);
    }
  }, [projectId]);

  const loadPlanningProposals = useCallback(async () => {
    if (!projectId) return;
    setIsPlanningLoading(true);
    try {
      const loaded = await listPlanningProposals(projectId);
      setPlanningProposals(loaded);
      setPlanningActionId((current) => current && loaded.some((proposal) => proposal.id === current && proposal.status === "generating") ? current : undefined);
    } catch (loadError) {
      setError(errorMessage(loadError, "Unable to load planning proposals."));
    } finally {
      setIsPlanningLoading(false);
    }
  }, [projectId]);

  const loadCollaboration = useCallback(async () => {
    if (!projectId) return;
    setIsCollaborationLoading(true);
    try { setCollaborationEntries(await listCollaborationEntries(projectId)); }
    catch (loadError) { setError(errorMessage(loadError, "Unable to load collaboration activity.")); }
    finally { setIsCollaborationLoading(false); }
  }, [projectId]);

  const loadBoard = useCallback(async (showLoading = false) => {
    if (!projectId) return;
    if (showLoading) setIsLoading(true);
    setError(undefined);
    try {
      const [loadedProject, loadedTasks, loadedAgents] = await Promise.all([getProject(projectId), listTasks(projectId), listAgents()]);
      setProject(loadedProject);
      setTasks(loadedTasks);
      setAgents(loadedAgents);
      void loadRepository();
      void loadIntegrationQueue();
      void loadQualityGates();
      void loadOutcomes();
      void loadFlowControl();
      void loadProjectBlockers();
      void loadArchitectureDecisions();
      void loadPlanningProposals();
      void loadCollaboration();
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Unable to load the project board.");
    } finally {
      if (showLoading) setIsLoading(false);
    }
  }, [loadArchitectureDecisions, loadCollaboration, loadFlowControl, loadIntegrationQueue, loadOutcomes, loadPlanningProposals, loadProjectBlockers, loadQualityGates, loadRepository, projectId]);

  useEffect(() => {
    void loadBoard(true);
  }, [loadBoard]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listenToPlanningEvents(() => { void loadPlanningProposals(); }).then((stopListening) => {
      if (disposed) stopListening(); else unlisten = stopListening;
    });
    return () => { disposed = true; unlisten?.(); };
  }, [loadPlanningProposals]);

  useEffect(() => {
    if (!inspectedTask) {
      setRuns([]);
      return;
    }
    void listTaskRuns(inspectedTask.id).then(setRuns).catch((loadError: unknown) => {
      setError(loadError instanceof Error ? loadError.message : "Unable to load task runs.");
    });
  }, [inspectedTask?.id, inspectedTask?.status]);

  useEffect(() => {
    if (!inspectedTask) { setInputRequests([]); return; }
    void listTaskInputRequests(inspectedTask.id).then(setInputRequests).catch((loadError: unknown) => {
      setError(errorMessage(loadError, "Unable to load task input requests."));
    });
  }, [inspectedTask?.id, inspectedTask?.status]);

  useEffect(() => {
    if (!inspectedTask) { setTaskArchitectureDecisions([]); return; }
    void listRelevantArchitectureDecisions(inspectedTask.id).then(setTaskArchitectureDecisions).catch((loadError: unknown) => {
      setError(errorMessage(loadError, "Unable to load task architecture context."));
    });
  }, [inspectedTask?.id]);

  useEffect(() => {
    if (!knowledgePreviewTaskId) { setKnowledgePreviewDecisions([]); return; }
    setIsKnowledgePreviewLoading(true);
    void listRelevantArchitectureDecisions(knowledgePreviewTaskId).then(setKnowledgePreviewDecisions).catch((loadError: unknown) => {
      setError(errorMessage(loadError, "Unable to preview task knowledge."));
    }).finally(() => setIsKnowledgePreviewLoading(false));
  }, [architectureDecisions, knowledgePreviewTaskId]);

  useEffect(() => {
    if (!inspectedTask || inspectedTask.status !== "review") { setReview(undefined); setReviewError(undefined); return; }
    setIsReviewLoading(true); setReviewError(undefined);
    void getTaskReview(inspectedTask.id).then(setReview).catch((error: unknown) => setReviewError(error instanceof Error ? error.message : "Unable to load task branch changes.")).finally(() => setIsReviewLoading(false));
  }, [inspectedTask?.id, inspectedTask?.status]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listenToAgentReviewEvents((reviewId) => {
      if (!inspectedTask) return;
      void listAgentReviews(inspectedTask.id).then((reviews) => {
        if (disposed) return;
        setAgentReviews(reviews);
        const changedReview = reviews.find((review) => review.id === reviewId);
        if (changedReview && changedReview.status !== "running") {
          void Promise.all([loadBoard(false), loadIntegrationQueue()]);
        }
      }).catch((loadError: unknown) => {
        if (!disposed) setError(loadError instanceof Error ? loadError.message : "Unable to refresh the architect review.");
      });
    }).then((stopListening) => { if (disposed) stopListening(); else unlisten = stopListening; });
    return () => { disposed = true; unlisten?.(); };
  }, [inspectedTask?.id, loadBoard, loadIntegrationQueue]);

  useEffect(() => {
    if (!inspectedTask) { setAgentReviews([]); return; }
    let disposed = false;
    void listAgentReviews(inspectedTask.id).then((reviews) => {
      if (disposed) return;
      setAgentReviews(reviews);
    }).catch((loadError: unknown) => {
      if (!disposed) setError(loadError instanceof Error ? loadError.message : "Unable to load architect reviews.");
    });
    return () => { disposed = true; };
  }, [inspectedTask?.id]);

  useEffect(() => {
    if (!inspectedTask || !agentReviews.some((review) => review.status === "running")) return;
    const interval = window.setInterval(() => {
      void listAgentReviews(inspectedTask.id).then(setAgentReviews).catch(() => undefined);
    }, 1_500);
    return () => window.clearInterval(interval);
  }, [agentReviews, inspectedTask?.id]);

  useEffect(() => {
    runIds.current = new Set(runs.map((run) => run.id));
  }, [runs]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listenToFlowChanges(() => { void Promise.all([loadBoard(), loadFlowControl()]); }).then((stopListening) => {
      if (disposed) stopListening(); else unlisten = stopListening;
    });
    return () => { disposed = true; unlisten?.(); };
  }, [loadBoard, loadFlowControl]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listenToWorkerRunEvents((event) => {
      if (!runIds.current.has(event.runId)) {
        if (event.kind !== "output") void Promise.all([loadBoard(), loadFlowControl()]);
        return;
      }
      if (event.stream && event.text !== null) {
        const output = { stream: event.stream, text: event.rawText ?? event.text, createdAt: new Date().toISOString() };
        const timelineEvent = { id: -Date.now(), kind: event.kind === "output" ? "command.output" : event.kind, message: event.text, command: event.command ?? null, filePath: null, exitCode: null, createdAt: output.createdAt };
        setRuns((current) => current.map((run) => run.id === event.runId ? {
          ...run,
          output: [...run.output, output],
          events: [...run.events, timelineEvent],
        } : run));
        return;
      }
      if (event.kind !== "output" && inspectedTask) {
        void Promise.all([listTaskRuns(inspectedTask.id), listTaskInputRequests(inspectedTask.id), loadBoard(), loadFlowControl()]).then(([loadedRuns, loadedInputRequests]) => {
          setRuns(loadedRuns);
          setInputRequests(loadedInputRequests);
        });
      }
    }).then((stopListening) => {
      if (disposed) stopListening();
      else unlisten = stopListening;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [inspectedTask?.id, loadBoard, loadFlowControl]);

  useEffect(() => {
    if (!inspectedTask) return;
    const current = tasks.find((task) => task.id === inspectedTask.id);
    if (current && current !== inspectedTask) setInspectedTask(current);
  }, [inspectedTask, tasks]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listenToValidationEvents((event) => {
      if (event.kind === "command.started" && projectId) {
        void listValidationAttempts(projectId).then(setValidationAttempts).catch(() => undefined);
      }
      setValidationAttempts((current) => current.map((attempt) => attempt.id !== event.validationAttemptId ? attempt : {
        ...attempt,
        events: [...attempt.events, { id: -Date.now(), commandId: event.commandId, kind: event.kind, message: event.text, stream: event.stream, exitCode: event.exitCode, createdAt: new Date().toISOString() }],
      }));
      if (event.kind.startsWith("validation.")) void loadQualityGates();
    }).then((stopListening) => { if (disposed) stopListening(); else unlisten = stopListening; });
    return () => { disposed = true; unlisten?.(); };
  }, [loadQualityGates, projectId]);

  const tasksByStatus = useMemo(() => Object.fromEntries(
    TASK_STATUSES.map((status) => [status, tasks.filter((task) => task.status === status).sort((a, b) => a.position - b.position)]),
  ) as Record<TaskStatus, Task[]>, [tasks]);
  const agentNamesById = useMemo(() => new Map(agents.map((agent) => [agent.id, agent.name])), [agents]);

  useEffect(() => {
    const previousStatuses = previousTaskStatuses.current;
    if (previousStatuses.size > 0) {
      tasks.forEach((task) => {
        if (previousStatuses.get(task.id) && previousStatuses.get(task.id) !== task.status) flashTaskTransition(task.id);
      });
    }
    previousTaskStatuses.current = new Map(tasks.map((task) => [task.id, task.status]));
  }, [flashTaskTransition, tasks]);

  const handleDragEnd = async ({ active, over }: DragEndEvent) => {
    if (!over) return;
    const source = tasks.find((task) => task.id === active.id);
    if (!source) return;
    const destinationStatus = statusForDropTarget(String(over.id), tasks);
    if (!destinationStatus) return;
    if (isSystemManagedStatus(source.status) || isSystemManagedStatus(destinationStatus)) {
      setError("Needs Input, Approved, Integrating, Blocked, and Done are workflow-managed states.");
      return;
    }
    const destinationTasks = tasksByStatus[destinationStatus];
    const overIndex = destinationTasks.findIndex((task) => task.id === over.id);
    const destinationPosition = overIndex === -1 ? destinationTasks.length : overIndex;

    const beforeMove = tasks;
    setTasks(moveTaskLocally(tasks, source.id, destinationStatus, destinationPosition));
    try {
      await moveTask(source.id, destinationStatus, destinationPosition);
      await loadBoard();
    } catch (moveError) {
      setTasks(beforeMove);
      setError(moveError instanceof Error ? moveError.message : "Unable to move task.");
    }
  };

  const saveTask = async (input: TaskInput) => {
    if (!projectId) return;
    if (editingTask) await updateTask(editingTask.id, input);
    else await createTask(projectId, input);
    await loadBoard();
  };

  const removeTask = async (task: Task) => {
    setError(undefined);
    try {
      await runConfirmedDestructiveAction({
        title: "Delete task",
        message: `Delete “${task.title}”? This cannot be undone.`,
        confirmLabel: "Delete task",
      }, async () => {
        await deleteTask(task.id);
        await loadBoard();
      });
    } catch (deleteError) {
      setError(errorMessage(deleteError, "Unable to delete task."));
    }
  };

  const startRun = async () => {
    if (!inspectedTask) return;
    setError(undefined);
    setIsStartingRun(true);
    try {
      const started = await startTaskRun(inspectedTask.id);
      setRuns((current) => [started.run, ...current]);
      setInspectedTask(started.task);
      await Promise.all([loadBoard(), loadFlowControl()]);
    } catch (runError) {
      setError(errorMessage(runError, "Unable to start the Codex task."));
    } finally {
      setIsStartingRun(false);
    }
  };

  const scheduleProject = async () => {
    if (!projectId) return;
    setError(undefined);
    setIsScheduling(true);
    try {
      const result = await scheduleReadyTasks(projectId);
      if (result.scheduled.length === 0) setError(result.blockedReason ?? "No Ready task could be scheduled.");
      await Promise.all([loadBoard(), loadFlowControl()]);
    } catch (scheduleError) {
      setError(errorMessage(scheduleError, "Unable to schedule Ready work."));
    } finally {
      setIsScheduling(false);
    }
  };

  const cancelRun = async (runId: string) => {
    setError(undefined);
    setCancellingRunId(runId);
    try {
      const queued = flow?.queue.some((run) => run.id === runId) || runs.some((run) => run.id === runId && run.status === "queued");
      await (queued ? cancelQueuedTaskRun(runId) : cancelTaskRun(runId));
      await loadFlowControl();
      if (inspectedTask) setRuns(await listTaskRuns(inspectedTask.id));
    } catch (runError) {
      setError(runError instanceof Error ? runError.message : "Unable to cancel the Codex task.");
    } finally {
      setCancellingRunId(undefined);
    }
  };

  const recoverRun = async (runId: string, mode: "resume" | "restart_clean", agentId?: string) => {
    const execute = async () => {
      setRunRecoveryAction(agentId ? `reassign:${agentId}` : mode);
      const started = await recoverTaskRun(runId, mode, agentId);
      setRuns((current) => [started.run, ...current]);
      setTasks((current) => current.map((task) => task.id === started.task.id ? started.task : task));
      setInspectedTask(started.task);
      await Promise.all([loadBoard(), loadFlowControl()]);
    };
    setError(undefined);
    try {
      if (mode === "restart_clean") {
        await runConfirmedDestructiveAction({
          title: "Restart task clean",
          message: "Remove the failed run's managed worktree and task branch, then restart from the integration branch? The run history will be preserved.",
          confirmLabel: "Restart clean",
        }, execute);
      } else {
        await execute();
      }
    } catch (recoveryError) {
      setError(errorMessage(recoveryError, "Unable to recover the failed run."));
    } finally {
      setRunRecoveryAction(undefined);
    }
  };

  const resolveRunFailure = async (runId: string, action: "abandon" | "escalate") => {
    setError(undefined);
    setRunRecoveryAction(action);
    try {
      const execute = async () => {
        const task = await resolveFailedRun(runId, action, action === "escalate" ? "A failed agent run requires human recovery." : undefined);
        setTasks((current) => current.map((currentTask) => currentTask.id === task.id ? task : currentTask));
        setInspectedTask(task);
        await Promise.all([loadBoard(), loadFlowControl()]);
      };
      if (action === "abandon") {
        await runConfirmedDestructiveAction({
          title: "Abandon run recovery",
          message: "Move this task back to Backlog? Its branch, worktree, and run history will be preserved for inspection.",
          confirmLabel: "Abandon recovery",
        }, execute);
      } else {
        await execute();
      }
    } catch (recoveryError) {
      setError(errorMessage(recoveryError, "Unable to resolve the failed run."));
    } finally {
      setRunRecoveryAction(undefined);
    }
  };

  const requestHumanInput = async (question: string, runId?: string) => {
    if (!inspectedTask) return;
    setInputAction("request"); setError(undefined);
    try {
      const request = await requestTaskInput(inspectedTask.id, question, runId);
      setInputRequests((current) => [request, ...current]);
      await Promise.all([loadBoard(false), loadFlowControl()]);
    } catch (inputError) {
      setError(errorMessage(inputError, "Unable to request human input."));
    } finally { setInputAction(undefined); }
  };

  const answerHumanInput = async (requestId: string, answer: string) => {
    setInputAction("answer"); setError(undefined);
    try {
      const result = await answerTaskInput(requestId, answer);
      setInputRequests((current) => current.map((request) => request.id === result.request.id ? result.request : request));
      setTasks((current) => current.map((task) => task.id === result.task.id ? result.task : task));
      setInspectedTask(result.task);
      if (inspectedTask) setRuns(await listTaskRuns(inspectedTask.id));
      await Promise.all([loadBoard(false), loadFlowControl()]);
    } catch (inputError) {
      setError(errorMessage(inputError, "Unable to answer the input request."));
    } finally { setInputAction(undefined); }
  };

  const addProjectBlocker = async (input: { title: string; description?: string; affectsAllTasks: boolean; affectedTaskIds: string[] }) => {
    if (!projectId) return;
    setIsBlockerSaving(true); setError(undefined);
    try {
      await createProjectBlocker({ projectId, ...input });
      await Promise.all([loadProjectBlockers(), loadBoard(false), loadFlowControl()]);
    } catch (blockerError) {
      setError(errorMessage(blockerError, "Unable to create the project blocker."));
    } finally { setIsBlockerSaving(false); }
  };

  const clearProjectBlocker = async (blockerId: string) => {
    setResolvingBlockerId(blockerId); setError(undefined);
    try {
      await resolveProjectBlocker(blockerId);
      await Promise.all([loadProjectBlockers(), loadBoard(false), loadFlowControl()]);
    } catch (blockerError) {
      setError(errorMessage(blockerError, "Unable to resolve the project blocker."));
    } finally { setResolvingBlockerId(undefined); }
  };

  const addArchitectureDecision = async (input: Omit<ArchitectureDecisionInput, "projectId">) => {
    if (!projectId) return;
    setIsKnowledgeSaving(true); setError(undefined);
    try {
      await createArchitectureDecision({ projectId, ...input });
      await loadArchitectureDecisions();
    } catch (knowledgeError) {
      setError(errorMessage(knowledgeError, "Unable to record the architecture proposal."));
    } finally { setIsKnowledgeSaving(false); }
  };

  const decideArchitecture = async (decisionId: string, status: "accepted" | "rejected") => {
    setDecidingArchitectureId(decisionId); setError(undefined);
    try {
      await decideArchitectureDecision(decisionId, status);
      await loadArchitectureDecisions();
    } catch (knowledgeError) {
      setError(errorMessage(knowledgeError, "Unable to decide the architecture proposal."));
    } finally { setDecidingArchitectureId(undefined); }
  };

  const generatePlanningProposal = async (agentId: string, goal: string) => {
    if (!projectId) return;
    setIsPlanningStarting(true); setError(undefined);
    try {
      const proposal = await startPlanningProposal(projectId, agentId, goal);
      setPlanningProposals((current) => [proposal, ...current]);
    } catch (planningError) {
      setError(errorMessage(planningError, "Unable to start the planning agent."));
      await loadPlanningProposals();
    } finally { setIsPlanningStarting(false); }
  };

  const approvePlan = async (proposalId: string) => {
    setPlanningActionId(proposalId); setError(undefined);
    try {
      await approvePlanningProposal(proposalId);
      await Promise.all([loadPlanningProposals(), loadBoard(false), loadOutcomes()]);
    } catch (planningError) {
      setError(errorMessage(planningError, "Unable to approve the planning proposal."));
    } finally { setPlanningActionId(undefined); }
  };

  const rejectPlan = async (proposalId: string) => {
    setPlanningActionId(proposalId); setError(undefined);
    try {
      await rejectPlanningProposal(proposalId);
      await loadPlanningProposals();
    } catch (planningError) {
      setError(errorMessage(planningError, "Unable to reject the planning proposal."));
    } finally { setPlanningActionId(undefined); }
  };

  const cancelPlan = async (proposalId: string) => {
    setPlanningActionId(proposalId); setError(undefined);
    try {
      await cancelPlanningProposal(proposalId);
    } catch (planningError) {
      setError(errorMessage(planningError, "Unable to cancel the planning agent."));
      setPlanningActionId(undefined);
    }
  };

  const addCollaboration = async (input: { taskId?: string; parentId?: string; kind: CollaborationKind; message: string; referencedTaskIds: string[] }) => {
    if (!projectId) return;
    setIsCollaborationSaving(true); setCollaborationActionId(input.parentId); setError(undefined);
    try { await createCollaborationEntry({ projectId, ...input }); await loadCollaboration(); }
    catch (collaborationError) { setError(errorMessage(collaborationError, "Unable to record collaboration activity.")); }
    finally { setIsCollaborationSaving(false); setCollaborationActionId(undefined); }
  };

  const resolveCollaboration = async (entryId: string) => {
    setCollaborationActionId(entryId); setError(undefined);
    try { await resolveCollaborationEntry(entryId); await loadCollaboration(); }
    catch (collaborationError) { setError(errorMessage(collaborationError, "Unable to resolve collaboration activity.")); }
    finally { setCollaborationActionId(undefined); }
  };

  const saveFlowLimits = async (limits: FlowLimitInput) => {
    if (!projectId) return;
    setIsFlowSaving(true);
    setError(undefined);
    try {
      setFlow(await updateFlowLimits(projectId, limits));
      await loadBoard();
    } catch (saveError) {
      setError(saveError instanceof Error ? saveError.message : "Unable to update flow limits.");
    } finally {
      setIsFlowSaving(false);
    }
  };

  const cleanupWorktree = async () => {
    if (!inspectedTask) return;
    setError(undefined);
    setIsCleaningWorktree(true);
    try {
      const updatedTask = await cleanupTaskWorktree(inspectedTask.id);
      setTasks((current) => current.map((task) => task.id === updatedTask.id ? updatedTask : task));
      setInspectedTask(updatedTask);
    } catch (cleanupError) {
      setError(cleanupError instanceof Error ? cleanupError.message : "Unable to remove the task worktree.");
    } finally {
      setIsCleaningWorktree(false);
    }
  };

  const openWorktree = async () => {
    if (!inspectedTask) return;
    setError(undefined);
    setIsOpeningWorktree(true);
    try {
      await openTaskWorktree(inspectedTask.id);
    } catch (openError) {
      setError(openError instanceof Error ? openError.message : "Unable to open the task worktree.");
    } finally {
      setIsOpeningWorktree(false);
    }
  };

  const resolveReview = async (decision: "approve" | "changes") => {
    if (!inspectedTask) return;
    setIsReviewActionPending(true); setError(undefined);
    try {
      const updatedTask = decision === "approve" ? await approveTaskReview(inspectedTask.id) : await requestTaskChanges(inspectedTask.id);
      setTasks((current) => current.map((task) => task.id === updatedTask.id ? updatedTask : task));
      setInspectedTask(updatedTask);
      await loadIntegrationQueue();
    } catch (reviewActionError) {
      setError(reviewActionError instanceof Error ? reviewActionError.message : "Unable to update the review state.");
    } finally { setIsReviewActionPending(false); }
  };

  const runArchitectReview = async (agentId: string) => {
    if (!inspectedTask) return;
    setError(undefined); setIsAgentReviewStarting(true);
    try {
      const reviewRun = await startAgentReview(inspectedTask.id, agentId);
      setAgentReviews((current) => [reviewRun, ...current]);
    } catch (reviewError) {
      setError(reviewError instanceof Error ? reviewError.message : "Unable to start the architect review.");
    } finally { setIsAgentReviewStarting(false); }
  };

  const integrateNext = async () => {
    if (!projectId) return;
    setError(undefined);
    setIsIntegrating(true);
    try {
      const result = await integrateNextTask(projectId);
      if (result.outcome !== "merged") setError(result.message);
      else if (result.cleanupError) setError(`Integrated successfully, but cleanup needs attention: ${result.cleanupError}`);
      await Promise.all([loadBoard(), loadIntegrationQueue(), loadRepository()]);
    } catch (integrationError) {
      setError(integrationError instanceof Error ? integrationError.message : "Unable to integrate the next task.");
      await loadIntegrationQueue();
    } finally {
      setIsIntegrating(false);
    }
  };

  const retryIntegration = async (attempt: IntegrationAttempt) => {
    setError(undefined);
    try {
      await retryIntegrationAttempt(attempt.id);
      await Promise.all([loadBoard(), loadIntegrationQueue()]);
    } catch (retryError) {
      setError(retryError instanceof Error ? retryError.message : "Unable to queue the integration retry.");
    }
  };

  const retryCleanup = async (attempt: IntegrationAttempt) => {
    setError(undefined);
    setRecoveringIntegrationId(attempt.id);
    try {
      await retryIntegrationCleanup(attempt.id);
      await Promise.all([loadBoard(), loadIntegrationQueue(), loadRepository()]);
    } catch (cleanupError) {
      setError(errorMessage(cleanupError, "Unable to retry integration cleanup."));
    } finally {
      setRecoveringIntegrationId(undefined);
    }
  };

  const revertMergedIntegration = async (attempt: IntegrationAttempt, createRepairTask: boolean) => {
    setError(undefined);
    try {
      await runConfirmedDestructiveAction({
        title: "Revert integrated change",
        message: `Create a normal Git revert for ${tasks.find((task) => task.id === attempt.taskId)?.title ?? attempt.taskId}? Shared history will not be rewritten${createRepairTask ? ", and a high-priority repair task will be created" : ""}.`,
        confirmLabel: createRepairTask ? "Revert + repair" : "Revert change",
      }, async () => {
        setRevertingIntegrationId(attempt.id);
        const reverted = await revertIntegration(attempt.id, createRepairTask);
        if (reverted.status === "failed" || reverted.status === "validation_failed") {
          setError(reverted.error ?? "The revert requires attention.");
        }
        await Promise.all([loadBoard(), loadIntegrationQueue(), loadRepository(), loadQualityGates()]);
      });
    } catch (revertError) {
      setError(errorMessage(revertError, "Unable to revert the integrated change."));
    } finally {
      setRevertingIntegrationId(undefined);
    }
  };

  const addValidationCommand = async (input: { stage: ValidationStage; name: string; program: string; arguments: string[] }) => {
    if (!projectId) return;
    try {
      await createValidationCommand({ projectId, ...input });
      await loadQualityGates();
    } catch (commandError) {
      setError(commandError instanceof Error ? commandError.message : "Unable to save validation command.");
    }
  };

  const removeValidationCommand = async (id: string) => {
    try {
      await deleteValidationCommand(id);
      await loadQualityGates();
    } catch (commandError) {
      setError(commandError instanceof Error ? commandError.message : "Unable to delete validation command.");
    }
  };

  const rerunQualityGates = async () => {
    if (!projectId) return;
    setIsRerunningIntegrationValidation(true);
    setError(undefined);
    try {
      await rerunIntegrationValidation(projectId);
      await Promise.all([loadQualityGates(), loadRepository(), loadIntegrationQueue()]);
    } catch (validationError) {
      setError(validationError instanceof Error ? validationError.message : "Unable to run integration validation.");
    } finally {
      setIsRerunningIntegrationValidation(false);
    }
  };

  if (isLoading) return <section className="page"><div className="empty-state"><span className="empty-index">SYNC</span><h2>Loading board</h2></div></section>;
  if (!project) return <section className="page"><div className="empty-state"><h2>Project not found</h2><Link className="secondary-button" to="/projects">Return to projects</Link></div></section>;

  const activeBlockerCount = projectBlockers.filter((blocker) => blocker.status === "active").length;
  const queuedIntegrationCount = integrationAttempts.filter((attempt) => attempt.status === "queued").length;
  const proposedPlanCount = planningProposals.filter((proposal) => proposal.status === "proposed").length;
  const openCollaborationCount = collaborationEntries.filter((entry) => !entry.parentId && entry.status === "open").length;
  const acceptedDecisionCount = architectureDecisions.filter((decision) => decision.status === "accepted").length;

  return (
    <section className={`board-page ${activeSidePanel ? "has-side-panel" : ""}`}>
      <header className="board-header">
        <Link className="back-link" to="/projects"><ArrowLeft size={15} /> Projects</Link>
        <div className="board-title-row">
          <div><p className="eyebrow">{project.defaultBranch} / local workspace</p><h1>{project.name}</h1><p className="muted">{project.description || "Project task board"}</p></div>
          <div className="board-header-actions">
            <div className="project-status-cluster" aria-label="Project status">
              <button className="project-status-button" type="button" onClick={() => openSidePanel("flow")}><Gauge size={14} /> Flow <strong>{flow?.activeWorkerRuns ?? 0}/{flow?.limits.workerMaxConcurrentRuns ?? 4}</strong>{Boolean(flow?.queued) && <span>+{flow?.queued}</span>}</button>
              <button className={`project-status-button project-health-button ${health?.status ?? "unknown"}`} type="button" onClick={() => openSidePanel("quality")}><Activity size={14} /> {health?.status ?? "unknown"}</button>
              <button className="project-status-button repository-status" type="button" onClick={() => openSidePanel("repository")} title="Inspect repository activity">
                <GitBranch size={14} />
                <span>{repository?.summary.currentBranch ?? project.defaultBranch}</span>
                <strong className={repository?.summary.isClean === false ? "repository-dirty" : repository ? "repository-clean" : "repository-pending"}>{repository?.summary.isClean === false ? `${repository.summary.changedFileCount} changed` : repository ? "Clean" : isRepositoryLoading ? "Checking" : "Unavailable"}</strong>
              </button>
            </div>
            {activeBlockerCount > 0 && <button className="project-alert-button" type="button" onClick={() => openSidePanel("blockers")}><ShieldAlert size={15} /> Blockers <span>{activeBlockerCount}</span></button>}
            {queuedIntegrationCount > 0 && <button className="secondary-button integration-action" type="button" onClick={() => openSidePanel("integration")}><GitMerge size={16} /> Integrate <span>{queuedIntegrationCount}</span></button>}
            <div className="project-tools" ref={projectToolsRef}>
              <button className="secondary-button" type="button" aria-haspopup="menu" aria-expanded={isProjectToolsOpen} aria-controls="project-tools-menu" onClick={() => setIsProjectToolsOpen((open) => !open)}><MoreHorizontal size={16} /> More</button>
              {isProjectToolsOpen && <div className="project-tools-menu" id="project-tools-menu" role="menu">
                <Link role="menuitem" to={`/projects/${project.id}/progress`} onClick={() => setIsProjectToolsOpen(false)}><ChartNoAxesCombined size={15} /> Progress</Link>
                <Link role="menuitem" to={`/projects/${project.id}/metrics`} onClick={() => setIsProjectToolsOpen(false)}><Activity size={15} /> Metrics & cost</Link>
                <Link role="menuitem" to={`/projects/${project.id}/autonomy`} onClick={() => setIsProjectToolsOpen(false)}><PlayCircle size={15} /> Autonomy</Link>
                <button type="button" role="menuitem" onClick={() => openProjectToolPanel("planning")}><Sparkles size={15} /> Plan <span>{proposedPlanCount}</span></button>
                <button type="button" role="menuitem" onClick={() => openProjectToolPanel("collaboration")}><MessagesSquare size={15} /> Collaborate <span>{openCollaborationCount}</span></button>
                <button type="button" role="menuitem" onClick={() => openProjectToolPanel("knowledge")}><BookOpenCheck size={15} /> Knowledge <span>{acceptedDecisionCount}</span></button>
                {activeBlockerCount === 0 && <button type="button" role="menuitem" onClick={() => openProjectToolPanel("blockers")}><ShieldAlert size={15} /> Blockers</button>}
                {queuedIntegrationCount === 0 && <button type="button" role="menuitem" onClick={() => openProjectToolPanel("integration")}><GitMerge size={15} /> Integration queue</button>}
              </div>}
            </div>
            <button className="primary-button" type="button" onClick={() => { setEditingTask(null); setIsCreating(true); }}><Plus size={16} /> New task</button>
          </div>
        </div>
      </header>
      <div className="board-body">
        <div className="board-notice">
          {error && <div className="inline-error" role="alert">{error}</div>}
        </div>
        <DndContext
          sensors={sensors}
          collisionDetection={columnCollisionDetection}
          onDragStart={({ active }) => setActiveTaskId(String(active.id))}
          onDragCancel={() => setActiveTaskId(undefined)}
          onDragEnd={(event) => { setActiveTaskId(undefined); void handleDragEnd(event); }}
        >
          <div className="kanban-board">
            {TASK_STATUSES.map((status) => <TaskColumn key={status} status={status} tasks={tasksByStatus[status]} agentNamesById={agentNamesById} recentlyTransitionedTaskIds={recentlyTransitionedTaskIds} onInspect={inspectTask} onEdit={setEditingTask} onDelete={(task) => void removeTask(task)} />)}
          </div>
          <DragOverlay dropAnimation={null}>
            {activeTaskId && <TaskDragPreview task={tasks.find((task) => task.id === activeTaskId)} />}
          </DragOverlay>
        </DndContext>
      </div>
      {(isCreating || editingTask) && <TaskDialog task={editingTask ?? undefined} agents={agents} milestones={milestones} epics={epics} onClose={() => { setIsCreating(false); setEditingTask(null); }} onSave={saveTask} />}
      {activeSidePanel === "task" && inspectedTask && <TaskDetailPanel task={inspectedTask} assignedAgent={agents.find((agent) => agent.id === inspectedTask.assignedAgentId)} recoveryAgents={agents.filter((agent) => agent.provider === "codex")} reviewerAgents={agents.filter((agent) => agent.id !== inspectedTask.assignedAgentId && agent.provider === "codex")} agentReviews={agentReviews} inputRequests={inputRequests} architectureDecisions={taskArchitectureDecisions} isAgentReviewStarting={isAgentReviewStarting} runs={runs} isStartingRun={isStartingRun} runRecoveryAction={runRecoveryAction} inputAction={inputAction} cancellingRunId={cancellingRunId} isCleaningWorktree={isCleaningWorktree} isOpeningWorktree={isOpeningWorktree} review={review} reviewError={reviewError} isReviewLoading={isReviewLoading} isReviewActionPending={isReviewActionPending} onClose={closeSidePanel} onEdit={(task) => { closeSidePanel(); setEditingTask(task); }} onStartRun={() => void startRun()} onCancelRun={(runId) => void cancelRun(runId)} onRecoverRun={(runId, mode, agentId) => void recoverRun(runId, mode, agentId)} onResolveRunFailure={(runId, action) => void resolveRunFailure(runId, action)} onRequestInput={(question, runId) => void requestHumanInput(question, runId)} onAnswerInput={(requestId, answer) => void answerHumanInput(requestId, answer)} onCleanupWorktree={() => void cleanupWorktree()} onOpenWorktree={() => void openWorktree()} onApproveReview={() => void resolveReview("approve")} onRequestChanges={() => void resolveReview("changes")} onStartAgentReview={(agentId) => void runArchitectReview(agentId)} />}
      {activeSidePanel === "repository" && projectId && <RepositoryInspector projectId={projectId} repository={repository} error={repositoryError} isLoading={isRepositoryLoading} onClose={closeSidePanel} onRefresh={() => void loadRepository()} />}
      {activeSidePanel === "integration" && <IntegrationQueuePanel attempts={integrationAttempts} reverts={revertAttempts} tasks={tasks} isLoading={isIntegrationQueueLoading} isIntegrating={isIntegrating} recoveringIntegrationId={recoveringIntegrationId} revertingIntegrationId={revertingIntegrationId} onClose={closeSidePanel} onRefresh={() => void loadIntegrationQueue()} onIntegrateNext={() => void integrateNext()} onRetry={(attempt) => void retryIntegration(attempt)} onRetryCleanup={(attempt) => void retryCleanup(attempt)} onRevert={(attempt, createRepairTask) => void revertMergedIntegration(attempt, createRepairTask)} />}
      {activeSidePanel === "quality" && <QualityGatesPanel health={health} implementationCommands={implementationCommands} integrationCommands={integrationCommands} attempts={validationAttempts} isLoading={isQualityLoading} isRunning={isRerunningIntegrationValidation} onClose={closeSidePanel} onRefresh={() => void loadQualityGates()} onAddCommand={addValidationCommand} onDeleteCommand={(id) => void removeValidationCommand(id)} onRerunIntegration={() => void rerunQualityGates()} />}
      {activeSidePanel === "flow" && <FlowControlPanel flow={flow} tasks={tasks} agents={agents} isLoading={isFlowLoading} isSaving={isFlowSaving} isScheduling={isScheduling} onClose={closeSidePanel} onRefresh={() => void loadFlowControl()} onSave={(limits) => void saveFlowLimits(limits)} onCancel={(runId) => void cancelRun(runId)} onSchedule={() => void scheduleProject()} />}
      {activeSidePanel === "blockers" && <ProjectBlockersPanel blockers={projectBlockers} tasks={tasks} isLoading={isBlockersLoading} isSaving={isBlockerSaving} resolvingId={resolvingBlockerId} onClose={closeSidePanel} onRefresh={() => void loadProjectBlockers()} onCreate={(input) => void addProjectBlocker(input)} onResolve={(blockerId) => void clearProjectBlocker(blockerId)} />}
      {activeSidePanel === "knowledge" && <ProjectKnowledgePanel decisions={architectureDecisions} tasks={tasks} previewTaskId={knowledgePreviewTaskId} previewDecisions={knowledgePreviewDecisions} isLoading={isKnowledgeLoading} isPreviewLoading={isKnowledgePreviewLoading} isSaving={isKnowledgeSaving} decidingId={decidingArchitectureId} onClose={closeSidePanel} onRefresh={() => void loadArchitectureDecisions()} onPreviewTask={setKnowledgePreviewTaskId} onCreate={(input) => void addArchitectureDecision(input)} onDecide={(decisionId, status) => void decideArchitecture(decisionId, status)} />}
      {activeSidePanel === "planning" && <PlanningPanel proposals={planningProposals} agents={agents.filter((agent) => agent.provider === "codex")} isLoading={isPlanningLoading} isStarting={isPlanningStarting} actionId={planningActionId} onClose={closeSidePanel} onRefresh={() => void loadPlanningProposals()} onStart={(agentId, goal) => void generatePlanningProposal(agentId, goal)} onApprove={(proposalId) => void approvePlan(proposalId)} onReject={(proposalId) => void rejectPlan(proposalId)} onCancel={(proposalId) => void cancelPlan(proposalId)} />}
      {activeSidePanel === "collaboration" && <CollaborationPanel entries={collaborationEntries} tasks={tasks} agents={agents} isLoading={isCollaborationLoading} isSaving={isCollaborationSaving} actionId={collaborationActionId} onClose={closeSidePanel} onRefresh={() => void loadCollaboration()} onCreate={(input) => void addCollaboration(input)} onResolve={(entryId) => void resolveCollaboration(entryId)} />}
    </section>
  );
}

function TaskColumn({ status, tasks, agentNamesById, recentlyTransitionedTaskIds, onInspect, onEdit, onDelete }: { status: TaskStatus; tasks: Task[]; agentNamesById: ReadonlyMap<string, string>; recentlyTransitionedTaskIds: string[]; onInspect: (task: Task) => void; onEdit: (task: Task) => void; onDelete: (task: Task) => void }) {
  const { setNodeRef, isOver } = useDroppable({ id: columnDropId(status), disabled: isSystemManagedStatus(status) });
  return (
    <section ref={setNodeRef} className={`kanban-column ${isOver ? "is-over" : ""}`}>
      <header className="column-header"><div><span className={`status-dot ${columns[status].tone}`} /><h2>{columns[status].label}</h2></div><span>{tasks.length}</span></header>
      <div className="task-list">
        <SortableContext items={tasks.map((task) => task.id)} strategy={verticalListSortingStrategy}>
          {tasks.map((task) => <TaskCard key={task.id} task={task} assignedAgentName={task.assignedAgentId ? agentNamesById.get(task.assignedAgentId) : undefined} isRecentlyTransitioned={recentlyTransitionedTaskIds.includes(task.id)} onInspect={onInspect} onEdit={onEdit} onDelete={onDelete} />)}
        </SortableContext>
        {tasks.length === 0 && <p className="empty-column">{isSystemManagedStatus(status) ? "Workflow managed" : "Drop task here"}</p>}
      </div>
    </section>
  );
}

function TaskCard({ task, assignedAgentName, isRecentlyTransitioned, onInspect, onEdit, onDelete }: { task: Task; assignedAgentName?: string; isRecentlyTransitioned: boolean; onInspect: (task: Task) => void; onEdit: (task: Task) => void; onDelete: (task: Task) => void }) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: task.id });
  return (
    <article ref={setNodeRef} style={{ transform: CSS.Transform.toString(transform), transition }} className={`task-card ${task.status === "in_progress" ? "is-running" : ""} ${isRecentlyTransitioned ? "just-transitioned" : ""} ${isDragging ? "is-dragging" : ""}`} {...attributes} {...listeners}>
      <span className="drag-handle" aria-hidden="true"><GripVertical size={15} /></span>
      <div className="task-card-copy" onClick={() => onInspect(task)}>
        <div className="task-card-title"><h3>{task.title}</h3><span className={`task-card-priority ${task.priority}`}>{task.priority}</span></div>
        {task.description && <p>{task.description}</p>}
        {task.status === "blocked" && task.blockedReason && <p className="task-card-blocked">{task.blockedReason}</p>}
        <div className={`task-assignment ${assignedAgentName ? "assigned" : "unassigned"}`}><Bot size={12} aria-hidden="true" /><span>{assignedAgentName ?? "Unassigned"}</span></div>
      </div>
      <div className="task-card-actions">
        <button type="button" onPointerDown={(event) => event.stopPropagation()} onClick={(event) => { event.stopPropagation(); onEdit(task); }} aria-label={`Edit ${task.title}`}><Pencil size={13} /></button>
        <button type="button" onPointerDown={(event) => event.stopPropagation()} onClick={(event) => { event.stopPropagation(); onDelete(task); }} aria-label={`Delete ${task.title}`}><Trash2 size={13} /></button>
      </div>
    </article>
  );
}

function TaskDragPreview({ task }: { task?: Task }) {
  if (!task) return null;
  return (
    <article className="task-card task-drag-overlay">
      <span className="drag-handle" aria-hidden="true"><GripVertical size={15} /></span>
      <div className="task-card-copy"><div className="task-card-title"><h3>{task.title}</h3><span className={`task-card-priority ${task.priority}`}>{task.priority}</span></div>{task.description && <p>{task.description}</p>}</div>
    </article>
  );
}

function columnDropId(status: TaskStatus) { return `column:${status}`; }

function columnCollisionDetection(args: Parameters<typeof pointerWithin>[0]) {
  const pointerCollisions = pointerWithin(args);
  return pointerCollisions.length > 0 ? pointerCollisions : closestCorners(args);
}

function statusForDropTarget(id: string, tasks: Task[]): TaskStatus | undefined {
  if (id.startsWith("column:")) return id.slice("column:".length) as TaskStatus;
  return tasks.find((task) => task.id === id)?.status;
}

function isSystemManagedStatus(status: TaskStatus) {
  return status === "needs_input" || status === "approved" || status === "integrating" || status === "blocked" || status === "done";
}

function moveTaskLocally(tasks: Task[], id: string, status: TaskStatus, position: number) {
  const active = tasks.find((task) => task.id === id);
  if (!active) return tasks;
  const source = tasks.filter((task) => task.status === active.status && task.id !== id).sort((a, b) => a.position - b.position);
  const target = active.status === status ? source : tasks.filter((task) => task.status === status).sort((a, b) => a.position - b.position);
  target.splice(Math.min(position, target.length), 0, { ...active, status });
  const updated = new Map<string, Task>();
  if (active.status !== status) source.forEach((task, index) => updated.set(task.id, { ...task, position: index }));
  target.forEach((task, index) => updated.set(task.id, { ...task, position: index }));
  return tasks.map((task) => updated.get(task.id) ?? task);
}
