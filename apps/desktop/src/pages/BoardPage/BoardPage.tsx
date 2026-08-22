import { closestCorners, DndContext, DragEndEvent, DragOverlay, KeyboardSensor, pointerWithin, PointerSensor, useSensor, useSensors, useDroppable } from "@dnd-kit/core";
import { arrayMove, SortableContext, sortableKeyboardCoordinates, useSortable, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { ArrowLeft, GitBranch, GripVertical, Pencil, Plus, SearchCode, Trash2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { RepositoryInspector } from "../../components/RepositoryInspector/RepositoryInspector";
import { TaskDetailPanel } from "../../components/TaskDetailPanel/TaskDetailPanel";
import { TaskDialog } from "../../components/TaskDialog/TaskDialog";
import { listAgents, type Agent } from "../../services/agents";
import { getProject, getRepositoryDetails, type Project, type RepositoryDetails } from "../../services/projects";
import { cancelTaskRun, listTaskRuns, startTaskRun, type TaskRun } from "../../services/runs";
import { createTask, deleteTask, listTasks, moveTask, TASK_STATUSES, type Task, type TaskInput, type TaskStatus, updateTask } from "../../services/tasks";
import { listenToWorkerRunEvents } from "../../services/workers";
import "./BoardPage.css";

const columns: Record<TaskStatus, { label: string; tone: string }> = {
  backlog: { label: "Backlog", tone: "neutral" },
  todo: { label: "Todo", tone: "blue" },
  in_progress: { label: "In Progress", tone: "amber" },
  review: { label: "Review", tone: "violet" },
  done: { label: "Done", tone: "green" },
};

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
  const [runs, setRuns] = useState<TaskRun[]>([]);
  const [isStartingRun, setIsStartingRun] = useState(false);
  const runIds = useRef(new Set<string>());
  const [repository, setRepository] = useState<RepositoryDetails>();
  const [repositoryError, setRepositoryError] = useState<string>();
  const [isRepositoryLoading, setIsRepositoryLoading] = useState(false);
  const [isRepositoryInspectorOpen, setIsRepositoryInspectorOpen] = useState(false);
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

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

  const loadBoard = useCallback(async () => {
    if (!projectId) return;
    setIsLoading(true);
    setError(undefined);
    try {
      const [loadedProject, loadedTasks, loadedAgents] = await Promise.all([getProject(projectId), listTasks(projectId), listAgents()]);
      setProject(loadedProject);
      setTasks(loadedTasks);
      setAgents(loadedAgents);
      void loadRepository();
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Unable to load the project board.");
    } finally {
      setIsLoading(false);
    }
  }, [loadRepository, projectId]);

  useEffect(() => {
    void loadBoard();
  }, [loadBoard]);

  useEffect(() => {
    if (!inspectedTask) {
      setRuns([]);
      return;
    }
    void listTaskRuns(inspectedTask.id).then(setRuns).catch((loadError: unknown) => {
      setError(loadError instanceof Error ? loadError.message : "Unable to load task runs.");
    });
  }, [inspectedTask?.id]);

  useEffect(() => {
    runIds.current = new Set(runs.map((run) => run.id));
  }, [runs]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listenToWorkerRunEvents((event) => {
      if (!runIds.current.has(event.runId)) return;
      if (event.kind === "output" && event.stream && event.text !== null) {
        const output = { stream: event.stream, text: event.text, createdAt: new Date().toISOString() };
        const timelineEvent = { id: -Date.now(), kind: "command.output", message: event.text, command: null, filePath: null, exitCode: null, createdAt: output.createdAt };
        setRuns((current) => current.map((run) => run.id === event.runId ? {
          ...run,
          output: [...run.output, output],
          events: [...run.events, timelineEvent],
        } : run));
        return;
      }
      if (event.kind !== "output" && inspectedTask) {
        void Promise.all([listTaskRuns(inspectedTask.id), loadBoard()]).then(([loadedRuns]) => setRuns(loadedRuns));
      }
    }).then((stopListening) => { unlisten = stopListening; });
    return () => unlisten?.();
  }, [inspectedTask?.id, loadBoard]);

  useEffect(() => {
    if (!inspectedTask) return;
    const current = tasks.find((task) => task.id === inspectedTask.id);
    if (current && current !== inspectedTask) setInspectedTask(current);
  }, [inspectedTask, tasks]);

  const tasksByStatus = useMemo(() => Object.fromEntries(
    TASK_STATUSES.map((status) => [status, tasks.filter((task) => task.status === status).sort((a, b) => a.position - b.position)]),
  ) as Record<TaskStatus, Task[]>, [tasks]);

  const handleDragEnd = async ({ active, over }: DragEndEvent) => {
    if (!over) return;
    const source = tasks.find((task) => task.id === active.id);
    if (!source) return;
    const destinationStatus = statusForDropTarget(String(over.id), tasks);
    if (!destinationStatus) return;
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
    if (!window.confirm(`Delete “${task.title}”? This cannot be undone.`)) return;
    try {
      await deleteTask(task.id);
      await loadBoard();
    } catch (deleteError) {
      setError(deleteError instanceof Error ? deleteError.message : "Unable to delete task.");
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
      await loadBoard();
    } catch (runError) {
      setError(runError instanceof Error ? runError.message : "Unable to start the Codex task.");
    } finally {
      setIsStartingRun(false);
    }
  };

  const cancelRun = async (runId: string) => {
    setError(undefined);
    try {
      await cancelTaskRun(runId);
    } catch (runError) {
      setError(runError instanceof Error ? runError.message : "Unable to cancel the Codex task.");
    }
  };

  if (isLoading) return <section className="page"><div className="empty-state"><span className="empty-index">SYNC</span><h2>Loading board</h2></div></section>;
  if (!project) return <section className="page"><div className="empty-state"><h2>Project not found</h2><Link className="secondary-button" to="/projects">Return to projects</Link></div></section>;

  return (
    <section className="board-page">
      <header className="board-header">
        <Link className="back-link" to="/projects"><ArrowLeft size={15} /> Projects</Link>
        <div className="board-title-row">
          <div><p className="eyebrow">{project.defaultBranch} / local workspace</p><h1>{project.name}</h1><p className="muted">{project.description || "Project task board"}</p></div>
          <div className="board-header-actions">
            <button className="repository-status" type="button" onClick={() => setIsRepositoryInspectorOpen(true)} title="Inspect repository activity">
              <GitBranch size={15} />
              <span>{repository?.summary.currentBranch ?? project.defaultBranch}</span>
              <strong className={repository?.summary.isClean === false ? "repository-dirty" : repository ? "repository-clean" : "repository-pending"}>{repository?.summary.isClean === false ? `${repository.summary.changedFileCount} changed` : repository ? "Clean" : isRepositoryLoading ? "Checking" : "Unavailable"}</strong>
            </button>
            <button className="secondary-button" type="button" onClick={() => setIsRepositoryInspectorOpen(true)}><SearchCode size={16} /> Inspect</button>
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
            {TASK_STATUSES.map((status) => <TaskColumn key={status} status={status} tasks={tasksByStatus[status]} onInspect={setInspectedTask} onEdit={setEditingTask} onDelete={(task) => void removeTask(task)} />)}
          </div>
          <DragOverlay dropAnimation={null}>
            {activeTaskId && <TaskDragPreview task={tasks.find((task) => task.id === activeTaskId)} />}
          </DragOverlay>
        </DndContext>
      </div>
      {(isCreating || editingTask) && <TaskDialog task={editingTask ?? undefined} agents={agents} onClose={() => { setIsCreating(false); setEditingTask(null); }} onSave={saveTask} />}
      {inspectedTask && <TaskDetailPanel task={inspectedTask} assignedAgent={agents.find((agent) => agent.id === inspectedTask.assignedAgentId)} runs={runs} isStartingRun={isStartingRun} onClose={() => setInspectedTask(null)} onEdit={(task) => { setInspectedTask(null); setEditingTask(task); }} onStartRun={() => void startRun()} onCancelRun={(runId) => void cancelRun(runId)} />}
      {isRepositoryInspectorOpen && projectId && <RepositoryInspector projectId={projectId} repository={repository} error={repositoryError} isLoading={isRepositoryLoading} onClose={() => setIsRepositoryInspectorOpen(false)} onRefresh={() => void loadRepository()} />}
    </section>
  );
}

function TaskColumn({ status, tasks, onInspect, onEdit, onDelete }: { status: TaskStatus; tasks: Task[]; onInspect: (task: Task) => void; onEdit: (task: Task) => void; onDelete: (task: Task) => void }) {
  const { setNodeRef, isOver } = useDroppable({ id: columnDropId(status) });
  return (
    <section ref={setNodeRef} className={`kanban-column ${isOver ? "is-over" : ""}`}>
      <header className="column-header"><div><span className={`status-dot ${columns[status].tone}`} /><h2>{columns[status].label}</h2></div><span>{tasks.length}</span></header>
      <div className="task-list">
        <SortableContext items={tasks.map((task) => task.id)} strategy={verticalListSortingStrategy}>
          {tasks.map((task) => <TaskCard key={task.id} task={task} onInspect={onInspect} onEdit={onEdit} onDelete={onDelete} />)}
        </SortableContext>
        {tasks.length === 0 && <p className="empty-column">Drop task here</p>}
      </div>
    </section>
  );
}

function TaskCard({ task, onInspect, onEdit, onDelete }: { task: Task; onInspect: (task: Task) => void; onEdit: (task: Task) => void; onDelete: (task: Task) => void }) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: task.id });
  return (
    <article ref={setNodeRef} style={{ transform: CSS.Transform.toString(transform), transition }} className={`task-card ${task.status === "in_progress" ? "is-running" : ""} ${isDragging ? "is-dragging" : ""}`} {...attributes} {...listeners}>
      <span className="drag-handle" aria-hidden="true"><GripVertical size={15} /></span>
      <div className="task-card-copy" onClick={() => onInspect(task)}>
        <h3>{task.title}</h3>
        {task.description && <p>{task.description}</p>}
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
      <div className="task-card-copy"><h3>{task.title}</h3>{task.description && <p>{task.description}</p>}</div>
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
