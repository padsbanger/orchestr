import type { DragEndEvent } from "@dnd-kit/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import type { DetailTab, TaskDetailPanelProps } from "../../components/TaskDetailPanel/TaskDetailPanel";
import { useWorkflowClock } from "../../components/WorkflowCockpit/WorkflowCockpit";
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
import { agentActivityDestination, attentionDestination, canChangePlanningStatus, canReorderPlanningTask, createFallbackWorkflowSnapshot, getProjectWorkflowSnapshot, listenToWorkflowChanges, loadWorkflowBoardView, mergeWorkflowSnapshots, saveWorkflowBoardView, type AttentionItem, type ProjectWorkflowSnapshot, type WorkflowBoardView, type WorkflowStage, type WorkflowTaskView } from "../../services/workflow";
import { BoardCockpit, BoardHeader, BoardSidePanels, BoardTaskDialog, type BoardSidePanel } from "./BoardPageView";
import { deriveBoardIndicators } from "./BoardPageModel";
import "./BoardPage.css";

export function BoardPage() {
  const { projectId } = useParams();
  const navigate = useNavigate();
  const [project, setProject] = useState<Project | null>();
  const [tasks, setTasks] = useState<Task[]>([]);
  const [agents, setAgents] = useState<Agent[]>([]);
  const [error, setError] = useState<string>();
  const [isLoading, setIsLoading] = useState(true);
  const [editingTask, setEditingTask] = useState<Task | null>();
  const [inspectedTask, setInspectedTask] = useState<Task | null>();
  const [inspectorTab, setInspectorTab] = useState<DetailTab>();
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
  const [workflowSnapshot, setWorkflowSnapshot] = useState<ProjectWorkflowSnapshot>();
  const [workflowSnapshotError, setWorkflowSnapshotError] = useState<string>();
  const [isWorkflowSnapshotLoading, setIsWorkflowSnapshotLoading] = useState(true);
  const [boardView, setBoardView] = useState<WorkflowBoardView>("flow");
  const [isAttentionExpanded, setIsAttentionExpanded] = useState(false);
  const [isAgentRailOpen, setIsAgentRailOpen] = useState(false);
  const [showIdleAgents, setShowIdleAgents] = useState(false);
  const [showAllDone, setShowAllDone] = useState(false);
  const [activeMobileStage, setActiveMobileStage] = useState<WorkflowStage>("queue");
  const projectToolsRef = useRef<HTMLDivElement>(null);
  const workflowRefreshTimer = useRef<number | undefined>(undefined);
  const workflowRequestSequence = useRef(0);
  const cockpitNow = useWorkflowClock();
  const isNarrowCockpit = useMediaQuery("(max-width: 1100px)");

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
    setWorkflowSnapshot(undefined);
    setWorkflowSnapshotError(undefined);
    setIsWorkflowSnapshotLoading(true);
    workflowRequestSequence.current += 1;
    setIsAttentionExpanded(false);
    setIsAgentRailOpen(false);
    setShowIdleAgents(false);
    setShowAllDone(false);
    setBoardView("flow");
    setActiveMobileStage("queue");
    setActiveSidePanel(undefined);
    setInspectedTask(null);
    setInspectorTab(undefined);
  }, [projectId]);

  useEffect(() => {
    if (!projectId) return;
    let disposed = false;
    void loadWorkflowBoardView(projectId).then((value) => {
      if (disposed) return;
      setBoardView(value);
    }).catch(() => {
      if (!disposed) setBoardView("flow");
    });
    return () => { disposed = true; };
  }, [projectId]);

  useEffect(() => {
    if (!isProjectToolsOpen) return;
    const closeWhenOutside = (event: PointerEvent) => {
      if (event.target instanceof Node && !elementContains(projectToolsRef.current, event.target)) setIsProjectToolsOpen(false);
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
    setIsAgentRailOpen(false);
    setInspectedTask(null);
    setInspectorTab(undefined);
    setActiveSidePanel(panel);
  };

  const openProjectToolPanel = (panel: Exclude<BoardSidePanel, "task">) => {
    setIsProjectToolsOpen(false);
    openSidePanel(panel);
  };

  const inspectTask = (task: Task) => {
    setIsAgentRailOpen(false);
    setRuns([]);
    setInputRequests([]);
    setTaskArchitectureDecisions([]);
    setReview(undefined);
    setAgentReviews([]);
    setInspectorTab(undefined);
    setInspectedTask(task);
    setActiveSidePanel("task");
  };

  const closeSidePanel = () => {
    setActiveSidePanel(undefined);
    setInspectedTask(null);
    setInspectorTab(undefined);
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

  const loadWorkflowSnapshot = useCallback(async () => {
    if (!projectId) return;
    const requestSequence = ++workflowRequestSequence.current;
    setIsWorkflowSnapshotLoading(true);
    try {
      const snapshot = await getProjectWorkflowSnapshot(projectId);
      if (requestSequence !== workflowRequestSequence.current) return;
      setWorkflowSnapshot(snapshot);
      setWorkflowSnapshotError(undefined);
    } catch (snapshotError) {
      if (requestSequence !== workflowRequestSequence.current) return;
      setWorkflowSnapshotError(errorMessage(snapshotError, "Live workflow status is unavailable. Displayed task data may be incomplete."));
    } finally {
      if (requestSequence === workflowRequestSequence.current) setIsWorkflowSnapshotLoading(false);
    }
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
      void loadOutcomes();
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Unable to load the project board.");
    } finally {
      if (showLoading) setIsLoading(false);
    }
  }, [loadOutcomes, projectId]);

  useEffect(() => {
    switch (activeSidePanel) {
      case "repository": void loadRepository(); break;
      case "integration": void loadIntegrationQueue(); break;
      case "quality": void loadQualityGates(); break;
      case "flow": void loadFlowControl(); break;
      case "blockers": void loadProjectBlockers(); break;
      case "knowledge": void loadArchitectureDecisions(); break;
      case "planning": void loadPlanningProposals(); break;
      case "collaboration": void loadCollaboration(); break;
      default: break;
    }
  }, [activeSidePanel, loadArchitectureDecisions, loadCollaboration, loadFlowControl, loadIntegrationQueue, loadPlanningProposals, loadProjectBlockers, loadQualityGates, loadRepository]);

  useEffect(() => {
    void Promise.all([loadBoard(true), loadWorkflowSnapshot()]);
  }, [loadBoard, loadWorkflowSnapshot]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listenToPlanningEvents(() => { void loadPlanningProposals(); }).then((stopListening) => {
      if (disposed) stopListening(); else unlisten = stopListening;
    });
    return () => { disposed = true; stopListening(unlisten); };
  }, [loadPlanningProposals]);

  useEffect(() => {
    if (!inspectedTask) {
      setRuns([]);
      return;
    }
    if (inspectorTab !== "activity") return;
    void listTaskRuns(inspectedTask.id).then(setRuns).catch((loadError: unknown) => {
      setError(loadError instanceof Error ? loadError.message : "Unable to load task runs.");
    });
  }, [taskId(inspectedTask), taskStatus(inspectedTask), inspectorTab]);

  useEffect(() => {
    if (!inspectedTask) { setInputRequests([]); return; }
    if (inspectorTab !== "activity") return;
    void listTaskInputRequests(inspectedTask.id).then(setInputRequests).catch((loadError: unknown) => {
      setError(errorMessage(loadError, "Unable to load task input requests."));
    });
  }, [taskId(inspectedTask), taskStatus(inspectedTask), inspectorTab]);

  useEffect(() => {
    if (!inspectedTask) { setTaskArchitectureDecisions([]); return; }
    if (inspectorTab !== "work") return;
    void listRelevantArchitectureDecisions(inspectedTask.id).then(setTaskArchitectureDecisions).catch((loadError: unknown) => {
      setError(errorMessage(loadError, "Unable to load task architecture context."));
    });
  }, [taskId(inspectedTask), inspectorTab]);

  useEffect(() => {
    if (!knowledgePreviewTaskId) { setKnowledgePreviewDecisions([]); return; }
    setIsKnowledgePreviewLoading(true);
    void listRelevantArchitectureDecisions(knowledgePreviewTaskId).then(setKnowledgePreviewDecisions).catch((loadError: unknown) => {
      setError(errorMessage(loadError, "Unable to preview task knowledge."));
    }).finally(() => setIsKnowledgePreviewLoading(false));
  }, [architectureDecisions, knowledgePreviewTaskId]);

  useEffect(() => {
    if (!inspectedTask) { setReview(undefined); setReviewError(undefined); return; }
    if (inspectorTab !== "review" || inspectedTask.status !== "review") return;
    setIsReviewLoading(true); setReviewError(undefined);
    void getTaskReview(inspectedTask.id).then(setReview).catch((error: unknown) => setReviewError(error instanceof Error ? error.message : "Unable to load task branch changes.")).finally(() => setIsReviewLoading(false));
  }, [taskId(inspectedTask), taskStatus(inspectedTask), inspectorTab]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listenToAgentReviewEvents((reviewId) => {
      if (!inspectedTask || inspectorTab !== "review") return;
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
    return () => { disposed = true; stopListening(unlisten); };
  }, [taskId(inspectedTask), inspectorTab, loadBoard, loadIntegrationQueue]);

  useEffect(() => {
    if (!inspectedTask) { setAgentReviews([]); return; }
    if (inspectorTab !== "review") return;
    let disposed = false;
    void listAgentReviews(inspectedTask.id).then((reviews) => {
      if (disposed) return;
      setAgentReviews(reviews);
    }).catch((loadError: unknown) => {
      if (!disposed) setError(loadError instanceof Error ? loadError.message : "Unable to load architect reviews.");
    });
    return () => { disposed = true; };
  }, [taskId(inspectedTask), inspectorTab]);

  useEffect(() => {
    if (!inspectedTask || inspectorTab !== "review" || !agentReviews.some((review) => review.status === "running")) return;
    const interval = window.setInterval(() => {
      void listAgentReviews(inspectedTask.id).then(setAgentReviews).catch(() => undefined);
    }, 1_500);
    return () => window.clearInterval(interval);
  }, [agentReviews, taskId(inspectedTask), inspectorTab]);

  useEffect(() => {
    if (!inspectedTask || inspectorTab !== "review") return;
    void Promise.all([loadIntegrationQueue(), loadQualityGates()]);
  }, [taskId(inspectedTask), inspectorTab, loadIntegrationQueue, loadQualityGates]);

  useEffect(() => {
    runIds.current = new Set(runs.map((run) => run.id));
  }, [runs]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listenToFlowChanges(() => { void Promise.all([loadBoard(), loadFlowControl()]); }).then((stopListening) => {
      if (disposed) stopListening(); else unlisten = stopListening;
    });
    return () => { disposed = true; stopListening(unlisten); };
  }, [loadBoard, loadFlowControl]);

  useEffect(() => {
    if (!projectId) return;
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listenToWorkflowChanges((event) => {
      if (event.projectId !== projectId) return;
      if (workflowRefreshTimer.current !== undefined) window.clearTimeout(workflowRefreshTimer.current);
      workflowRefreshTimer.current = window.setTimeout(() => {
        workflowRefreshTimer.current = undefined;
        void Promise.all([
          loadWorkflowSnapshot(),
          listTasks(projectId).then(setTasks),
        ]);
      }, 100);
    }).then((stopListening) => {
      if (disposed) stopListening();
      else unlisten = stopListening;
    }).catch(() => undefined);
    return () => {
      disposed = true;
      stopListening(unlisten);
      if (workflowRefreshTimer.current !== undefined) {
        window.clearTimeout(workflowRefreshTimer.current);
        workflowRefreshTimer.current = undefined;
      }
    };
  }, [loadWorkflowSnapshot, projectId]);

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
      stopListening(unlisten);
    };
  }, [taskId(inspectedTask), loadBoard, loadFlowControl]);

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
    return () => { disposed = true; stopListening(unlisten); };
  }, [loadQualityGates, projectId]);

  const tasksByStatus = useMemo(() => Object.fromEntries(
    TASK_STATUSES.map((status) => [status, tasks.filter((task) => task.status === status).sort((a, b) => a.position - b.position)]),
  ) as Record<TaskStatus, Task[]>, [tasks]);
  const tasksById = useMemo(() => new Map(tasks.map((task) => [task.id, task])), [tasks]);
  const fallbackWorkflow = useMemo(() => projectId ? createFallbackWorkflowSnapshot({
    projectId,
    tasks,
    agents,
    health,
    flow,
    blockers: projectBlockers,
    integrations: integrationAttempts,
    proposals: planningProposals,
    collaboration: collaborationEntries,
  }) : undefined, [agents, collaborationEntries, flow, health, integrationAttempts, planningProposals, projectBlockers, projectId, tasks]);
  const workflow = useMemo(() => mergeWorkflowSnapshots(workflowSnapshot, fallbackWorkflow), [fallbackWorkflow, workflowSnapshot]);
  const workflowTaskViewsById = useMemo(() => taskViewsById(workflow), [workflow]);
  const idleAgents = useMemo(() => findIdleAgents(agents, workflow), [agents, workflow]);

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
    if (!canReorderPlanningTask(source.status, destinationStatus)) {
      setError("Drag only reorders tasks within Draft or Ready. Use Mark ready or Defer to change planning state.");
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

  const changePlanningStatus = async (task: Task, status: "backlog" | "ready") => {
    if (task.status === status) return;
    if (!canChangePlanningStatus(task.status, status)) {
      setError("Only Draft and Ready tasks can be changed from the board. Workflow transitions use their dedicated actions.");
      return;
    }
    const destinationPosition = tasksByStatus[status].length;
    const beforeMove = tasks;
    setTasks(moveTaskLocally(tasks, task.id, status, destinationPosition));
    setError(undefined);
    try {
      await moveTask(task.id, status, destinationPosition);
      await loadBoard();
    } catch (moveError) {
      setTasks(beforeMove);
      setError(errorMessage(moveError, `Unable to ${status === "ready" ? "mark the task ready" : "defer the task"}.`));
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

  const changeBoardView = (view: WorkflowBoardView) => {
    if (!projectId || view === boardView) return;
    const previous = boardView;
    setBoardView(view);
    setActiveTaskId(undefined);
    void saveWorkflowBoardView(projectId, view).catch((saveError: unknown) => {
      setBoardView(previous);
      setError(errorMessage(saveError, "Unable to save the board view."));
    });
  };

  const openAttentionItem = (item: AttentionItem) => {
    const destination = attentionDestination(item);
    if (!destination) return;
    if (destination.kind === "task") {
      const task = tasksById.get(destination.taskId);
      if (task) {
        inspectTask(task);
      }
      return;
    }
    if (destination.kind === "panel") {
      openSidePanel(destination.panel);
      return;
    }
    if (projectId) void navigate(`/projects/${projectId}/${destination.route}`);
  };

  const openAgentActivity = (activity: ProjectWorkflowSnapshot["agentActivity"][number]) => {
    const destination = agentActivityDestination(activity);
    if (destination.kind === "task") {
      const task = tasksById.get(destination.taskId);
      if (task) inspectTask(task);
      return;
    }
    if (destination.kind === "panel") {
      openSidePanel(destination.panel);
      return;
    }
    void navigate(`/${destination.route}`);
  };

  if (isLoading) return <section className="page"><div className="empty-state"><span className="empty-index">SYNC</span><h2>Loading board</h2></div></section>;
  if (!project) return <section className="page"><div className="empty-state"><h2>Project not found</h2><Link className="secondary-button" to="/projects">Return to projects</Link></div></section>;

  const indicators = deriveBoardIndicators({ workflow, blockers: projectBlockers, integrations: integrationAttempts, proposals: planningProposals, collaboration: collaborationEntries, decisions: architectureDecisions, flow, agents });

  const inspectedWorkflowView = workflowViewForTask(inspectedTask, workflowTaskViewsById);
  const taskPanelProps = buildTaskPanelProps(inspectedTask, inspectedWorkflowView, {
    assignedAgent: assignedAgentForTask(inspectedTask, agents),
    recoveryAgents: agents.filter((agent) => agent.provider === "codex"),
    reviewerAgents: reviewerAgentsForTask(inspectedTask, agents),
    agentReviews,
    inputRequests,
    architectureDecisions: taskArchitectureDecisions,
    isAgentReviewStarting,
    runs,
    isStartingRun,
    runRecoveryAction,
    inputAction,
    cancellingRunId,
    isCleaningWorktree,
    isOpeningWorktree,
    review,
    reviewError,
    isReviewLoading,
    isReviewActionPending,
    integrationAttempts,
    revertAttempts,
    validationAttempts,
    onTabChange: setInspectorTab,
    onClose: closeSidePanel,
    onEdit: (task: Task) => { closeSidePanel(); setEditingTask(task); },
    onStartRun: () => void startRun(),
    onCancelRun: (runId: string) => void cancelRun(runId),
    onRecoverRun: (runId: string, mode: "resume" | "restart_clean", agentId?: string) => void recoverRun(runId, mode, agentId),
    onResolveRunFailure: (runId: string, action: "abandon" | "escalate") => void resolveRunFailure(runId, action),
    onRequestInput: (question: string, runId?: string) => void requestHumanInput(question, runId),
    onAnswerInput: (requestId: string, answer: string) => void answerHumanInput(requestId, answer),
    onCleanupWorktree: () => void cleanupWorktree(),
    onOpenWorktree: () => void openWorktree(),
    onApproveReview: () => void resolveReview("approve"),
    onRequestChanges: () => void resolveReview("changes"),
    onStartAgentReview: (agentId: string) => void runArchitectReview(agentId),
  });

  return (
    <section className={boardPageClass(activeSidePanel)}>
      <BoardHeader
        project={project}
        workflow={workflow}
        health={health}
        repository={repository}
        isRepositoryLoading={isRepositoryLoading}
        {...indicators}
        isProjectToolsOpen={isProjectToolsOpen}
        projectToolsRef={projectToolsRef}
        onToggleProjectTools={() => setIsProjectToolsOpen((open) => !open)}
        onCloseProjectTools={() => setIsProjectToolsOpen(false)}
        onOpenSidePanel={openSidePanel}
        onOpenProjectToolPanel={openProjectToolPanel}
        onNewTask={() => { setEditingTask(null); setIsCreating(true); }}
      />
      <BoardCockpit
        error={error}
        boardView={boardView}
        workflow={workflow}
        workflowSnapshot={workflowSnapshot}
        workflowSnapshotError={workflowSnapshotError}
        isWorkflowSnapshotLoading={isWorkflowSnapshotLoading}
        isAttentionExpanded={isAttentionExpanded}
        isAgentRailOpen={isAgentRailOpen}
        isAgentRailDrawer={agentRailIsDrawer(isNarrowCockpit, activeSidePanel)}
        showIdleAgents={showIdleAgents}
        showAllDone={showAllDone}
        activeMobileStage={activeMobileStage}
        activeTaskId={activeTaskId}
        tasks={tasks}
        tasksByStatus={tasksByStatus}
        tasksById={tasksById}
        taskViewsById={workflowTaskViewsById}
        idleAgents={idleAgents}
        recentlyTransitionedTaskIds={recentlyTransitionedTaskIds}
        now={cockpitNow}
        onChangeView={changeBoardView}
        onToggleAttention={() => setIsAttentionExpanded((expanded) => !expanded)}
        onOpenAttention={openAttentionItem}
        onToggleAgentRail={() => setIsAgentRailOpen((open) => !open)}
        onCloseAgentRail={() => setIsAgentRailOpen(false)}
        onToggleIdle={() => setShowIdleAgents((show) => !show)}
        onToggleDone={() => setShowAllDone((showAll) => !showAll)}
        onChangeMobileStage={setActiveMobileStage}
        onOpenActivity={openAgentActivity}
        onDragStart={setActiveTaskId}
        onDragCancel={() => setActiveTaskId(undefined)}
        onDragEnd={(event) => { setActiveTaskId(undefined); void handleDragEnd(event); }}
        onInspect={inspectTask}
        onEdit={setEditingTask}
        onDelete={(task) => void removeTask(task)}
        onPlanningState={(task, status) => void changePlanningStatus(task, status)}
      />
      <BoardTaskDialog isCreating={isCreating} editingTask={editingTask} agents={agents} milestones={milestones} epics={epics} onClose={() => { setIsCreating(false); setEditingTask(null); }} onSave={saveTask} />
      <BoardSidePanels
        activePanel={activeSidePanel}
        task={taskPanelProps}
        repository={{ projectId: project.id, repository, error: repositoryError, isLoading: isRepositoryLoading, onClose: closeSidePanel, onRefresh: () => void loadRepository() }}
        integration={{ attempts: integrationAttempts, reverts: revertAttempts, tasks, isLoading: isIntegrationQueueLoading, isIntegrating, recoveringIntegrationId, revertingIntegrationId, onClose: closeSidePanel, onRefresh: () => void loadIntegrationQueue(), onIntegrateNext: () => void integrateNext(), onRetry: (attempt) => void retryIntegration(attempt), onRetryCleanup: (attempt) => void retryCleanup(attempt), onRevert: (attempt, createRepairTask) => void revertMergedIntegration(attempt, createRepairTask) }}
        quality={{ health, implementationCommands, integrationCommands, attempts: validationAttempts, isLoading: isQualityLoading, isRunning: isRerunningIntegrationValidation, onClose: closeSidePanel, onRefresh: () => void loadQualityGates(), onAddCommand: addValidationCommand, onDeleteCommand: (id) => void removeValidationCommand(id), onRerunIntegration: () => void rerunQualityGates() }}
        flow={{ flow, tasks, agents, isLoading: isFlowLoading, isSaving: isFlowSaving, isScheduling, onClose: closeSidePanel, onRefresh: () => void loadFlowControl(), onSave: (limits) => void saveFlowLimits(limits), onCancel: (runId) => void cancelRun(runId), onSchedule: () => void scheduleProject() }}
        blockers={{ blockers: projectBlockers, tasks, isLoading: isBlockersLoading, isSaving: isBlockerSaving, resolvingId: resolvingBlockerId, onClose: closeSidePanel, onRefresh: () => void loadProjectBlockers(), onCreate: (input) => void addProjectBlocker(input), onResolve: (blockerId) => void clearProjectBlocker(blockerId) }}
        knowledge={{ decisions: architectureDecisions, tasks, previewTaskId: knowledgePreviewTaskId, previewDecisions: knowledgePreviewDecisions, isLoading: isKnowledgeLoading, isPreviewLoading: isKnowledgePreviewLoading, isSaving: isKnowledgeSaving, decidingId: decidingArchitectureId, onClose: closeSidePanel, onRefresh: () => void loadArchitectureDecisions(), onPreviewTask: setKnowledgePreviewTaskId, onCreate: (input) => void addArchitectureDecision(input), onDecide: (decisionId, status) => void decideArchitecture(decisionId, status) }}
        planning={{ proposals: planningProposals, agents: agents.filter((agent) => agent.provider === "codex"), isLoading: isPlanningLoading, isStarting: isPlanningStarting, actionId: planningActionId, onClose: closeSidePanel, onRefresh: () => void loadPlanningProposals(), onStart: (agentId, goal) => void generatePlanningProposal(agentId, goal), onApprove: (proposalId) => void approvePlan(proposalId), onReject: (proposalId) => void rejectPlan(proposalId), onCancel: (proposalId) => void cancelPlan(proposalId) }}
        collaboration={{ entries: collaborationEntries, tasks, agents, isLoading: isCollaborationLoading, isSaving: isCollaborationSaving, actionId: collaborationActionId, onClose: closeSidePanel, onRefresh: () => void loadCollaboration(), onCreate: (input) => void addCollaboration(input), onResolve: (entryId) => void resolveCollaboration(entryId) }}
      />
    </section>
  );
}

function statusForDropTarget(id: string, tasks: Task[]): TaskStatus | undefined {
  if (id.startsWith("column:")) return id.slice("column:".length) as TaskStatus;
  return tasks.find((task) => task.id === id)?.status;
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

function useMediaQuery(query: string) {
  const [matches, setMatches] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    const media = window.matchMedia(query);
    const update = () => setMatches(media.matches);
    update();
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, [query]);
  return matches;
}

function stopListening(listener: (() => void) | undefined) {
  if (listener) listener();
}

function elementContains(element: HTMLElement | null, node: Node): boolean {
  return element ? element.contains(node) : false;
}

function taskId(task: Task | null | undefined): string | undefined {
  return task ? task.id : undefined;
}

function taskStatus(task: Task | null | undefined): TaskStatus | undefined {
  return task ? task.status : undefined;
}

function taskViewsById(workflow: ProjectWorkflowSnapshot | undefined): Map<string, WorkflowTaskView> {
  const tasks = workflow ? workflow.stages.flatMap((stage) => stage.tasks) : [];
  return new Map(tasks.map((task) => [task.id, task]));
}

function findIdleAgents(agents: Agent[], workflow: ProjectWorkflowSnapshot | undefined): Agent[] {
  const activities = workflow ? workflow.agentActivity : [];
  const activeAgentIds = new Set(activities.map((activity) => activity.agentId));
  return agents.filter((agent) => !activeAgentIds.has(agent.id));
}

function workflowViewForTask(task: Task | null | undefined, taskViews: ReadonlyMap<string, WorkflowTaskView>): WorkflowTaskView | undefined {
  return task ? taskViews.get(task.id) : undefined;
}

function assignedAgentForTask(task: Task | null | undefined, agents: Agent[]): Agent | undefined {
  return task ? agents.find((agent) => agent.id === task.assignedAgentId) : undefined;
}

function reviewerAgentsForTask(task: Task | null | undefined, agents: Agent[]): Agent[] {
  return agents.filter((agent) => agent.provider === "codex" && agent.id !== task?.assignedAgentId);
}

function buildTaskPanelProps(task: Task | null | undefined, workflowView: WorkflowTaskView | undefined, props: Omit<TaskDetailPanelProps, "task" | "workflowView">): TaskDetailPanelProps | undefined {
  if (!task || !workflowView) return undefined;
  return { task, workflowView, ...props };
}

function boardPageClass(activePanel: BoardSidePanel | undefined): string {
  return activePanel ? "board-page has-side-panel" : "board-page";
}

function agentRailIsDrawer(isNarrow: boolean, activePanel: BoardSidePanel | undefined): boolean {
  return isNarrow || Boolean(activePanel);
}
