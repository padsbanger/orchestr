use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    thread,
};

use orchestr_db::{
    Agent, AgentReview, AgentReviewDecision, AgentReviewStatus, AgentUpdate, Database, Epic,
    FlowLimitUpdate, FlowLimits, FlowState, IntegrationAttempt, Milestone, NewAgent,
    NewAgentReview, NewEpic, NewMilestone, NewProject, NewRun, NewRunEvent, NewTask,
    NewValidationCommand, NewValidationEvent, Project, ProjectDeletion, ProjectHealth,
    ProjectProgress, RevertAttempt, RevertStatus, Run, RunEvent, RunOutput, RunStatus, Task,
    TaskPriority, TaskStatus, TaskUpdate, ValidationAttempt, ValidationCommand, ValidationStage,
    ValidationStatus, Workspace,
};
use orchestr_git::{GitService, IntegrationPreparation, IntegrationResult, RepositoryDetails};
use orchestr_provider::{
    AgentProvider, AgentRunInput, CodexProvider, ProviderAction, ProviderReadiness, ProviderStatus,
};
use orchestr_worker::{LocalWorker, OutputStream, ProcessRequest, WorkerHandle, WorkerProfile};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

const LOCAL_WORKER_ID: &str = "local";

struct AppState {
    database: Arc<Mutex<Database>>,
    local_worker_runs: Arc<Mutex<HashMap<String, ActiveLocalRun>>>,
}

struct ActiveLocalRun {
    handle: WorkerHandle,
    cancel_requested: bool,
}

#[derive(Clone)]
struct RepositoryObservation {
    changed_files: HashMap<String, String>,
    latest_commit: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateProjectInput {
    name: String,
    description: Option<String>,
    parent_path: String,
    directory_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterProjectInput {
    name: String,
    description: Option<String>,
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateTaskInput {
    project_id: String,
    title: String,
    description: Option<String>,
    #[serde(default)]
    acceptance_criteria: Vec<String>,
    implementation_notes: Option<String>,
    #[serde(default)]
    relevant_paths: Vec<String>,
    #[serde(default)]
    dependency_ids: Vec<String>,
    assigned_agent_id: Option<String>,
    #[serde(default = "default_task_priority")]
    priority: String,
    milestone_id: Option<String>,
    epic_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateTaskInput {
    id: String,
    title: String,
    description: Option<String>,
    #[serde(default)]
    acceptance_criteria: Vec<String>,
    implementation_notes: Option<String>,
    #[serde(default)]
    relevant_paths: Vec<String>,
    #[serde(default)]
    dependency_ids: Vec<String>,
    assigned_agent_id: Option<String>,
    #[serde(default = "default_task_priority")]
    priority: String,
    milestone_id: Option<String>,
    epic_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateAgentInput {
    name: String,
    provider: String,
    role: String,
    model: Option<String>,
    system_prompt: Option<String>,
    #[serde(default)]
    skills: Vec<String>,
    max_concurrent_tasks: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateAgentInput {
    id: String,
    name: String,
    provider: String,
    role: String,
    model: Option<String>,
    system_prompt: Option<String>,
    #[serde(default)]
    skills: Vec<String>,
    max_concurrent_tasks: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveTaskInput {
    id: String,
    status: String,
    position: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepositoryDiffInput {
    project_id: String,
    file_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartedWorkerRun {
    run_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkerRunEvent {
    run_id: String,
    kind: String,
    stream: Option<OutputStream>,
    text: Option<String>,
    raw_text: Option<String>,
    command: Option<String>,
    exit_code: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunOutputResponse {
    stream: String,
    text: String,
    created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunEventResponse {
    id: i64,
    kind: String,
    message: String,
    command: Option<String>,
    file_path: Option<String>,
    exit_code: Option<i32>,
    created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunResponse {
    id: String,
    task_id: String,
    agent_id: String,
    worker_id: String,
    status: String,
    started_at: String,
    completed_at: Option<String>,
    exit_code: Option<i32>,
    error: Option<String>,
    output: Vec<RunOutputResponse>,
    events: Vec<RunEventResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartedTaskRunResponse {
    run: RunResponse,
    task: TaskResponse,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecoverTaskRunInput {
    run_id: String,
    mode: String,
    agent_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveFailedRunInput {
    run_id: String,
    action: String,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateFlowLimitsInput {
    project_id: String,
    worker_max_concurrent_runs: i64,
    in_progress_limit: i64,
    review_limit: i64,
    approved_limit: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FlowLimitsResponse {
    project_id: String,
    worker_id: String,
    worker_max_concurrent_runs: i64,
    in_progress_limit: i64,
    review_limit: i64,
    approved_limit: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FlowStateResponse {
    limits: FlowLimitsResponse,
    active_worker_runs: i64,
    in_progress: i64,
    review: i64,
    approved: i64,
    integrating: i64,
    queued: i64,
    blocked_reason: Option<String>,
    queue: Vec<RunResponse>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartAgentReviewInput {
    task_id: String,
    agent_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentReviewResponse {
    id: String,
    task_id: String,
    agent_id: String,
    status: String,
    decision: Option<String>,
    notes: Option<String>,
    raw_output: String,
    error: Option<String>,
    started_at: String,
    completed_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectResponse {
    id: String,
    name: String,
    description: Option<String>,
    default_branch: String,
    created_at: String,
    updated_at: String,
    workspaces: Vec<WorkspaceResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceResponse {
    id: String,
    project_id: String,
    worker_id: String,
    path: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskResponse {
    id: String,
    project_id: String,
    title: String,
    description: Option<String>,
    acceptance_criteria: Vec<String>,
    implementation_notes: Option<String>,
    relevant_paths: Vec<String>,
    dependency_ids: Vec<String>,
    assigned_agent_id: Option<String>,
    branch: Option<String>,
    worktree_path: Option<String>,
    priority: String,
    blocked_reason: Option<String>,
    milestone_id: Option<String>,
    epic_id: Option<String>,
    status: String,
    position: i64,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateMilestoneInput {
    project_id: String,
    title: String,
    description: Option<String>,
    status: String,
    target_date: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateEpicInput {
    project_id: String,
    milestone_id: Option<String>,
    title: String,
    description: Option<String>,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateOutcomeStatusInput {
    id: String,
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MilestoneResponse {
    id: String,
    project_id: String,
    title: String,
    description: Option<String>,
    status: String,
    target_date: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EpicResponse {
    id: String,
    project_id: String,
    milestone_id: Option<String>,
    title: String,
    description: Option<String>,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskProgressCountsResponse {
    total: i64,
    backlog: i64,
    ready: i64,
    in_progress: i64,
    review: i64,
    blocked: i64,
    done: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MilestoneProgressResponse {
    milestone: MilestoneResponse,
    counts: TaskProgressCountsResponse,
    epics: Vec<EpicResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectProgressResponse {
    counts: TaskProgressCountsResponse,
    milestones: Vec<MilestoneProgressResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IntegrationAttemptResponse {
    id: String,
    task_id: String,
    source_branch: String,
    target_branch: String,
    status: String,
    queue_position: i64,
    merge_commit: Option<String>,
    error: Option<String>,
    created_at: String,
    started_at: Option<String>,
    completed_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IntegrationExecutionResponse {
    task: TaskResponse,
    attempt: IntegrationAttemptResponse,
    outcome: String,
    message: String,
    cleanup_error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RevertIntegrationInput {
    attempt_id: String,
    create_repair_task: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RevertAttemptResponse {
    id: String,
    project_id: String,
    original_task_id: String,
    integration_attempt_id: String,
    original_commit: String,
    status: String,
    revert_commit: Option<String>,
    repair_task_id: Option<String>,
    error: Option<String>,
    started_at: String,
    completed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateValidationCommandInput {
    project_id: String,
    stage: String,
    name: String,
    program: String,
    #[serde(default)]
    arguments: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidationCommandResponse {
    id: String,
    project_id: String,
    stage: String,
    name: String,
    program: String,
    arguments: Vec<String>,
    position: i64,
    enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidationEventResponse {
    id: i64,
    command_id: Option<String>,
    kind: String,
    message: String,
    stream: Option<String>,
    exit_code: Option<i32>,
    created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidationAttemptResponse {
    id: String,
    project_id: String,
    task_id: Option<String>,
    integration_attempt_id: Option<String>,
    stage: String,
    status: String,
    error: Option<String>,
    started_at: String,
    completed_at: Option<String>,
    events: Vec<ValidationEventResponse>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectHealthResponse {
    project_id: String,
    status: String,
    last_validation_attempt_id: Option<String>,
    last_successful_validation_at: Option<String>,
    last_integration_at: Option<String>,
    failing_gate: Option<String>,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidationRunEvent {
    validation_attempt_id: String,
    kind: String,
    command_id: Option<String>,
    stream: Option<OutputStream>,
    text: String,
    exit_code: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentResponse {
    id: String,
    name: String,
    provider: String,
    role: String,
    model: Option<String>,
    system_prompt: Option<String>,
    skills: Vec<String>,
    max_concurrent_tasks: i64,
    created_at: String,
    updated_at: String,
}

impl From<Project> for ProjectResponse {
    fn from(project: Project) -> Self {
        Self {
            id: project.id,
            name: project.name,
            description: project.description,
            default_branch: project.default_branch,
            created_at: project.created_at,
            updated_at: project.updated_at,
            workspaces: project.workspaces.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<Workspace> for WorkspaceResponse {
    fn from(workspace: Workspace) -> Self {
        Self {
            id: workspace.id,
            project_id: workspace.project_id,
            worker_id: workspace.worker_id,
            path: normalize_workspace_path(&workspace.path),
            created_at: workspace.created_at,
            updated_at: workspace.updated_at,
        }
    }
}

impl From<Task> for TaskResponse {
    fn from(task: Task) -> Self {
        Self {
            id: task.id,
            project_id: task.project_id,
            title: task.title,
            description: task.description,
            acceptance_criteria: task.acceptance_criteria,
            implementation_notes: task.implementation_notes,
            relevant_paths: task.relevant_paths,
            dependency_ids: task.dependency_ids,
            assigned_agent_id: task.assigned_agent_id,
            branch: task.branch,
            worktree_path: task
                .worktree_path
                .map(|path| normalize_workspace_path(&path)),
            priority: task.priority.as_str().to_owned(),
            blocked_reason: task.blocked_reason,
            milestone_id: task.milestone_id,
            epic_id: task.epic_id,
            status: task.status.as_str().to_owned(),
            position: task.position,
            created_at: task.created_at,
            updated_at: task.updated_at,
        }
    }
}

impl From<Milestone> for MilestoneResponse {
    fn from(value: Milestone) -> Self {
        Self {
            id: value.id,
            project_id: value.project_id,
            title: value.title,
            description: value.description,
            status: value.status,
            target_date: value.target_date,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<Epic> for EpicResponse {
    fn from(value: Epic) -> Self {
        Self {
            id: value.id,
            project_id: value.project_id,
            milestone_id: value.milestone_id,
            title: value.title,
            description: value.description,
            status: value.status,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<orchestr_db::TaskProgressCounts> for TaskProgressCountsResponse {
    fn from(value: orchestr_db::TaskProgressCounts) -> Self {
        Self {
            total: value.total,
            backlog: value.backlog,
            ready: value.ready,
            in_progress: value.in_progress,
            review: value.review,
            blocked: value.blocked,
            done: value.done,
        }
    }
}

impl From<ProjectProgress> for ProjectProgressResponse {
    fn from(value: ProjectProgress) -> Self {
        Self {
            counts: value.counts.into(),
            milestones: value
                .milestones
                .into_iter()
                .map(|entry| MilestoneProgressResponse {
                    milestone: entry.milestone.into(),
                    counts: entry.counts.into(),
                    epics: entry.epics.into_iter().map(Into::into).collect(),
                })
                .collect(),
        }
    }
}

impl From<IntegrationAttempt> for IntegrationAttemptResponse {
    fn from(attempt: IntegrationAttempt) -> Self {
        Self {
            id: attempt.id,
            task_id: attempt.task_id,
            source_branch: attempt.source_branch,
            target_branch: attempt.target_branch,
            status: attempt.status.as_str().to_owned(),
            queue_position: attempt.queue_position,
            merge_commit: attempt.merge_commit,
            error: attempt.error,
            created_at: attempt.created_at,
            started_at: attempt.started_at,
            completed_at: attempt.completed_at,
        }
    }
}

impl From<RevertAttempt> for RevertAttemptResponse {
    fn from(attempt: RevertAttempt) -> Self {
        Self {
            id: attempt.id,
            project_id: attempt.project_id,
            original_task_id: attempt.original_task_id,
            integration_attempt_id: attempt.integration_attempt_id,
            original_commit: attempt.original_commit,
            status: attempt.status.as_str().to_owned(),
            revert_commit: attempt.revert_commit,
            repair_task_id: attempt.repair_task_id,
            error: attempt.error,
            started_at: attempt.started_at,
            completed_at: attempt.completed_at,
        }
    }
}

impl From<ValidationCommand> for ValidationCommandResponse {
    fn from(command: ValidationCommand) -> Self {
        Self {
            id: command.id,
            project_id: command.project_id,
            stage: command.stage.as_str().into(),
            name: command.name,
            program: command.program,
            arguments: command.arguments,
            position: command.position,
            enabled: command.enabled,
            created_at: command.created_at,
            updated_at: command.updated_at,
        }
    }
}

impl From<orchestr_db::ValidationEvent> for ValidationEventResponse {
    fn from(event: orchestr_db::ValidationEvent) -> Self {
        Self {
            id: event.id,
            command_id: event.command_id,
            kind: event.kind,
            message: event.message,
            stream: event.stream,
            exit_code: event.exit_code,
            created_at: event.created_at,
        }
    }
}

impl From<ValidationAttempt> for ValidationAttemptResponse {
    fn from(attempt: ValidationAttempt) -> Self {
        Self {
            id: attempt.id,
            project_id: attempt.project_id,
            task_id: attempt.task_id,
            integration_attempt_id: attempt.integration_attempt_id,
            stage: attempt.stage.as_str().into(),
            status: attempt.status.as_str().into(),
            error: attempt.error,
            started_at: attempt.started_at,
            completed_at: attempt.completed_at,
            events: attempt.events.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ProjectHealth> for ProjectHealthResponse {
    fn from(health: ProjectHealth) -> Self {
        Self {
            project_id: health.project_id,
            status: health.status.as_str().into(),
            last_validation_attempt_id: health.last_validation_attempt_id,
            last_successful_validation_at: health.last_successful_validation_at,
            last_integration_at: health.last_integration_at,
            failing_gate: health.failing_gate,
            updated_at: health.updated_at,
        }
    }
}

impl From<FlowLimits> for FlowLimitsResponse {
    fn from(limits: FlowLimits) -> Self {
        Self {
            project_id: limits.project_id,
            worker_id: limits.worker_id,
            worker_max_concurrent_runs: limits.worker_max_concurrent_runs,
            in_progress_limit: limits.in_progress_limit,
            review_limit: limits.review_limit,
            approved_limit: limits.approved_limit,
        }
    }
}

impl From<Agent> for AgentResponse {
    fn from(agent: Agent) -> Self {
        Self {
            id: agent.id,
            name: agent.name,
            provider: agent.provider,
            role: agent.role,
            model: agent.model,
            system_prompt: agent.system_prompt,
            skills: agent.skills,
            max_concurrent_tasks: agent.max_concurrent_tasks,
            created_at: agent.created_at,
            updated_at: agent.updated_at,
        }
    }
}

impl From<AgentReview> for AgentReviewResponse {
    fn from(review: AgentReview) -> Self {
        Self {
            id: review.id,
            task_id: review.task_id,
            agent_id: review.agent_id,
            status: review.status.as_str().into(),
            decision: review.decision.map(|decision| decision.as_str().into()),
            notes: review.notes,
            raw_output: review.raw_output,
            error: review.error,
            started_at: review.started_at,
            completed_at: review.completed_at,
        }
    }
}

impl From<RunOutput> for RunOutputResponse {
    fn from(output: RunOutput) -> Self {
        Self {
            stream: output.stream,
            text: output.text,
            created_at: output.created_at,
        }
    }
}

impl From<RunEvent> for RunEventResponse {
    fn from(event: RunEvent) -> Self {
        Self {
            id: event.id,
            kind: event.kind,
            message: event.message,
            command: event.command,
            file_path: event.file_path,
            exit_code: event.exit_code,
            created_at: event.created_at,
        }
    }
}

impl From<Run> for RunResponse {
    fn from(run: Run) -> Self {
        Self {
            id: run.id,
            task_id: run.task_id,
            agent_id: run.agent_id,
            worker_id: run.worker_id,
            status: run.status.as_str().to_owned(),
            started_at: run.started_at,
            completed_at: run.completed_at,
            exit_code: run.exit_code,
            error: run.error,
            output: run.output.into_iter().map(Into::into).collect(),
            events: run.events.into_iter().map(Into::into).collect(),
        }
    }
}

#[tauri::command]
fn get_setting(key: String, state: State<'_, AppState>) -> Result<Option<String>, String> {
    state
        .database
        .lock()
        .map_err(|_| "Local settings store is unavailable.".to_owned())?
        .get(&key)
        .map_err(|error| format!("Unable to read local setting: {error}"))
}

#[tauri::command]
fn set_setting(key: String, value: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .database
        .lock()
        .map_err(|_| "Local settings store is unavailable.".to_owned())?
        .set(&key, &value)
        .map_err(|error| format!("Unable to save local setting: {error}"))
}

#[tauri::command]
fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectResponse>, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .list_projects()
        .map(|projects| projects.into_iter().map(Into::into).collect())
        .map_err(|error| format!("Unable to load projects: {error}"))
}

#[tauri::command]
fn get_project(id: String, state: State<'_, AppState>) -> Result<Option<ProjectResponse>, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .get_project(&id)
        .map(|project| project.map(Into::into))
        .map_err(|error| format!("Unable to load the project: {error}"))
}

#[tauri::command]
fn delete_project(id: String, state: State<'_, AppState>) -> Result<(), String> {
    match state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .delete_project(&id)
        .map_err(|error| format!("Unable to remove the project: {error}"))?
    {
        ProjectDeletion::Deleted => Ok(()),
        ProjectDeletion::NotFound => Err("The project no longer exists.".into()),
        ProjectDeletion::HasAttachedWorktrees => {
            Err("Remove any task worktrees before deleting this project.".into())
        }
    }
}

#[tauri::command]
fn create_project(
    input: CreateProjectInput,
    state: State<'_, AppState>,
) -> Result<ProjectResponse, String> {
    let name = validate_project_name(&input.name)?;
    ensure_project_name_available(&state, &name)?;
    let directory = create_workspace_directory(&input.parent_path, &input.directory_name)?;
    let repository = GitService::initialize_repository(&directory)
        .map_err(|error| format!("Unable to initialize the Git repository: {error}"))?;
    let repository = GitService::create_initial_commit(Path::new(&repository.root_path))
        .map_err(|error| format!("Unable to create the initial Git commit: {error}"))?;

    save_project(
        &state,
        name,
        normalize_optional_text(input.description),
        repository.default_branch,
        repository.root_path,
    )
    .map_err(|error| {
        format!("The repository was initialized, but Orchestr could not save the project: {error}")
    })
}

#[tauri::command]
fn register_project(
    input: RegisterProjectInput,
    state: State<'_, AppState>,
) -> Result<ProjectResponse, String> {
    let name = validate_project_name(&input.name)?;
    ensure_project_name_available(&state, &name)?;
    let repository = GitService::inspect_repository(Path::new(&input.path)).map_err(|error| {
        format!("The selected directory is not a usable Git repository: {error}")
    })?;

    save_project(
        &state,
        name,
        normalize_optional_text(input.description),
        repository.default_branch,
        repository.root_path,
    )
}

#[tauri::command]
fn get_repository_details(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<RepositoryDetails, String> {
    let workspace_path = workspace_path_for_project(&state, &project_id)?;
    GitService::repository_details(Path::new(&workspace_path))
        .map_err(|error| format!("Unable to inspect the repository: {error}"))
}

#[tauri::command]
fn get_repository_diff(
    input: RepositoryDiffInput,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let workspace_path = workspace_path_for_project(&state, &input.project_id)?;
    GitService::file_diff(Path::new(&workspace_path), &input.file_path)
        .map_err(|error| format!("Unable to inspect the file diff: {error}"))
}

#[tauri::command]
fn get_repository_file_preview(
    input: RepositoryDiffInput,
    state: State<'_, AppState>,
) -> Result<Option<orchestr_git::FilePreview>, String> {
    let workspace_path = workspace_path_for_project(&state, &input.project_id)?;
    GitService::file_preview(Path::new(&workspace_path), &input.file_path)
        .map_err(|error| format!("Unable to preview the file: {error}"))
}

#[tauri::command]
fn get_local_worker_profile(state: State<'_, AppState>) -> Result<WorkerProfile, String> {
    let mut profile = LocalWorker::profile();
    if !state
        .local_worker_runs
        .lock()
        .map_err(|_| "The local worker state is unavailable.".to_owned())?
        .is_empty()
    {
        profile.status = "busy".into();
    }
    Ok(profile)
}

#[tauri::command]
fn run_local_diagnostic(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<StartedWorkerRun, String> {
    start_local_worker_run(
        app,
        &state,
        ProcessRequest {
            program: "git".into(),
            arguments: vec!["--version".into()],
            working_directory: None,
            standard_input: None,
        },
        "the local worker diagnostic",
    )
}

#[tauri::command]
fn get_codex_provider_status() -> Result<ProviderStatus, String> {
    CodexProvider
        .inspect()
        .map_err(|error| format!("Unable to inspect Codex: {error}"))
}

#[tauri::command]
fn start_codex_login(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<StartedWorkerRun, String> {
    start_local_worker_run(
        app,
        &state,
        CodexProvider.action_request(ProviderAction::Login),
        "the Codex sign-in flow",
    )
}

#[tauri::command]
fn logout_codex(app: AppHandle, state: State<'_, AppState>) -> Result<StartedWorkerRun, String> {
    start_local_worker_run(
        app,
        &state,
        CodexProvider.action_request(ProviderAction::Logout),
        "Codex sign-out",
    )
}

#[tauri::command]
fn test_codex_connection(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<StartedWorkerRun, String> {
    start_local_worker_run(
        app,
        &state,
        CodexProvider.action_request(ProviderAction::CheckConnection),
        "the Codex status check",
    )
}

fn start_local_worker_run(
    app: AppHandle,
    state: &State<'_, AppState>,
    request: ProcessRequest,
    operation: &str,
) -> Result<StartedWorkerRun, String> {
    let run = LocalWorker::start(request)
        .map_err(|error| format!("Unable to start {operation}: {error}"))?;
    let run_id = Uuid::new_v4().to_string();
    let handle = run.handle;
    let active_runs = Arc::clone(&state.local_worker_runs);
    active_runs
        .lock()
        .map_err(|_| "The local worker state is unavailable.".to_owned())?
        .insert(
            run_id.clone(),
            ActiveLocalRun {
                handle: handle.clone(),
                cancel_requested: false,
            },
        );

    let event_run_id = run_id.clone();
    thread::spawn(move || {
        for output in run.output {
            let _ = app.emit(
                "worker://run-event",
                WorkerRunEvent {
                    run_id: event_run_id.clone(),
                    kind: "output".into(),
                    stream: Some(output.stream),
                    text: Some(output.text),
                    raw_text: None,
                    command: None,
                    exit_code: None,
                },
            );
        }

        let result = handle.wait();
        let cancelled = active_runs
            .lock()
            .ok()
            .and_then(|mut runs| runs.remove(&event_run_id))
            .is_some_and(|run| run.cancel_requested);
        let (kind, text, exit_code) = match result {
            Ok(status) if cancelled => (
                "cancelled",
                Some("Command cancelled.".into()),
                status.code(),
            ),
            Ok(status) if status.success() => ("completed", None, status.code()),
            Ok(status) => (
                "failed",
                Some("Command exited with an error.".into()),
                status.code(),
            ),
            Err(error) => ("failed", Some(error.to_string()), None),
        };
        let _ = app.emit(
            "worker://run-event",
            WorkerRunEvent {
                run_id: event_run_id,
                kind: kind.into(),
                stream: None,
                text,
                raw_text: None,
                command: None,
                exit_code,
            },
        );
    });

    Ok(StartedWorkerRun { run_id })
}

#[tauri::command]
fn cancel_local_worker_run(run_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut active_runs = state
        .local_worker_runs
        .lock()
        .map_err(|_| "The local worker state is unavailable.".to_owned())?;
    let run = active_runs
        .get_mut(&run_id)
        .ok_or_else(|| "The worker command is no longer running.".to_owned())?;
    run.handle
        .cancel()
        .map_err(|error| format!("Unable to cancel the local worker command: {error}"))?;
    run.cancel_requested = true;
    Ok(())
}

#[tauri::command]
fn cancel_queued_task_run(run_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .database
        .lock()
        .map_err(|_| "The local run store is unavailable.".to_owned())?
        .cancel_queued_run(&run_id)
        .map_err(|error| format!("Unable to cancel the queued task: {error}"))?
        .then_some(())
        .ok_or_else(|| "The task run is no longer queued.".into())
}

#[tauri::command]
fn list_task_runs(task_id: String, state: State<'_, AppState>) -> Result<Vec<RunResponse>, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local run store is unavailable.".to_owned())?
        .list_runs_for_task(&task_id)
        .map(|runs| runs.into_iter().map(Into::into).collect())
        .map_err(|error| format!("Unable to load task runs: {error}"))
}

#[tauri::command]
fn recover_task_run(
    input: RecoverTaskRunInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<StartedTaskRunResponse, String> {
    let prepared = prepare_run_recovery(&input, &state)?;
    let queued = queue_prepared_run_recovery(&input, &state, prepared)?;
    dispatch_and_reload_recovery(app, &state, queued)
}

struct PreparedRunRecovery {
    source_run: Run,
}

fn prepare_run_recovery(
    input: &RecoverTaskRunInput,
    state: &AppState,
) -> Result<PreparedRunRecovery, String> {
    validate_run_recovery_mode(&input.mode)?;
    let context = load_run_recovery_context(state, &input.run_id)?;
    prepare_recovery_context(input, state, context)
}

fn prepare_recovery_context(
    input: &RecoverTaskRunInput,
    state: &AppState,
    context: (Run, Task, String),
) -> Result<PreparedRunRecovery, String> {
    let (source_run, task, workspace_path) = context;
    if input.mode == "restart_clean" {
        reset_failed_task_worktree(state, &task, &workspace_path)?;
    }
    Ok(PreparedRunRecovery { source_run })
}

fn validate_run_recovery_mode(mode: &str) -> Result<(), String> {
    if matches!(mode, "resume" | "restart_clean") {
        Ok(())
    } else {
        Err("Choose resume or restart clean for run recovery.".into())
    }
}

fn load_run_recovery_context(
    state: &AppState,
    run_id: &str,
) -> Result<(Run, Task, String), String> {
    let database = state
        .database
        .lock()
        .map_err(|_| "The local run store is unavailable.".to_owned())?;
    load_run_recovery_entities(&database, run_id)
}

fn load_run_recovery_entities(
    database: &Database,
    run_id: &str,
) -> Result<(Run, Task, String), String> {
    let run = required_recovery_run(&database, run_id)?;
    let task = required_recovery_task(&database, &run.task_id)?;
    let workspace = required_recovery_workspace(&database, &task.project_id)?;
    Ok((run, task, workspace))
}

fn required_recovery_run(database: &Database, run_id: &str) -> Result<Run, String> {
    database
        .get_run(run_id)
        .map_err(|error| format!("Unable to load the failed run: {error}"))?
        .ok_or_else(|| "The failed run no longer exists.".to_owned())
}

fn required_recovery_task(database: &Database, task_id: &str) -> Result<Task, String> {
    database
        .get_task(task_id)
        .map_err(|error| format!("Unable to load the failed task: {error}"))?
        .ok_or_else(|| "The failed task no longer exists.".to_owned())
}

fn required_recovery_workspace(database: &Database, project_id: &str) -> Result<String, String> {
    database
        .get_project(project_id)
        .map_err(|error| format!("Unable to load the recovery workspace: {error}"))?
        .and_then(|project| {
            project
                .workspaces
                .into_iter()
                .find(|workspace| workspace.worker_id == LOCAL_WORKER_ID)
                .map(|workspace| workspace.path)
        })
        .ok_or_else(|| "This project has no local recovery workspace.".to_owned())
}

fn queue_prepared_run_recovery(
    input: &RecoverTaskRunInput,
    state: &AppState,
    prepared: PreparedRunRecovery,
) -> Result<(String, Run, Task), String> {
    let (agent_id, action) = recovery_assignment(input, &prepared.source_run);
    let run_id = Uuid::new_v4().to_string();
    let queued = persist_run_recovery(state, input, &run_id, agent_id, action)?;
    Ok((run_id, queued.0, queued.1))
}

fn recovery_assignment<'a>(
    input: &'a RecoverTaskRunInput,
    source_run: &'a Run,
) -> (&'a str, &'a str) {
    let agent_id = input.agent_id.as_deref().unwrap_or(&source_run.agent_id);
    let action = if agent_id == source_run.agent_id {
        input.mode.as_str()
    } else {
        "reassign"
    };
    (agent_id, action)
}

fn persist_run_recovery(
    state: &AppState,
    input: &RecoverTaskRunInput,
    run_id: &str,
    agent_id: &str,
    action: &str,
) -> Result<(Run, Task), String> {
    state.database.lock().map_err(|_| "The local run store is unavailable.".to_owned())?
        .queue_run_recovery(&input.run_id, run_id, &Uuid::new_v4().to_string(), agent_id, action)
        .map_err(|_| "Only failed or cancelled In Progress runs can be recovered, and the selected Codex agent must be available.".to_owned())?
        .ok_or_else(|| "The failed run no longer exists.".to_owned())
}

fn dispatch_and_reload_recovery(
    app: AppHandle,
    state: &AppState,
    queued: (String, Run, Task),
) -> Result<StartedTaskRunResponse, String> {
    dispatch_queued_task_runs(
        app,
        Arc::clone(&state.database),
        Arc::clone(&state.local_worker_runs),
    )?;
    reload_recovery_response(&state.database, queued)
}

fn reload_recovery_response(
    database: &Arc<Mutex<Database>>,
    queued: (String, Run, Task),
) -> Result<StartedTaskRunResponse, String> {
    let store = database
        .lock()
        .map_err(|_| "The local run store is unavailable.".to_owned())?;
    let run = store
        .get_run(&queued.0)
        .map_err(|error| format!("Unable to reload the recovery run: {error}"))?
        .unwrap_or(queued.1);
    let task = store
        .get_task(&queued.2.id)
        .map_err(|error| format!("Unable to reload the recovered task: {error}"))?
        .unwrap_or(queued.2);
    Ok(StartedTaskRunResponse {
        run: run.into(),
        task: task.into(),
    })
}

fn reset_failed_task_worktree(
    state: &AppState,
    task: &Task,
    workspace_path: &str,
) -> Result<(), String> {
    reset_failed_worktree_record(state, task, workspace_path)?;
    reset_failed_task_branch(task, workspace_path)
}

fn reset_failed_worktree_record(
    state: &AppState,
    task: &Task,
    workspace_path: &str,
) -> Result<(), String> {
    if let Some(recorded_path) = task.worktree_path.as_deref() {
        remove_failed_worktree(task, workspace_path, recorded_path)?;
        release_failed_worktree_record(state, &task.id)?;
    }
    Ok(())
}

fn remove_failed_worktree(
    task: &Task,
    workspace_path: &str,
    recorded_path: &str,
) -> Result<(), String> {
    let recorded = PathBuf::from(recorded_path);
    if recorded.exists() {
        remove_existing_failed_worktree(task, workspace_path, recorded_path)
    } else {
        validate_missing_failed_worktree(task, workspace_path, recorded_path)
    }
}

fn remove_existing_failed_worktree(
    task: &Task,
    workspace_path: &str,
    recorded_path: &str,
) -> Result<(), String> {
    let actual = canonical_recovery_worktree(task, workspace_path, recorded_path)?;
    GitService::remove_task_worktree(Path::new(workspace_path), &actual)
        .map_err(|error| format!("Unable to remove the failed task worktree: {error}"))
}

fn validate_missing_failed_worktree(
    task: &Task,
    workspace_path: &str,
    recorded_path: &str,
) -> Result<(), String> {
    let expected = task_worktree_path(workspace_path, &task.project_id, &task.id)?;
    if PathBuf::from(normalize_workspace_path(recorded_path))
        == PathBuf::from(normalize_workspace_path(&expected.to_string_lossy()))
    {
        Ok(())
    } else {
        Err("The failed task worktree is outside Orchestr's managed location.".into())
    }
}

fn release_failed_worktree_record(state: &AppState, task_id: &str) -> Result<(), String> {
    state
        .database
        .lock()
        .map_err(|_| "The local run store is unavailable.".to_owned())?
        .release_task_worktree(task_id)
        .map_err(|error| format!("Unable to release the failed task worktree: {error}"))?;
    Ok(())
}

fn reset_failed_task_branch(task: &Task, workspace_path: &str) -> Result<(), String> {
    if let Some(branch) = task.branch.as_deref() {
        GitService::delete_task_branch_if_exists(Path::new(workspace_path), branch)
            .map_err(|error| format!("Unable to reset the failed task branch: {error}"))?;
    }
    Ok(())
}

#[tauri::command]
fn resolve_failed_run(
    input: ResolveFailedRunInput,
    state: State<'_, AppState>,
) -> Result<TaskResponse, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local run store is unavailable.".to_owned())?
        .resolve_failed_run(
            &input.run_id,
            &Uuid::new_v4().to_string(),
            &input.action,
            input.note.as_deref(),
        )
        .map_err(|_| {
            "Only failed or cancelled In Progress runs can be abandoned or escalated.".to_owned()
        })?
        .map(Into::into)
        .ok_or_else(|| "The failed run no longer exists.".into())
}

fn load_flow_state(database: &Database, project_id: &str) -> Result<FlowStateResponse, String> {
    let FlowState {
        limits,
        active_worker_runs,
        in_progress,
        review,
        approved,
        integrating,
        queued,
        blocked_reason,
    } = database
        .flow_state(project_id, LOCAL_WORKER_ID)
        .map_err(|error| format!("Unable to load flow control: {error}"))?;
    let queue = database
        .list_queued_runs(project_id)
        .map_err(|error| format!("Unable to load the execution queue: {error}"))?
        .into_iter()
        .map(Into::into)
        .collect();
    Ok(FlowStateResponse {
        limits: limits.into(),
        active_worker_runs,
        in_progress,
        review,
        approved,
        integrating,
        queued,
        blocked_reason,
        queue,
    })
}

#[tauri::command]
fn get_flow_state(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<FlowStateResponse, String> {
    let database = state
        .database
        .lock()
        .map_err(|_| "The local run store is unavailable.".to_owned())?;
    load_flow_state(&database, &project_id)
}

#[tauri::command]
fn update_flow_limits(
    input: UpdateFlowLimitsInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<FlowStateResponse, String> {
    save_flow_limit_update(&state.database, &input)?;
    let _ = dispatch_queued_task_runs(
        app,
        Arc::clone(&state.database),
        Arc::clone(&state.local_worker_runs),
    );
    load_flow_state_from_store(&state.database, &input.project_id)
}

fn save_flow_limit_update(
    database: &Arc<Mutex<Database>>,
    input: &UpdateFlowLimitsInput,
) -> Result<(), String> {
    database
        .lock()
        .map_err(|_| "The local run store is unavailable.".to_owned())?
        .update_flow_limits(
            &input.project_id,
            LOCAL_WORKER_ID,
            FlowLimitUpdate {
                worker_max_concurrent_runs: input.worker_max_concurrent_runs,
                in_progress_limit: input.in_progress_limit,
                review_limit: input.review_limit,
                approved_limit: input.approved_limit,
            },
        )
        .map(|_| ())
        .map_err(|error| format!("Unable to update flow limits: {error}"))
}

fn load_flow_state_from_store(
    database: &Arc<Mutex<Database>>,
    project_id: &str,
) -> Result<FlowStateResponse, String> {
    let database = database
        .lock()
        .map_err(|_| "The local run store is unavailable.".to_owned())?;
    load_flow_state(&database, project_id)
}

#[tauri::command]
fn export_task_run_log(
    run_id: String,
    destination_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let run = state
        .database
        .lock()
        .map_err(|_| "The local run store is unavailable.".to_owned())?
        .get_run(&run_id)
        .map_err(|error| format!("Unable to load the execution log: {error}"))?
        .ok_or_else(|| "The execution run no longer exists.".to_owned())?;
    let destination = PathBuf::from(destination_path);
    if destination.is_dir() {
        return Err("Choose a file destination for the execution log.".into());
    }
    let parent = destination
        .parent()
        .filter(|path| path.is_dir())
        .ok_or_else(|| "The selected log destination folder no longer exists.".to_owned())?;
    if parent.as_os_str().is_empty() {
        return Err("Choose a file destination for the execution log.".into());
    }
    fs::write(&destination, format_run_log(&run))
        .map_err(|error| format!("Unable to export the execution log: {error}"))
}

fn format_run_log(run: &Run) -> String {
    let mut output = format!(
        "Orchestr execution log\nRun: {}\nTask: {}\nAgent: {}\nWorker: {}\nStatus: {}\nStarted: {}\nCompleted: {}\nExit code: {}\nError: {}\n\n=== Raw process output (ANSI control codes removed) ===\n",
        run.id,
        run.task_id,
        run.agent_id,
        run.worker_id,
        run.status.as_str(),
        run.started_at,
        run.completed_at.as_deref().unwrap_or("still running"),
        run.exit_code.map(|code| code.to_string()).as_deref().unwrap_or("not available"),
        run.error.as_deref().unwrap_or("none"),
    );
    if run.output.is_empty() {
        output.push_str("No raw process output was persisted for this run.\n");
    } else {
        for entry in &run.output {
            output.push_str(&format!(
                "[{}] [{}] {}\n",
                entry.created_at, entry.stream, entry.text
            ));
        }
    }
    output.push_str("\n=== Orchestr event timeline ===\n");
    for event in &run.events {
        output.push_str(&format!(
            "[{}] [{}] {}\n",
            event.created_at, event.kind, event.message
        ));
        if let Some(command) = &event.command {
            output.push_str(&format!("  command: {command}\n"));
        }
        if let Some(file_path) = &event.file_path {
            output.push_str(&format!("  file: {file_path}\n"));
        }
        if let Some(exit_code) = event.exit_code {
            output.push_str(&format!("  exit: {exit_code}\n"));
        }
    }
    output
}

#[tauri::command]
fn get_task_review(
    task_id: String,
    state: State<'_, AppState>,
) -> Result<orchestr_git::TaskReview, String> {
    let (task, project) = {
        let database = state
            .database
            .lock()
            .map_err(|_| "The local project store is unavailable.".to_owned())?;
        let task = database
            .get_task(&task_id)
            .map_err(|error| format!("Unable to load the task review: {error}"))?
            .ok_or_else(|| "The task no longer exists.".to_owned())?;
        let project = database
            .get_project(&task.project_id)
            .map_err(|error| format!("Unable to load the project review settings: {error}"))?
            .ok_or_else(|| "The project no longer exists.".to_owned())?;
        (task, project)
    };
    let worktree_path = task
        .worktree_path
        .ok_or_else(|| "This task has no isolated worktree to review.".to_owned())?;
    GitService::task_review(Path::new(&worktree_path), &project.default_branch)
        .map_err(|error| format!("Unable to inspect the task branch: {error}"))
}

#[tauri::command]
fn list_agent_reviews(
    task_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<AgentReviewResponse>, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local review store is unavailable.".to_owned())?
        .list_agent_reviews(&task_id)
        .map(|reviews| reviews.into_iter().map(Into::into).collect())
        .map_err(|error| format!("Unable to load agent reviews: {error}"))
}

#[tauri::command]
fn start_agent_review(
    input: StartAgentReviewInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AgentReviewResponse, String> {
    let (task, reviewer, default_branch, recent_runs, validation_attempts) = {
        let database = state
            .database
            .lock()
            .map_err(|_| "The local review store is unavailable.".to_owned())?;
        let task = database
            .get_task(&input.task_id)
            .map_err(|error| format!("Unable to load task for review: {error}"))?
            .ok_or_else(|| "The task no longer exists.".to_owned())?;
        if task.status != TaskStatus::Review {
            return Err("Only Review tasks can be evaluated by an architect agent.".into());
        }
        if task.assigned_agent_id.as_deref() == Some(input.agent_id.as_str()) {
            return Err("An implementation agent cannot review its own task.".into());
        }
        let reviewer = database
            .get_agent(&input.agent_id)
            .map_err(|error| format!("Unable to load reviewer agent: {error}"))?
            .ok_or_else(|| "The selected reviewer agent no longer exists.".to_owned())?;
        if reviewer.provider != "codex" {
            return Err(
                "Only Codex agents can perform local architect reviews at this stage.".into(),
            );
        }
        let project = database
            .get_project(&task.project_id)
            .map_err(|error| format!("Unable to load the project for review: {error}"))?
            .ok_or_else(|| "The task project no longer exists.".to_owned())?;
        let recent_runs = database
            .list_runs_for_task(&task.id)
            .map_err(|error| format!("Unable to load the implementation run summary: {error}"))?;
        let validation_attempts = database
            .list_validation_attempts(&task.project_id, 20)
            .map_err(|error| format!("Unable to load implementation validation: {error}"))?
            .into_iter()
            .filter(|attempt| attempt.task_id.as_deref() == Some(task.id.as_str()))
            .collect::<Vec<_>>();
        (
            task,
            reviewer,
            project.default_branch,
            recent_runs,
            validation_attempts,
        )
    };
    let worktree_path = task
        .worktree_path
        .clone()
        .ok_or_else(|| "This task has no isolated worktree to review.".to_owned())?;
    let task_review = GitService::task_review(Path::new(&worktree_path), &default_branch)
        .map_err(|error| format!("Unable to inspect the task branch: {error}"))?;
    let provider_status = CodexProvider
        .inspect()
        .map_err(|error| format!("Unable to inspect Codex before starting the review: {error}"))?;
    if !matches!(provider_status.readiness, ProviderReadiness::Ready) {
        return Err(format!(
            "Codex is not ready to review this task. {}",
            provider_status.detail
        ));
    }
    let review_id = Uuid::new_v4().to_string();
    let persisted_review = state
        .database
        .lock()
        .map_err(|_| "The local review store is unavailable.".to_owned())?
        .start_agent_review(NewAgentReview {
            id: review_id.clone(),
            task_id: task.id.clone(),
            agent_id: reviewer.id.clone(),
        })
        .map_err(|error| format!("Unable to start the architect review: {error}"))?;
    let request = match CodexProvider.execution_request(AgentRunInput {
        model: reviewer.model.clone(),
        prompt: build_agent_review_prompt(
            &task,
            &reviewer,
            &task_review,
            &recent_runs,
            &validation_attempts,
        ),
        working_directory: PathBuf::from(&worktree_path),
        additional_writable_directories: Vec::new(),
        read_only: true,
    }) {
        Ok(request) => request,
        Err(error) => {
            let _ = state.database.lock().ok().and_then(|mut database| {
                database
                    .finish_agent_review(
                        &review_id,
                        AgentReviewStatus::Failed,
                        None,
                        None,
                        Some(&error.to_string()),
                    )
                    .ok()
            });
            return Err(format!("Unable to prepare the architect review: {error}"));
        }
    };
    let run = match LocalWorker::start(request) {
        Ok(run) => run,
        Err(error) => {
            let _ = state.database.lock().ok().and_then(|mut database| {
                database
                    .finish_agent_review(
                        &review_id,
                        AgentReviewStatus::Failed,
                        None,
                        None,
                        Some(&error.to_string()),
                    )
                    .ok()
            });
            return Err(format!(
                "Unable to start Codex for architect review: {error}"
            ));
        }
    };
    let handle = run.handle;
    let active_runs = Arc::clone(&state.local_worker_runs);
    active_runs
        .lock()
        .map_err(|_| "The local worker state is unavailable.".to_owned())?
        .insert(
            review_id.clone(),
            ActiveLocalRun {
                handle: handle.clone(),
                cancel_requested: false,
            },
        );
    let database = Arc::clone(&state.database);
    let event_review_id = review_id.clone();
    let task_id = task.id.clone();
    thread::spawn(move || {
        for output in run.output {
            if let Ok(mut database) = database.lock() {
                let _ = database.append_agent_review_output(&event_review_id, &output.text);
                let _ = database.append_agent_review_output(&event_review_id, "\n");
            }
            let _ = app.emit("agent-review://event", event_review_id.clone());
        }
        let result = handle.wait();
        let cancelled = active_runs
            .lock()
            .ok()
            .and_then(|mut runs| runs.remove(&event_review_id))
            .is_some_and(|run| run.cancel_requested);
        let finish = |status, decision, notes: Option<String>, error: Option<String>| {
            if let Ok(mut database) = database.lock() {
                let _ = database.finish_agent_review(
                    &event_review_id,
                    status,
                    decision,
                    notes.as_deref(),
                    error.as_deref(),
                );
                if status == AgentReviewStatus::Completed {
                    let transition = match decision {
                        Some(AgentReviewDecision::Approve) => {
                            database.approve_task_review(&task_id, &Uuid::new_v4().to_string())
                        }
                        Some(AgentReviewDecision::RequestChanges) => {
                            database.request_task_changes(&task_id)
                        }
                        None => Ok(None),
                    };
                    let _ = transition;
                }
            }
        };
        match result {
            Ok(_exit_status) if cancelled => finish(
                AgentReviewStatus::Cancelled,
                None,
                None,
                Some("Architect review cancelled.".into()),
            ),
            Ok(exit_status) if exit_status.success() => {
                let output = database
                    .lock()
                    .ok()
                    .and_then(|database| {
                        database
                            .get_agent_review(&event_review_id)
                            .ok()
                            .flatten()
                            .map(|review| review.raw_output)
                    })
                    .unwrap_or_default();
                match parse_agent_review_decision(&output) {
                    Some((decision, notes)) => finish(
                        AgentReviewStatus::Completed,
                        Some(decision),
                        Some(notes),
                        None,
                    ),
                    None => finish(
                        AgentReviewStatus::Failed,
                        None,
                        None,
                        Some("Codex did not return the required review decision format.".into()),
                    ),
                }
            }
            Ok(_) => finish(
                AgentReviewStatus::Failed,
                None,
                None,
                Some("Codex exited with an error while reviewing the task.".into()),
            ),
            Err(error) => finish(
                AgentReviewStatus::Failed,
                None,
                None,
                Some(error.to_string()),
            ),
        }
        let _ = app.emit("agent-review://event", event_review_id);
        let _ = dispatch_queued_task_runs(app, database, active_runs);
    });
    Ok(persisted_review.into())
}

#[tauri::command]
fn approve_task_review(
    task_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<TaskResponse, String> {
    let worktree_path = {
        let database = state
            .database
            .lock()
            .map_err(|_| "The local project store is unavailable.".to_owned())?;
        let task = database
            .get_task(&task_id)
            .map_err(|error| format!("Unable to load the task for approval: {error}"))?
            .ok_or_else(|| "The task no longer exists.".to_owned())?;
        if task.status != TaskStatus::Review {
            return Err("Only Review tasks can be approved for integration.".into());
        }
        task.worktree_path
            .ok_or_else(|| "This task has no isolated worktree to approve.".to_owned())?
    };
    let repository = GitService::inspect_repository(Path::new(&worktree_path))
        .map_err(|error| format!("Unable to verify the task worktree before approval: {error}"))?;
    if !repository.is_clean {
        return Err(
            "The task worktree has uncommitted changes. Commit or discard them before approval."
                .into(),
        );
    }

    let result = state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .approve_task_review(&task_id, &Uuid::new_v4().to_string())
        .map_err(|_| "Only Review tasks with an isolated branch can be approved.".to_owned())?
        .map(Into::into)
        .ok_or_else(|| "The task no longer exists.".into());
    let _ = dispatch_queued_task_runs(
        app,
        Arc::clone(&state.database),
        Arc::clone(&state.local_worker_runs),
    );
    result
}

#[tauri::command]
fn request_task_changes(
    task_id: String,
    state: State<'_, AppState>,
) -> Result<TaskResponse, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .request_task_changes(&task_id)
        .map_err(|_| "Only tasks in Review can be returned for changes.".to_owned())?
        .map(Into::into)
        .ok_or_else(|| "The task no longer exists.".into())
}

#[tauri::command]
fn list_integration_attempts(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<IntegrationAttemptResponse>, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .list_integration_attempts(&project_id)
        .map(|attempts| attempts.into_iter().map(Into::into).collect())
        .map_err(|error| format!("Unable to load the integration queue: {error}"))
}

#[tauri::command]
fn retry_integration_attempt(
    attempt_id: String,
    state: State<'_, AppState>,
) -> Result<TaskResponse, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .retry_integration(&attempt_id, &Uuid::new_v4().to_string())
        .map_err(|_| "Only failed or conflicted integrations can be retried.".to_owned())?
        .map(Into::into)
        .ok_or_else(|| "The integration attempt no longer exists.".into())
}

#[tauri::command]
fn retry_integration_cleanup(
    attempt_id: String,
    state: State<'_, AppState>,
) -> Result<IntegrationAttemptResponse, String> {
    let attempt = integration_attempt(&state, &attempt_id)?;
    let (task, workspace_path) = retry_cleanup_context(&state, &attempt)?;
    execute_cleanup_retry(&state, &attempt, &task, &workspace_path)
}

fn retry_cleanup_context(
    state: &AppState,
    attempt: &IntegrationAttempt,
) -> Result<(Task, String), String> {
    if attempt.status != orchestr_db::IntegrationStatus::Merged || attempt.error.is_none() {
        return Err(
            "Only merged integrations with a recorded cleanup failure can retry cleanup.".into(),
        );
    }
    integration_context(state, attempt)
}

fn execute_cleanup_retry(
    state: &AppState,
    attempt: &IntegrationAttempt,
    task: &Task,
    workspace_path: &str,
) -> Result<IntegrationAttemptResponse, String> {
    cleanup_integrated_task(
        state,
        &attempt.id,
        task,
        workspace_path,
        &attempt.source_branch,
    )?;
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .clear_integration_cleanup_error(&attempt.id)
        .map_err(|error| format!("Unable to clear the cleanup failure: {error}"))?;
    integration_attempt(state, &attempt.id).map(Into::into)
}

#[tauri::command]
fn list_revert_attempts(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<RevertAttemptResponse>, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .list_revert_attempts(&project_id)
        .map(|attempts| attempts.into_iter().map(Into::into).collect())
        .map_err(|error| format!("Unable to load revert history: {error}"))
}

#[tauri::command]
fn revert_integration(
    input: RevertIntegrationInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RevertAttemptResponse, String> {
    run_revert_integration(&state, &app, &input)
}

fn run_revert_integration(
    state: &AppState,
    app: &AppHandle,
    input: &RevertIntegrationInput,
) -> Result<RevertAttemptResponse, String> {
    let context = prepare_revert_execution(state, &input.attempt_id)?;
    let outcome = execute_revert_commit(state, &context)?;
    finish_revert_command(state, app, input, context, outcome)
}

fn finish_revert_command(
    state: &AppState,
    app: &AppHandle,
    input: &RevertIntegrationInput,
    context: RevertExecutionContext,
    outcome: RevertCommitOutcome,
) -> Result<RevertAttemptResponse, String> {
    match outcome {
        RevertCommitOutcome::Committed(commit) => {
            complete_revert_execution(state, app, input, context, commit)
        }
        RevertCommitOutcome::Failed(response) => Ok(response),
    }
}

struct RevertExecutionContext {
    revert: RevertAttempt,
    project: Project,
    workspace_path: String,
    original_task: Task,
}
enum RevertCommitOutcome {
    Committed(String),
    Failed(RevertAttemptResponse),
}
struct RevertValidationOutcome {
    status: RevertStatus,
    error: Option<String>,
    validation_id: Option<String>,
}

fn prepare_revert_execution(
    state: &AppState,
    attempt_id: &str,
) -> Result<RevertExecutionContext, String> {
    let revert = begin_revert_attempt(state, attempt_id)?;
    let (project, original_task, workspace_path) = load_revert_resources(state, &revert)?;
    Ok(RevertExecutionContext {
        revert,
        project,
        workspace_path,
        original_task,
    })
}

fn begin_revert_attempt(state: &AppState, attempt_id: &str) -> Result<RevertAttempt, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .begin_revert(&Uuid::new_v4().to_string(), attempt_id)
        .map_err(|_| {
            "Only a merged integration that has not already been reverted can be reverted."
                .to_owned()
        })?
        .ok_or_else(|| "The merged integration no longer exists.".to_owned())
}

fn load_revert_resources(
    state: &AppState,
    revert: &RevertAttempt,
) -> Result<(Project, Task, String), String> {
    let database = state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?;
    let (project, task) = load_revert_project_and_task(&database, revert)?;
    let workspace = local_project_workspace(&project)?;
    Ok((project, task, workspace))
}

fn load_revert_project_and_task(
    database: &Database,
    revert: &RevertAttempt,
) -> Result<(Project, Task), String> {
    let project = required_revert_project(database, &revert.project_id)?;
    let task = required_reverted_task(database, &revert.original_task_id)?;
    Ok((project, task))
}

fn required_revert_project(database: &Database, project_id: &str) -> Result<Project, String> {
    let project = database
        .get_project(project_id)
        .map_err(|error| format!("Unable to load the revert project: {error}"))?
        .ok_or_else(|| "The revert project no longer exists.".to_owned())?;
    Ok(project)
}

fn required_reverted_task(database: &Database, task_id: &str) -> Result<Task, String> {
    let task = database
        .get_task(task_id)
        .map_err(|error| format!("Unable to load the reverted task: {error}"))?
        .ok_or_else(|| "The reverted task no longer exists.".to_owned())?;
    Ok(task)
}

fn local_project_workspace(project: &Project) -> Result<String, String> {
    project
        .workspaces
        .iter()
        .find(|workspace| workspace.worker_id == LOCAL_WORKER_ID)
        .map(|workspace| workspace.path.clone())
        .ok_or_else(|| "This project has no local integration workspace.".to_owned())
}

fn execute_revert_commit(
    state: &AppState,
    context: &RevertExecutionContext,
) -> Result<RevertCommitOutcome, String> {
    match GitService::revert_integration_commit(
        Path::new(&context.workspace_path),
        &context.project.default_branch,
        &context.revert.original_commit,
    ) {
        Ok(commit) => Ok(RevertCommitOutcome::Committed(commit)),
        Err(error) => record_failed_revert(state, &context.revert.id, &error.to_string())
            .map(RevertCommitOutcome::Failed),
    }
}

fn record_failed_revert(
    state: &AppState,
    revert_id: &str,
    error: &str,
) -> Result<RevertAttemptResponse, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .finish_revert(revert_id, RevertStatus::Failed, None, Some(error), None)
        .map_err(|database_error| format!("Unable to record the failed revert: {database_error}"))?
        .map(Into::into)
        .ok_or_else(|| "The revert attempt is no longer active.".to_owned())
}

fn complete_revert_execution(
    state: &AppState,
    app: &AppHandle,
    input: &RevertIntegrationInput,
    context: RevertExecutionContext,
    commit: String,
) -> Result<RevertAttemptResponse, String> {
    let mut validation = validate_reverted_project(state, app, &context);
    let health_error = record_revert_health(state, &context.project.id, &validation).err();
    let repair = create_optional_repair_task(state, input, &context, &commit);
    validation.error = combine_revert_errors(validation.error, health_error);
    validation.error = combine_revert_errors(validation.error, repair.error);
    finish_completed_revert(
        state,
        &context.revert.id,
        &commit,
        validation,
        repair.task_id,
    )
}

fn validate_reverted_project(
    state: &AppState,
    app: &AppHandle,
    context: &RevertExecutionContext,
) -> RevertValidationOutcome {
    match run_validation(
        &state.database,
        app,
        &context.project.id,
        None,
        None,
        ValidationStage::Integration,
        Path::new(&context.workspace_path),
        true,
    ) {
        Ok(validation) if validation.status == ValidationStatus::Passed => {
            RevertValidationOutcome {
                status: RevertStatus::Reverted,
                error: None,
                validation_id: Some(validation.id),
            }
        }
        Ok(validation) => RevertValidationOutcome {
            status: RevertStatus::ValidationFailed,
            error: validation
                .error
                .or_else(|| Some("The integration branch is unhealthy after revert.".into())),
            validation_id: Some(validation.id),
        },
        Err(error) => RevertValidationOutcome {
            status: RevertStatus::ValidationFailed,
            error: Some(error),
            validation_id: None,
        },
    }
}

fn record_revert_health(
    state: &AppState,
    project_id: &str,
    outcome: &RevertValidationOutcome,
) -> Result<(), String> {
    match outcome.validation_id.as_deref() {
        Some(validation_id) => {
            record_revert_validation_health(state, project_id, validation_id, outcome)
        }
        None => mark_revert_health_broken(
            state,
            project_id,
            outcome
                .error
                .as_deref()
                .unwrap_or("Revert validation could not run."),
        ),
    }
}

fn record_revert_validation_health(
    state: &AppState,
    project_id: &str,
    validation_id: &str,
    outcome: &RevertValidationOutcome,
) -> Result<(), String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .record_project_validation(
            project_id,
            validation_id,
            if outcome.status == RevertStatus::Reverted {
                ValidationStatus::Passed
            } else {
                ValidationStatus::Failed
            },
            outcome.error.as_deref(),
            false,
        )
        .map_err(|error| format!("Unable to update project health after revert: {error}"))
}

fn mark_revert_health_broken(
    state: &AppState,
    project_id: &str,
    error: &str,
) -> Result<(), String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .mark_project_health_broken(project_id, error)
        .map_err(|database_error| {
            format!("Unable to mark project health after revert: {database_error}")
        })
}

fn maybe_create_repair_task(
    state: &AppState,
    input: &RevertIntegrationInput,
    context: &RevertExecutionContext,
    revert_commit: &str,
) -> Result<Option<String>, String> {
    if !input.create_repair_task {
        return Ok(None);
    }
    create_revert_repair_task(state, input, context, revert_commit).map(Some)
}

struct RepairTaskOutcome {
    task_id: Option<String>,
    error: Option<String>,
}

fn create_optional_repair_task(
    state: &AppState,
    input: &RevertIntegrationInput,
    context: &RevertExecutionContext,
    revert_commit: &str,
) -> RepairTaskOutcome {
    match maybe_create_repair_task(state, input, context, revert_commit) {
        Ok(task_id) => RepairTaskOutcome {
            task_id,
            error: None,
        },
        Err(error) => RepairTaskOutcome {
            task_id: None,
            error: Some(error),
        },
    }
}

fn combine_revert_errors(primary: Option<String>, secondary: Option<String>) -> Option<String> {
    match (primary, secondary) {
        (Some(primary), Some(secondary)) => Some(format!("{primary} {secondary}")),
        (Some(error), None) | (None, Some(error)) => Some(error),
        (None, None) => None,
    }
}

fn create_revert_repair_task(
    state: &AppState,
    input: &RevertIntegrationInput,
    context: &RevertExecutionContext,
    revert_commit: &str,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .create_task(NewTask {
            id: id.clone(),
            project_id: context.project.id.clone(),
            title: format!("Repair reverted task: {}", context.original_task.title),
            description: Some(format!(
                "Follow-up for reverted integration {} (commit {}).",
                input.attempt_id, context.revert.original_commit
            )),
            acceptance_criteria: vec![
                "Restore the intended behavior without reintroducing the regression.".into(),
            ],
            implementation_notes: Some(format!(
                "Review revert commit {revert_commit} and the original task {}.",
                context.original_task.id
            )),
            relevant_paths: context.original_task.relevant_paths.clone(),
            dependency_ids: Vec::new(),
            assigned_agent_id: None,
            priority: TaskPriority::High,
            milestone_id: context.original_task.milestone_id.clone(),
            epic_id: context.original_task.epic_id.clone(),
        })
        .map_err(|error| {
            format!("The revert succeeded, but its repair task could not be created: {error}")
        })?;
    Ok(id)
}

fn finish_completed_revert(
    state: &AppState,
    revert_id: &str,
    commit: &str,
    outcome: RevertValidationOutcome,
    repair_task_id: Option<String>,
) -> Result<RevertAttemptResponse, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .finish_revert(
            revert_id,
            outcome.status,
            Some(commit),
            outcome.error.as_deref(),
            repair_task_id.as_deref(),
        )
        .map_err(|error| format!("Unable to record the completed revert: {error}"))?
        .map(Into::into)
        .ok_or_else(|| "The revert attempt is no longer active.".to_owned())
}

#[tauri::command]
fn list_validation_commands(
    project_id: String,
    stage: String,
    state: State<'_, AppState>,
) -> Result<Vec<ValidationCommandResponse>, String> {
    let stage = parse_validation_stage(&stage)?;
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .list_validation_commands(&project_id, stage)
        .map(|commands| commands.into_iter().map(Into::into).collect())
        .map_err(|error| format!("Unable to load validation commands: {error}"))
}

#[tauri::command]
fn create_validation_command(
    input: CreateValidationCommandInput,
    state: State<'_, AppState>,
) -> Result<ValidationCommandResponse, String> {
    let stage = parse_validation_stage(&input.stage)?;
    let name = input.name.trim();
    let program = input.program.trim();
    if name.is_empty() || program.is_empty() {
        return Err("A validation command needs both a name and executable program.".into());
    }
    let arguments = input
        .arguments
        .into_iter()
        .filter(|argument| !argument.trim().is_empty())
        .collect();
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .create_validation_command(NewValidationCommand {
            id: Uuid::new_v4().to_string(),
            project_id: input.project_id,
            stage,
            name: name.to_owned(),
            program: program.to_owned(),
            arguments,
        })
        .map(Into::into)
        .map_err(|error| format!("Unable to save validation command: {error}"))
}

#[tauri::command]
fn delete_validation_command(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let removed = state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .delete_validation_command(&id)
        .map_err(|error| format!("Unable to delete validation command: {error}"))?;
    if removed {
        Ok(())
    } else {
        Err("The validation command no longer exists.".into())
    }
}

#[tauri::command]
fn list_validation_attempts(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ValidationAttemptResponse>, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .list_validation_attempts(&project_id, 20)
        .map(|attempts| attempts.into_iter().map(Into::into).collect())
        .map_err(|error| format!("Unable to load validation history: {error}"))
}

#[tauri::command]
fn get_project_health(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<ProjectHealthResponse, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .get_project_health(&project_id)
        .map(Into::into)
        .map_err(|error| format!("Unable to load project health: {error}"))
}

#[tauri::command]
fn rerun_integration_validation(
    project_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ValidationAttemptResponse, String> {
    let workspace_path = state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .get_project(&project_id)
        .map_err(|error| format!("Unable to load the project workspace: {error}"))?
        .and_then(|project| {
            project
                .workspaces
                .into_iter()
                .find(|workspace| workspace.worker_id == LOCAL_WORKER_ID)
        })
        .map(|workspace| workspace.path)
        .ok_or_else(|| "This project has no local integration workspace.".to_owned())?;
    run_validation(
        &state.database,
        &app,
        &project_id,
        None,
        None,
        ValidationStage::Integration,
        Path::new(&workspace_path),
        true,
    )
    .map(Into::into)
}

fn parse_validation_stage(value: &str) -> Result<ValidationStage, String> {
    match value {
        "implementation" => Ok(ValidationStage::Implementation),
        "integration" => Ok(ValidationStage::Integration),
        _ => Err("Unknown validation stage.".into()),
    }
}

fn run_validation(
    database: &Arc<Mutex<Database>>,
    app: &AppHandle,
    project_id: &str,
    task_id: Option<&str>,
    integration_attempt_id: Option<&str>,
    stage: ValidationStage,
    working_directory: &Path,
    updates_project_health: bool,
) -> Result<ValidationAttempt, String> {
    let attempt_id = Uuid::new_v4().to_string();
    let commands = {
        let mut database = database
            .lock()
            .map_err(|_| "The local project store is unavailable.".to_owned())?;
        let attempt = database
            .start_validation_attempt(
                &attempt_id,
                project_id,
                task_id,
                integration_attempt_id,
                stage,
            )
            .map_err(|error| format!("Unable to begin validation: {error}"))?;
        let commands = database
            .list_validation_commands(project_id, stage)
            .map_err(|error| format!("Unable to load validation commands: {error}"))?;
        if commands.is_empty() {
            database
                .append_validation_event(
                    &attempt.id,
                    NewValidationEvent {
                        command_id: None,
                        kind: "validation.skipped".into(),
                        message: "No validation commands are configured for this stage.".into(),
                        stream: None,
                        exit_code: None,
                    },
                )
                .map_err(|error| format!("Unable to record validation event: {error}"))?;
        }
        commands
            .into_iter()
            .filter(|command| command.enabled)
            .collect::<Vec<_>>()
    };

    let mut status = ValidationStatus::Passed;
    let mut failure: Option<String> = None;
    for command in commands {
        let command_line = display_validation_command(&command);
        append_validation_event(
            database,
            app,
            &attempt_id,
            NewValidationEvent {
                command_id: Some(command.id.clone()),
                kind: "command.started".into(),
                message: command_line.clone(),
                stream: None,
                exit_code: None,
            },
        )?;
        let run = match LocalWorker::start(ProcessRequest {
            program: command.program.clone(),
            arguments: command.arguments.clone(),
            working_directory: Some(working_directory.to_path_buf()),
            standard_input: None,
        }) {
            Ok(run) => run,
            Err(error) => {
                status = ValidationStatus::Failed;
                failure = Some(format!("{} could not start: {error}", command.name));
                append_validation_event(
                    database,
                    app,
                    &attempt_id,
                    NewValidationEvent {
                        command_id: Some(command.id.clone()),
                        kind: "command.failed".into(),
                        message: failure.clone().unwrap_or_default(),
                        stream: None,
                        exit_code: None,
                    },
                )?;
                break;
            }
        };
        let handle = run.handle;
        for output in run.output {
            let stream_name = match output.stream {
                OutputStream::Stdout => "stdout",
                OutputStream::Stderr => "stderr",
            };
            append_validation_event(
                database,
                app,
                &attempt_id,
                NewValidationEvent {
                    command_id: Some(command.id.clone()),
                    kind: "command.output".into(),
                    message: output.text,
                    stream: Some(stream_name.into()),
                    exit_code: None,
                },
            )?;
        }
        match handle.wait() {
            Ok(exit) if exit.success() => append_validation_event(
                database,
                app,
                &attempt_id,
                NewValidationEvent {
                    command_id: Some(command.id.clone()),
                    kind: "command.completed".into(),
                    message: format!("{} passed.", command.name),
                    stream: None,
                    exit_code: exit.code(),
                },
            )?,
            Ok(exit) => {
                status = ValidationStatus::Failed;
                failure = Some(format!(
                    "{} failed with exit code {}.",
                    command.name,
                    exit.code()
                        .map_or("unknown".into(), |code| code.to_string())
                ));
                append_validation_event(
                    database,
                    app,
                    &attempt_id,
                    NewValidationEvent {
                        command_id: Some(command.id.clone()),
                        kind: "command.failed".into(),
                        message: failure.clone().unwrap_or_default(),
                        stream: None,
                        exit_code: exit.code(),
                    },
                )?;
                break;
            }
            Err(error) => {
                status = ValidationStatus::Failed;
                failure = Some(format!("{} could not finish: {error}", command.name));
                append_validation_event(
                    database,
                    app,
                    &attempt_id,
                    NewValidationEvent {
                        command_id: Some(command.id.clone()),
                        kind: "command.failed".into(),
                        message: failure.clone().unwrap_or_default(),
                        stream: None,
                        exit_code: None,
                    },
                )?;
                break;
            }
        }
    }
    let attempt = database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .finish_validation_attempt(&attempt_id, status, failure.as_deref())
        .map_err(|error| format!("Unable to finish validation: {error}"))?
        .ok_or_else(|| "The validation attempt no longer exists.".to_owned())?;
    if updates_project_health {
        database
            .lock()
            .map_err(|_| "The local project store is unavailable.".to_owned())?
            .record_project_validation(project_id, &attempt_id, status, failure.as_deref(), false)
            .map_err(|error| format!("Unable to update project health: {error}"))?;
    }
    let _ = app.emit(
        "validation://event",
        ValidationRunEvent {
            validation_attempt_id: attempt_id,
            kind: format!("validation.{}", status.as_str()),
            command_id: None,
            stream: None,
            text: failure.unwrap_or_else(|| "Validation passed.".into()),
            exit_code: None,
        },
    );
    Ok(attempt)
}

fn append_validation_event(
    database: &Arc<Mutex<Database>>,
    app: &AppHandle,
    attempt_id: &str,
    event: NewValidationEvent,
) -> Result<(), String> {
    let text = event.message.clone();
    let command_id = event.command_id.clone();
    let kind = event.kind.clone();
    let stream = match event.stream.as_deref() {
        Some("stdout") => Some(OutputStream::Stdout),
        Some("stderr") => Some(OutputStream::Stderr),
        _ => None,
    };
    let exit_code = event.exit_code;
    database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .append_validation_event(attempt_id, event)
        .map_err(|error| format!("Unable to record validation output: {error}"))?;
    let _ = app.emit(
        "validation://event",
        ValidationRunEvent {
            validation_attempt_id: attempt_id.to_owned(),
            kind,
            command_id,
            stream,
            text,
            exit_code,
        },
    );
    Ok(())
}

fn display_validation_command(command: &ValidationCommand) -> String {
    std::iter::once(command.program.as_str())
        .chain(command.arguments.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

#[tauri::command]
fn integrate_next_task(
    project_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<IntegrationExecutionResponse, String> {
    let health = state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .get_project_health(&project_id)
        .map_err(|error| format!("Unable to load project health: {error}"))?;
    if health.status == orchestr_db::ProjectHealthStatus::Broken {
        let gate = health
            .failing_gate
            .unwrap_or_else(|| "a required integration validation command".into());
        return Err(format!("Integration is paused because {} is broken. Re-run the integration validation after fixing {gate}.", project_id));
    }
    let attempt = state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .claim_next_integration(&project_id)
        .map_err(|_| "Another task is already integrating for this project.".to_owned())?
        .ok_or_else(|| "No approved task is waiting for integration.".to_owned())?;

    let (task, workspace_path) = match integration_context(&state, &attempt) {
        Ok(context) => context,
        Err(error) => return finish_failed_integration(&state, &attempt.id, error),
    };
    let worktree_path = match task.worktree_path.as_deref() {
        Some(path) => path,
        None => {
            return finish_failed_integration(
                &state,
                &attempt.id,
                "The task worktree is no longer available.".into(),
            )
        }
    };
    if task.branch.as_deref() != Some(attempt.source_branch.as_str()) {
        return finish_failed_integration(
            &state,
            &attempt.id,
            "The task branch no longer matches its queued integration attempt.".into(),
        );
    }

    match GitService::prepare_task_for_integration(
        Path::new(&workspace_path),
        Path::new(worktree_path),
        &attempt.source_branch,
        &attempt.target_branch,
    ) {
        Ok(IntegrationPreparation::Ready) => {}
        Ok(IntegrationPreparation::Conflict { paths }) => {
            let message = format!("Integration conflicts: {}", paths.join(", "));
            let task = state
                .database
                .lock()
                .map_err(|_| "The local project store is unavailable.".to_owned())?
                .block_integration(&attempt.id, &message)
                .map_err(|error| format!("Unable to record the integration conflict: {error}"))?
                .ok_or_else(|| "The integration attempt is no longer active.".to_owned())?;
            let attempt = integration_attempt(&state, &attempt.id)?;
            return Ok(IntegrationExecutionResponse {
                task: task.into(),
                attempt: attempt.into(),
                outcome: "conflict".into(),
                message,
                cleanup_error: None,
            });
        }
        Err(error) => return finish_failed_integration(&state, &attempt.id, error.to_string()),
    }
    let validation = run_validation(
        &state.database,
        &app,
        &task.project_id,
        Some(&task.id),
        Some(&attempt.id),
        ValidationStage::Integration,
        Path::new(worktree_path),
        true,
    );
    let validation = match validation {
        Ok(validation) => validation,
        Err(error) => return finish_failed_integration(&state, &attempt.id, error),
    };
    if validation.status != ValidationStatus::Passed {
        return finish_failed_integration(
            &state,
            &attempt.id,
            validation
                .error
                .unwrap_or_else(|| "Integration validation failed.".into()),
        );
    }

    let message = format!("task: {}", task.title);
    match GitService::squash_integrate_task(
        Path::new(&workspace_path),
        Path::new(worktree_path),
        &attempt.source_branch,
        &attempt.target_branch,
        &message,
    ) {
        Ok(IntegrationResult::Conflict { paths }) => {
            let message = format!("Integration conflicts: {}", paths.join(", "));
            let task = state
                .database
                .lock()
                .map_err(|_| "The local project store is unavailable.".to_owned())?
                .block_integration(&attempt.id, &message)
                .map_err(|error| format!("Unable to record the integration conflict: {error}"))?
                .ok_or_else(|| "The integration attempt is no longer active.".to_owned())?;
            let attempt = integration_attempt(&state, &attempt.id)?;
            Ok(IntegrationExecutionResponse {
                task: task.into(),
                attempt: attempt.into(),
                outcome: "conflict".into(),
                message,
                cleanup_error: None,
            })
        }
        Ok(IntegrationResult::Merged { commit }) => {
            state
                .database
                .lock()
                .map_err(|_| "The local project store is unavailable.".to_owned())?
                .record_project_validation(
                    &task.project_id,
                    &validation.id,
                    ValidationStatus::Passed,
                    None,
                    true,
                )
                .map_err(|error| {
                    format!("Unable to update project health after integration: {error}")
                })?;
            let completed_task = state
                .database
                .lock()
                .map_err(|_| "The local project store is unavailable.".to_owned())?
                .complete_integration(&attempt.id, &commit)
                .map_err(|error| format!("Unable to record the completed integration: {error}"))?
                .ok_or_else(|| "The integration attempt is no longer active.".to_owned())?;
            let cleanup_error = cleanup_integrated_task(
                &state,
                &attempt.id,
                &completed_task,
                &workspace_path,
                &attempt.source_branch,
            )
            .err();
            let task = state
                .database
                .lock()
                .map_err(|_| "The local project store is unavailable.".to_owned())?
                .get_task(&completed_task.id)
                .map_err(|error| format!("Unable to load the integrated task: {error}"))?
                .ok_or_else(|| "The integrated task no longer exists.".to_owned())?;
            let attempt = integration_attempt(&state, &attempt.id)?;
            let target_branch = attempt.target_branch.clone();
            let _ = dispatch_queued_task_runs(
                app,
                Arc::clone(&state.database),
                Arc::clone(&state.local_worker_runs),
            );
            Ok(IntegrationExecutionResponse {
                task: task.into(),
                attempt: attempt.into(),
                outcome: "merged".into(),
                message: format!("Squash-merged into {target_branch}."),
                cleanup_error,
            })
        }
        Err(error) => finish_failed_integration(&state, &attempt.id, error.to_string()),
    }
}

fn integration_context(
    state: &AppState,
    attempt: &IntegrationAttempt,
) -> Result<(Task, String), String> {
    let database = state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?;
    let task = database
        .get_task(&attempt.task_id)
        .map_err(|error| format!("Unable to load the queued task: {error}"))?
        .ok_or_else(|| "The queued task no longer exists.".to_owned())?;
    let workspace_path = database
        .get_project(&task.project_id)
        .map_err(|error| format!("Unable to load the project integration workspace: {error}"))?
        .and_then(|project| {
            project
                .workspaces
                .into_iter()
                .find(|workspace| workspace.worker_id == LOCAL_WORKER_ID)
                .map(|workspace| workspace.path)
        })
        .ok_or_else(|| "This project has no local integration workspace.".to_owned())?;
    Ok((task, workspace_path))
}

fn integration_attempt(state: &AppState, attempt_id: &str) -> Result<IntegrationAttempt, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .get_integration_attempt(attempt_id)
        .map_err(|error| format!("Unable to load the integration result: {error}"))?
        .ok_or_else(|| "The integration attempt no longer exists.".to_owned())
}

fn finish_failed_integration(
    state: &AppState,
    attempt_id: &str,
    error: String,
) -> Result<IntegrationExecutionResponse, String> {
    let task = state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .fail_integration(attempt_id, &error)
        .map_err(|database_error| {
            format!("Unable to record the failed integration: {database_error}")
        })?
        .ok_or_else(|| "The integration attempt is no longer active.".to_owned())?;
    let attempt = integration_attempt(state, attempt_id)?;
    Ok(IntegrationExecutionResponse {
        task: task.into(),
        attempt: attempt.into(),
        outcome: "failed".into(),
        message: error,
        cleanup_error: None,
    })
}

fn cleanup_integrated_task(
    state: &AppState,
    attempt_id: &str,
    task: &Task,
    workspace_path: &str,
    branch: &str,
) -> Result<(), String> {
    let cleanup = (|| {
        if let Some(worktree_path) = task.worktree_path.as_deref() {
            if Path::new(worktree_path).exists() {
                GitService::remove_task_worktree(
                    Path::new(workspace_path),
                    Path::new(worktree_path),
                )
                .map_err(|error| {
                    format!("Unable to remove the integrated task worktree: {error}")
                })?;
            }
            state
                .database
                .lock()
                .map_err(|_| "The local project store is unavailable.".to_owned())?
                .release_task_worktree(&task.id)
                .map_err(|error| {
                    format!("Unable to release the integrated task worktree: {error}")
                })?;
        }
        GitService::delete_task_branch_if_exists(Path::new(workspace_path), branch)
            .map_err(|error| format!("Unable to delete the integrated task branch: {error}"))
    })();
    if let Err(error) = cleanup {
        state
            .database
            .lock()
            .map_err(|_| "The local project store is unavailable.".to_owned())?
            .record_integration_cleanup_error(attempt_id, &error)
            .map_err(|database_error| {
                format!("Unable to record the cleanup failure: {database_error}")
            })?;
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
fn cleanup_task_worktree(
    task_id: String,
    state: State<'_, AppState>,
) -> Result<TaskResponse, String> {
    let (task, workspace_path) = {
        let database = state
            .database
            .lock()
            .map_err(|_| "The local project store is unavailable.".to_owned())?;
        let task = database
            .get_task(&task_id)
            .map_err(|error| format!("Unable to load the task worktree: {error}"))?
            .ok_or_else(|| "The task no longer exists.".to_owned())?;
        if database
            .list_runs_for_task(&task_id)
            .map_err(|error| format!("Unable to inspect task runs: {error}"))?
            .iter()
            .any(|run| run.status == RunStatus::Running)
        {
            return Err("Cancel or wait for the active run before removing its worktree.".into());
        }
        let workspace_path = database
            .get_project(&task.project_id)
            .map_err(|error| format!("Unable to load the task workspace: {error}"))?
            .and_then(|project| {
                project
                    .workspaces
                    .into_iter()
                    .find(|workspace| workspace.worker_id == LOCAL_WORKER_ID)
                    .map(|workspace| workspace.path)
            })
            .ok_or_else(|| "This project has no local workspace.".to_owned())?;
        (task, workspace_path)
    };
    let worktree_path = task
        .worktree_path
        .as_deref()
        .ok_or_else(|| "This task does not own an isolated worktree.".to_owned())?;
    GitService::remove_task_worktree(Path::new(&workspace_path), Path::new(worktree_path))
        .map_err(|error| {
            format!(
                "Unable to remove the task worktree. Commit, stash, or discard its changes first: {error}"
            )
        })?;
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .release_task_worktree(&task_id)
        .map_err(|error| format!("Unable to release the task worktree: {error}"))?
        .map(Into::into)
        .ok_or_else(|| "The task worktree was already released.".into())
}

#[tauri::command]
fn open_task_worktree(task_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let (task, workspace_path) = {
        let database = state
            .database
            .lock()
            .map_err(|_| "The local project store is unavailable.".to_owned())?;
        let task = database
            .get_task(&task_id)
            .map_err(|error| format!("Unable to load the task worktree: {error}"))?
            .ok_or_else(|| "The task no longer exists.".to_owned())?;
        let workspace_path = database
            .get_project(&task.project_id)
            .map_err(|error| format!("Unable to load the task workspace: {error}"))?
            .and_then(|project| {
                project
                    .workspaces
                    .into_iter()
                    .find(|workspace| workspace.worker_id == LOCAL_WORKER_ID)
                    .map(|workspace| workspace.path)
            })
            .ok_or_else(|| "This project has no local workspace.".to_owned())?;
        (task, workspace_path)
    };

    let recorded_path = task
        .worktree_path
        .ok_or_else(|| "This task does not own an isolated worktree.".to_owned())?;
    let expected_path = task_worktree_path(&workspace_path, &task.project_id, &task.id)?;
    let worktree_path = fs::canonicalize(&recorded_path)
        .map_err(|error| format!("The task worktree is no longer available: {error}"))?;
    let expected_path = fs::canonicalize(&expected_path).map_err(|_| {
        "The task worktree is not in Orchestr's managed worktree location.".to_owned()
    })?;
    if worktree_path != expected_path || !worktree_path.is_dir() {
        return Err("The task worktree is not in Orchestr's managed worktree location.".into());
    }

    open_directory(&worktree_path)
}

#[tauri::command]
fn start_task_run(
    task_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<StartedTaskRunResponse, String> {
    let (task, agent) = {
        let database = state
            .database
            .lock()
            .map_err(|_| "The local run store is unavailable.".to_owned())?;
        let task = database
            .get_task(&task_id)
            .map_err(|error| format!("Unable to load task for execution: {error}"))?
            .ok_or_else(|| "The task no longer exists.".to_owned())?;
        if task.status != TaskStatus::Ready {
            return Err(
                "Only Ready tasks can be started. Resolve any blocked requirements before running it again."
                    .into(),
            );
        }
        let agent_id = task
            .assigned_agent_id
            .as_deref()
            .ok_or_else(|| "Assign a Codex agent before starting this task.".to_owned())?;
        let agent = database
            .get_agent(agent_id)
            .map_err(|error| format!("Unable to load the assigned agent: {error}"))?
            .ok_or_else(|| "The assigned agent no longer exists.".to_owned())?;
        if agent.provider != "codex" {
            return Err("Only Codex agents can run locally at this stage.".into());
        }
        (task, agent)
    };

    let provider_status = CodexProvider
        .inspect()
        .map_err(|error| format!("Unable to inspect Codex before starting the task: {error}"))?;
    if !matches!(provider_status.readiness, ProviderReadiness::Ready) {
        return Err(format!(
            "Codex is not ready to run this task. {}",
            provider_status.detail
        ));
    }

    if task.worktree_path.is_some() {
        return Err(
            "This task already owns an isolated worktree. Remove it explicitly before starting a fresh run."
                .into(),
        );
    }
    let run_id = Uuid::new_v4().to_string();
    let (queued_run, queued_task) = state
        .database
        .lock()
        .map_err(|_| "The local run store is unavailable.".to_owned())?
        .enqueue_run(NewRun {
            id: run_id.clone(),
            task_id: task.id,
            agent_id: agent.id,
            worker_id: LOCAL_WORKER_ID.to_owned(),
        })
        .map_err(|error| format!("Unable to queue the task run: {error}"))?;
    dispatch_queued_task_runs(
        app,
        Arc::clone(&state.database),
        Arc::clone(&state.local_worker_runs),
    )?;
    let database = state
        .database
        .lock()
        .map_err(|_| "The local run store is unavailable.".to_owned())?;
    Ok(StartedTaskRunResponse {
        run: database
            .get_run(&run_id)
            .map_err(|error| format!("Unable to reload the queued run: {error}"))?
            .unwrap_or(queued_run)
            .into(),
        task: database
            .get_task(&queued_task.id)
            .map_err(|error| format!("Unable to reload the queued task: {error}"))?
            .unwrap_or(queued_task)
            .into(),
    })
}

fn dispatch_queued_task_runs(
    app: AppHandle,
    database: Arc<Mutex<Database>>,
    active_runs: Arc<Mutex<HashMap<String, ActiveLocalRun>>>,
) -> Result<(), String> {
    loop {
        let Some((run, task)) = claim_queued_task_run(&database)? else {
            return Ok(());
        };
        dispatch_claimed_task_run(
            run,
            task,
            app.clone(),
            Arc::clone(&database),
            Arc::clone(&active_runs),
        );
    }
}

fn claim_queued_task_run(database: &Arc<Mutex<Database>>) -> Result<Option<(Run, Task)>, String> {
    database
        .lock()
        .map_err(|_| "The local run store is unavailable.".to_owned())?
        .claim_next_run(LOCAL_WORKER_ID)
        .map_err(|error| format!("Unable to claim queued work: {error}"))
}

fn dispatch_claimed_task_run(
    run: Run,
    task: Task,
    app: AppHandle,
    database: Arc<Mutex<Database>>,
    active_runs: Arc<Mutex<HashMap<String, ActiveLocalRun>>>,
) {
    let result = load_queued_run_context(&database, &run, &task).and_then(
        |(agent, workspace_path, default_branch)| {
            launch_claimed_task_run(
                run.clone(),
                task,
                agent,
                workspace_path,
                default_branch,
                app.clone(),
                Arc::clone(&database),
                Arc::clone(&active_runs),
            )
        },
    );
    if let Err(error) = result {
        record_task_launch_failure(&database, &app, run.id, error);
    }
}

fn load_queued_run_context(
    database: &Arc<Mutex<Database>>,
    run: &Run,
    task: &Task,
) -> Result<(Agent, String, String), String> {
    let store = database
        .lock()
        .map_err(|_| "The local run store is unavailable.".to_owned())?;
    let agent = queued_run_agent(&store, &run.agent_id)?;
    let (workspace_path, default_branch) = queued_run_workspace(&store, &task.project_id)?;
    Ok((agent, workspace_path, default_branch))
}

fn queued_run_agent(database: &Database, agent_id: &str) -> Result<Agent, String> {
    database
        .get_agent(agent_id)
        .map_err(|error| format!("Unable to load the queued run's agent: {error}"))?
        .ok_or_else(|| "The queued run's agent no longer exists.".to_owned())
}

fn queued_run_workspace(database: &Database, project_id: &str) -> Result<(String, String), String> {
    let project = database
        .get_project(project_id)
        .map_err(|error| format!("Unable to load the queued run's project: {error}"))?
        .ok_or_else(|| "The queued run's project no longer exists.".to_owned())?;
    project
        .workspaces
        .into_iter()
        .find(|workspace| workspace.worker_id == LOCAL_WORKER_ID)
        .map(|workspace| (workspace.path, project.default_branch))
        .ok_or_else(|| "This project has no local workspace.".to_owned())
}

fn record_task_launch_failure(
    database: &Arc<Mutex<Database>>,
    app: &AppHandle,
    run_id: String,
    error: String,
) {
    if let Ok(mut store) = database.lock() {
        let _ = store.finish_run(&run_id, RunStatus::Failed, None, Some(&error));
    }
    let _ = app.emit(
        "worker://run-event",
        WorkerRunEvent {
            run_id,
            kind: "failed".into(),
            stream: None,
            text: Some(error),
            raw_text: None,
            command: None,
            exit_code: None,
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn launch_claimed_task_run(
    persisted_run: Run,
    task: Task,
    agent: Agent,
    workspace_path: String,
    default_branch: String,
    app: AppHandle,
    database: Arc<Mutex<Database>>,
    active_runs: Arc<Mutex<HashMap<String, ActiveLocalRun>>>,
) -> Result<(), String> {
    let prepared = prepare_task_run_worktree(&task, &workspace_path, &default_branch, &database)?;
    let request = prepare_task_process_request(&task, &agent, &prepared.worktree_path)?;
    let run = LocalWorker::start(request)
        .map_err(|error| format!("Unable to start Codex for this task: {error}"))?;
    record_task_worktree_events(&database, &persisted_run.id, &prepared);
    register_task_worker(
        run,
        persisted_run.id,
        task.project_id,
        task.id,
        prepared,
        app,
        database,
        active_runs,
    )
}

struct PreparedTaskRun {
    branch: String,
    worktree_path: String,
    created_branch: bool,
    repository_before: Option<RepositoryObservation>,
}

fn prepare_task_run_worktree(
    task: &Task,
    workspace_path: &str,
    default_branch: &str,
    database: &Arc<Mutex<Database>>,
) -> Result<PreparedTaskRun, String> {
    if let Some(worktree_path) = task.worktree_path.as_deref() {
        return reuse_task_run_worktree(task, workspace_path, worktree_path);
    }
    create_fresh_task_run_worktree(task, workspace_path, default_branch, database)
}

fn reuse_task_run_worktree(
    task: &Task,
    workspace_path: &str,
    worktree_path: &str,
) -> Result<PreparedTaskRun, String> {
    let branch = task
        .branch
        .clone()
        .ok_or_else(|| "The recoverable task worktree has no recorded branch.".to_owned())?;
    let worktree = canonical_recovery_worktree(task, workspace_path, worktree_path)?;
    ensure_recovery_worktree_branch(&worktree, &branch)?;
    Ok(PreparedTaskRun {
        branch,
        repository_before: repository_observation(&worktree),
        worktree_path: normalize_workspace_path(&worktree.to_string_lossy()),
        created_branch: false,
    })
}

fn canonical_recovery_worktree(
    task: &Task,
    workspace_path: &str,
    worktree_path: &str,
) -> Result<PathBuf, String> {
    let expected = task_worktree_path(workspace_path, &task.project_id, &task.id)?;
    let worktree = fs::canonicalize(worktree_path)
        .map_err(|error| format!("The recoverable task worktree is unavailable: {error}"))?;
    let expected = fs::canonicalize(expected).map_err(|_| {
        "The recoverable task worktree is outside Orchestr's managed location.".to_owned()
    })?;
    ensure_managed_worktree_path(worktree, expected)
}

fn ensure_managed_worktree_path(worktree: PathBuf, expected: PathBuf) -> Result<PathBuf, String> {
    if worktree == expected {
        Ok(worktree)
    } else {
        Err("The recoverable task worktree is outside Orchestr's managed location.".into())
    }
}

fn ensure_recovery_worktree_branch(worktree: &Path, branch: &str) -> Result<(), String> {
    let repository = GitService::inspect_repository(worktree)
        .map_err(|error| format!("Unable to inspect the recoverable task worktree: {error}"))?;
    if repository.current_branch.as_deref() == Some(branch) {
        Ok(())
    } else {
        Err("The recoverable task worktree is checked out on a different branch.".into())
    }
}

fn create_fresh_task_run_worktree(
    task: &Task,
    workspace_path: &str,
    default_branch: &str,
    database: &Arc<Mutex<Database>>,
) -> Result<PreparedTaskRun, String> {
    let branch = task
        .branch
        .clone()
        .unwrap_or_else(|| task_branch_name(&task.id, &task.title));
    let target_worktree_path = task_worktree_path(&workspace_path, &task.project_id, &task.id)?;
    let task_worktree = GitService::create_task_worktree(
        Path::new(&workspace_path),
        &target_worktree_path,
        &branch,
        &default_branch,
    )
    .map_err(|error| format!("Unable to create the isolated task worktree: {error}"))?;
    let worktree_path = normalize_workspace_path(&task_worktree.path.to_string_lossy());
    assign_prepared_task_worktree(
        database,
        task,
        &branch,
        &worktree_path,
        workspace_path,
        &task_worktree.path,
    )?;
    Ok(PreparedTaskRun {
        branch,
        repository_before: repository_observation(Path::new(&worktree_path)),
        worktree_path,
        created_branch: task_worktree.created_branch,
    })
}

fn assign_prepared_task_worktree(
    database: &Arc<Mutex<Database>>,
    task: &Task,
    branch: &str,
    worktree_path: &str,
    workspace_path: &str,
    created_path: &Path,
) -> Result<(), String> {
    let assigned = database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .assign_task_worktree(&task.id, branch, worktree_path)
        .map_err(|error| format!("Unable to record the task worktree: {error}"))?;
    if assigned.is_some() {
        return Ok(());
    }
    let _ = GitService::remove_task_worktree(Path::new(workspace_path), created_path);
    Err("The task is no longer eligible to start.".into())
}

fn prepare_task_process_request(
    task: &Task,
    agent: &Agent,
    worktree_path: &str,
) -> Result<ProcessRequest, String> {
    let additional_writable_directories =
        GitService::writable_git_directories(Path::new(worktree_path)).map_err(|error| {
            format!("Unable to prepare Git metadata access for the task worktree: {error}")
        })?;
    CodexProvider
        .execution_request(AgentRunInput {
            model: agent.model.clone(),
            prompt: build_task_prompt(&task, &agent),
            working_directory: PathBuf::from(worktree_path),
            additional_writable_directories,
            read_only: false,
        })
        .map_err(|error| format!("Unable to prepare the Codex task: {error}"))
}

fn record_task_worktree_events(
    database: &Arc<Mutex<Database>>,
    run_id: &str,
    prepared: &PreparedTaskRun,
) {
    if let Ok(mut database) = database.lock() {
        if prepared.created_branch {
            let _ = database.append_run_event(
                run_id,
                NewRunEvent {
                    kind: "git.branch.created".into(),
                    message: format!("Created task branch {}.", prepared.branch),
                    command: None,
                    file_path: None,
                    exit_code: None,
                },
            );
        }
        let _ = database.append_run_event(
            run_id,
            NewRunEvent {
                kind: "git.worktree.created".into(),
                message: format!("Created isolated worktree at {}.", prepared.worktree_path),
                command: None,
                file_path: None,
                exit_code: None,
            },
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn register_task_worker(
    run: orchestr_worker::WorkerRun,
    run_id: String,
    project_id: String,
    task_id: String,
    prepared: PreparedTaskRun,
    app: AppHandle,
    database: Arc<Mutex<Database>>,
    active_runs: Arc<Mutex<HashMap<String, ActiveLocalRun>>>,
) -> Result<(), String> {
    let handle = run.handle;
    active_runs
        .lock()
        .map_err(|_| "The local worker state is unavailable.".to_owned())?
        .insert(
            run_id.clone(),
            ActiveLocalRun {
                handle: handle.clone(),
                cancel_requested: false,
            },
        );
    let _ = app.emit("scheduler://changed", run_id.clone());
    thread::spawn(move || {
        monitor_task_worker(
            run.output,
            handle,
            run_id,
            project_id,
            task_id,
            prepared,
            app,
            database,
            active_runs,
        );
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn monitor_task_worker(
    output: std::sync::mpsc::Receiver<orchestr_worker::ProcessOutput>,
    handle: WorkerHandle,
    run_id: String,
    project_id: String,
    task_id: String,
    prepared: PreparedTaskRun,
    app: AppHandle,
    database: Arc<Mutex<Database>>,
    active_runs: Arc<Mutex<HashMap<String, ActiveLocalRun>>>,
) {
    forward_task_output(output, &run_id, &database, &app);
    let result = handle.wait();
    let cancelled = remove_active_task_run(&active_runs, &run_id);
    let outcome = classify_task_process_result(
        result,
        cancelled,
        &database,
        &app,
        &project_id,
        &task_id,
        &prepared.worktree_path,
    );
    finish_task_worker(&database, &run_id, &prepared, &outcome);
    emit_task_worker_outcome(&app, &run_id, outcome);
    let _ = dispatch_queued_task_runs(app, database, active_runs);
}

fn forward_task_output(
    output: std::sync::mpsc::Receiver<orchestr_worker::ProcessOutput>,
    run_id: &str,
    database: &Arc<Mutex<Database>>,
    app: &AppHandle,
) {
    for output in output {
        let stream = output.stream;
        let raw_text = output.text;
        let event = CodexProvider::execution_event(&raw_text);
        if let Ok(mut database) = database.lock() {
            let _ = database.append_run_output(run_id, output_stream_name(&stream), &raw_text);
            let _ = database.append_run_event(
                run_id,
                NewRunEvent {
                    kind: event.kind.clone(),
                    message: event.message.clone(),
                    command: event.command.clone(),
                    file_path: None,
                    exit_code: event.exit_code,
                },
            );
        }
        let _ = app.emit(
            "worker://run-event",
            WorkerRunEvent {
                run_id: run_id.to_owned(),
                kind: event.kind,
                stream: Some(stream),
                text: Some(event.message),
                raw_text: Some(raw_text),
                command: event.command,
                exit_code: None,
            },
        );
    }
}

fn output_stream_name(stream: &OutputStream) -> &'static str {
    match stream {
        OutputStream::Stdout => "stdout",
        OutputStream::Stderr => "stderr",
    }
}

fn remove_active_task_run(
    active_runs: &Arc<Mutex<HashMap<String, ActiveLocalRun>>>,
    run_id: &str,
) -> bool {
    active_runs
        .lock()
        .ok()
        .and_then(|mut runs| runs.remove(run_id))
        .is_some_and(|run| run.cancel_requested)
}

struct TaskRunOutcome {
    status: RunStatus,
    kind: &'static str,
    text: Option<String>,
    exit_code: Option<i32>,
}

#[allow(clippy::too_many_arguments)]
fn classify_task_process_result(
    result: orchestr_worker::Result<std::process::ExitStatus>,
    cancelled: bool,
    database: &Arc<Mutex<Database>>,
    app: &AppHandle,
    project_id: &str,
    task_id: &str,
    worktree_path: &str,
) -> TaskRunOutcome {
    match result {
        Ok(exit_status) => classify_task_exit_status(
            exit_status,
            cancelled,
            database,
            app,
            project_id,
            task_id,
            worktree_path,
        ),
        Err(error) => failed_task_outcome(error.to_string(), None),
    }
}

#[allow(clippy::too_many_arguments)]
fn classify_task_exit_status(
    exit_status: std::process::ExitStatus,
    cancelled: bool,
    database: &Arc<Mutex<Database>>,
    app: &AppHandle,
    project_id: &str,
    task_id: &str,
    worktree_path: &str,
) -> TaskRunOutcome {
    if cancelled {
        return TaskRunOutcome {
            status: RunStatus::Cancelled,
            kind: "cancelled",
            text: Some("Codex task cancelled.".into()),
            exit_code: exit_status.code(),
        };
    }
    if !exit_status.success() {
        return failed_task_outcome("Codex exited with an error.".into(), exit_status.code());
    }
    inspect_completed_task(
        database,
        app,
        project_id,
        task_id,
        worktree_path,
        exit_status.code(),
    )
}

fn inspect_completed_task(
    database: &Arc<Mutex<Database>>,
    app: &AppHandle,
    project_id: &str,
    task_id: &str,
    worktree_path: &str,
    exit_code: Option<i32>,
) -> TaskRunOutcome {
    match GitService::inspect_repository(Path::new(worktree_path)) {
        Ok(repository) => validate_completed_task(
            repository.is_clean,
            database,
            app,
            project_id,
            task_id,
            worktree_path,
            exit_code,
        ),
        Err(error) => failed_task_outcome(
            format!("Unable to verify the task worktree after Codex completed: {error}"),
            exit_code,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_completed_task(
    is_clean: bool,
    database: &Arc<Mutex<Database>>,
    app: &AppHandle,
    project_id: &str,
    task_id: &str,
    worktree_path: &str,
    exit_code: Option<i32>,
) -> TaskRunOutcome {
    if !is_clean {
        return failed_task_outcome(
            "Codex finished with uncommitted task changes. Commit the task worktree before requesting review."
                .into(),
            exit_code,
        );
    }
    match run_validation(
        database,
        app,
        project_id,
        Some(task_id),
        None,
        ValidationStage::Implementation,
        Path::new(worktree_path),
        false,
    ) {
        Ok(validation) => implementation_validation_outcome(validation, exit_code),
        Err(error) => failed_task_outcome(error, exit_code),
    }
}

fn implementation_validation_outcome(
    validation: ValidationAttempt,
    exit_code: Option<i32>,
) -> TaskRunOutcome {
    if validation.status == ValidationStatus::Passed {
        return TaskRunOutcome {
            status: RunStatus::Completed,
            kind: "completed",
            text: None,
            exit_code,
        };
    }
    failed_task_outcome(
        validation
            .error
            .unwrap_or_else(|| "Implementation validation failed.".into()),
        exit_code,
    )
}

fn failed_task_outcome(message: String, exit_code: Option<i32>) -> TaskRunOutcome {
    TaskRunOutcome {
        status: RunStatus::Failed,
        kind: "failed",
        text: Some(message),
        exit_code,
    }
}

fn finish_task_worker(
    database: &Arc<Mutex<Database>>,
    run_id: &str,
    prepared: &PreparedTaskRun,
    outcome: &TaskRunOutcome,
) {
    if let Ok(mut database) = database.lock() {
        record_repository_events(
            &mut database,
            run_id,
            Path::new(&prepared.worktree_path),
            prepared.repository_before.as_ref(),
        );
        let _ = database.finish_run(
            run_id,
            outcome.status,
            outcome.exit_code,
            outcome.text.as_deref(),
        );
    }
}

fn emit_task_worker_outcome(app: &AppHandle, run_id: &str, outcome: TaskRunOutcome) {
    let _ = app.emit(
        "worker://run-event",
        WorkerRunEvent {
            run_id: run_id.to_owned(),
            kind: outcome.kind.into(),
            stream: None,
            text: outcome.text,
            raw_text: None,
            command: None,
            exit_code: outcome.exit_code,
        },
    );
}

#[tauri::command]
fn list_agents(state: State<'_, AppState>) -> Result<Vec<AgentResponse>, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local agent store is unavailable.".to_owned())?
        .list_agents()
        .map(|agents| agents.into_iter().map(Into::into).collect())
        .map_err(|error| format!("Unable to load agents: {error}"))
}

#[tauri::command]
fn create_agent(
    input: CreateAgentInput,
    state: State<'_, AppState>,
) -> Result<AgentResponse, String> {
    let agent = validate_agent_input(input)?;
    state
        .database
        .lock()
        .map_err(|_| "The local agent store is unavailable.".to_owned())?
        .create_agent(NewAgent {
            id: Uuid::new_v4().to_string(),
            name: agent.name,
            provider: agent.provider,
            role: agent.role,
            model: agent.model,
            system_prompt: agent.system_prompt,
            skills: agent.skills,
            max_concurrent_tasks: agent.max_concurrent_tasks,
        })
        .map(Into::into)
        .map_err(|error| format!("Unable to create agent: {error}"))
}

#[tauri::command]
fn update_agent(
    input: UpdateAgentInput,
    state: State<'_, AppState>,
) -> Result<AgentResponse, String> {
    let id = input.id.clone();
    let agent = validate_agent_input(CreateAgentInput {
        name: input.name,
        provider: input.provider,
        role: input.role,
        model: input.model,
        system_prompt: input.system_prompt,
        skills: input.skills,
        max_concurrent_tasks: input.max_concurrent_tasks,
    })?;
    state
        .database
        .lock()
        .map_err(|_| "The local agent store is unavailable.".to_owned())?
        .update_agent(
            &id,
            AgentUpdate {
                name: agent.name,
                provider: agent.provider,
                role: agent.role,
                model: agent.model,
                system_prompt: agent.system_prompt,
                skills: agent.skills,
                max_concurrent_tasks: agent.max_concurrent_tasks,
            },
        )
        .map_err(|error| format!("Unable to update agent: {error}"))?
        .map(Into::into)
        .ok_or_else(|| "The agent no longer exists.".into())
}

#[tauri::command]
fn delete_agent(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .database
        .lock()
        .map_err(|_| "The local agent store is unavailable.".to_owned())?
        .delete_agent(&id)
        .map_err(|error| format!("Unable to delete agent: {error}"))?
        .then_some(())
        .ok_or_else(|| "The agent no longer exists.".into())
}

#[tauri::command]
fn list_tasks(project_id: String, state: State<'_, AppState>) -> Result<Vec<TaskResponse>, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .list_tasks(&project_id)
        .map(|tasks| tasks.into_iter().map(Into::into).collect())
        .map_err(|error| format!("Unable to load tasks: {error}"))
}

#[tauri::command]
fn list_milestones(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<MilestoneResponse>, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .list_milestones(&project_id)
        .map(|items| items.into_iter().map(Into::into).collect())
        .map_err(|error| format!("Unable to load milestones: {error}"))
}

#[tauri::command]
fn create_milestone(
    input: CreateMilestoneInput,
    state: State<'_, AppState>,
) -> Result<MilestoneResponse, String> {
    let title = validate_task_title(&input.title)?;
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .create_milestone(NewMilestone {
            id: Uuid::new_v4().to_string(),
            project_id: input.project_id,
            title,
            description: normalize_optional_text(input.description),
            status: input.status,
            target_date: normalize_optional_text(input.target_date),
        })
        .map(Into::into)
        .map_err(|error| format!("Unable to create milestone: {error}"))
}

#[tauri::command]
fn update_milestone_status(
    input: UpdateOutcomeStatusInput,
    state: State<'_, AppState>,
) -> Result<MilestoneResponse, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .update_milestone_status(&input.id, &input.status)
        .map_err(|error| format!("Unable to update milestone: {error}"))?
        .map(Into::into)
        .ok_or_else(|| "The milestone no longer exists.".into())
}

#[tauri::command]
fn list_epics(project_id: String, state: State<'_, AppState>) -> Result<Vec<EpicResponse>, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .list_epics(&project_id)
        .map(|items| items.into_iter().map(Into::into).collect())
        .map_err(|error| format!("Unable to load epics: {error}"))
}

#[tauri::command]
fn create_epic(input: CreateEpicInput, state: State<'_, AppState>) -> Result<EpicResponse, String> {
    let title = validate_task_title(&input.title)?;
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .create_epic(NewEpic {
            id: Uuid::new_v4().to_string(),
            project_id: input.project_id,
            milestone_id: normalize_optional_text(input.milestone_id),
            title,
            description: normalize_optional_text(input.description),
            status: input.status,
        })
        .map(Into::into)
        .map_err(|error| format!("Unable to create epic: {error}"))
}

#[tauri::command]
fn update_epic_status(
    input: UpdateOutcomeStatusInput,
    state: State<'_, AppState>,
) -> Result<EpicResponse, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .update_epic_status(&input.id, &input.status)
        .map_err(|error| format!("Unable to update epic: {error}"))?
        .map(Into::into)
        .ok_or_else(|| "The epic no longer exists.".into())
}

#[tauri::command]
fn get_project_progress(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<ProjectProgressResponse, String> {
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .project_progress(&project_id)
        .map(Into::into)
        .map_err(|error| format!("Unable to calculate project progress: {error}"))
}

#[tauri::command]
fn create_task(input: CreateTaskInput, state: State<'_, AppState>) -> Result<TaskResponse, String> {
    let title = validate_task_title(&input.title)?;
    let assigned_agent_id = normalize_optional_text(input.assigned_agent_id);
    let priority = parse_task_priority(&input.priority)?;
    let mut database = state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?;
    validate_assigned_agent(&database, assigned_agent_id.as_deref())?;
    database
        .create_task(NewTask {
            id: Uuid::new_v4().to_string(),
            project_id: input.project_id,
            title,
            description: normalize_optional_text(input.description),
            acceptance_criteria: normalize_task_list(
                input.acceptance_criteria,
                "Acceptance criteria",
                50,
                500,
            )?,
            implementation_notes: normalize_optional_text(input.implementation_notes),
            relevant_paths: normalize_task_list(input.relevant_paths, "Relevant paths", 50, 500)?,
            dependency_ids: normalize_task_list(input.dependency_ids, "Dependencies", 50, 120)?,
            assigned_agent_id,
            priority,
            milestone_id: normalize_optional_text(input.milestone_id),
            epic_id: normalize_optional_text(input.epic_id),
        })
        .map(Into::into)
        .map_err(|error| format!("Unable to create task: {error}"))
}

#[tauri::command]
fn update_task(input: UpdateTaskInput, state: State<'_, AppState>) -> Result<TaskResponse, String> {
    let title = validate_task_title(&input.title)?;
    let dependency_ids = normalize_task_list(input.dependency_ids, "Dependencies", 50, 120)?;
    if dependency_ids
        .iter()
        .any(|dependency_id| dependency_id == &input.id)
    {
        return Err("A task cannot depend on itself.".into());
    }
    let assigned_agent_id = normalize_optional_text(input.assigned_agent_id);
    let priority = parse_task_priority(&input.priority)?;
    let mut database = state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?;
    validate_assigned_agent(&database, assigned_agent_id.as_deref())?;
    database
        .update_task(
            &input.id,
            TaskUpdate {
                title,
                description: normalize_optional_text(input.description),
                acceptance_criteria: normalize_task_list(
                    input.acceptance_criteria,
                    "Acceptance criteria",
                    50,
                    500,
                )?,
                implementation_notes: normalize_optional_text(input.implementation_notes),
                relevant_paths: normalize_task_list(
                    input.relevant_paths,
                    "Relevant paths",
                    50,
                    500,
                )?,
                dependency_ids,
                assigned_agent_id,
                priority,
                milestone_id: normalize_optional_text(input.milestone_id),
                epic_id: normalize_optional_text(input.epic_id),
            },
        )
        .map_err(|error| format!("Unable to update task: {error}"))?
        .map(Into::into)
        .ok_or_else(|| "The task no longer exists.".into())
}

#[tauri::command]
fn delete_task(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut database = state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?;
    let task = database
        .get_task(&id)
        .map_err(|error| format!("Unable to load the task: {error}"))?
        .ok_or_else(|| "The task no longer exists.".to_owned())?;
    if task.worktree_path.is_some() {
        return Err("Remove the task worktree before deleting this task.".into());
    }
    let deleted = database
        .delete_task(&id)
        .map_err(|error| format!("Unable to delete task: {error}"))?;
    deleted
        .then_some(())
        .ok_or_else(|| "The task no longer exists.".into())
}

#[tauri::command]
fn move_task(input: MoveTaskInput, state: State<'_, AppState>) -> Result<TaskResponse, String> {
    let status =
        TaskStatus::parse(&input.status).ok_or_else(|| "Unknown task status.".to_owned())?;
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .move_task(&input.id, status, input.position)
        .map_err(|error| format!("Unable to move task: {error}"))?
        .map(Into::into)
        .ok_or_else(|| "The task no longer exists.".into())
}

fn save_project(
    state: &State<'_, AppState>,
    name: String,
    description: Option<String>,
    default_branch: String,
    workspace_path: String,
) -> Result<ProjectResponse, String> {
    let project = state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .create_project(NewProject {
            id: Uuid::new_v4().to_string(),
            name,
            description,
            default_branch,
            workspace_id: Uuid::new_v4().to_string(),
            worker_id: LOCAL_WORKER_ID.to_owned(),
            workspace_path: normalize_workspace_path(&workspace_path),
        })
        .map_err(|error| format!("Unable to save the project: {error}"))?;
    Ok(project.into())
}

fn workspace_path_for_project(
    state: &State<'_, AppState>,
    project_id: &str,
) -> Result<String, String> {
    let project = state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .get_project(project_id)
        .map_err(|error| format!("Unable to load the project workspace: {error}"))?
        .ok_or_else(|| "The project no longer exists.".to_owned())?;
    project
        .workspaces
        .into_iter()
        .find(|workspace| workspace.worker_id == LOCAL_WORKER_ID)
        .map(|workspace| workspace.path)
        .ok_or_else(|| "This project has no local workspace.".to_owned())
}

fn ensure_project_name_available(state: &State<'_, AppState>, name: &str) -> Result<(), String> {
    let exists = state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .project_name_exists(name)
        .map_err(|error| format!("Unable to validate the project name: {error}"))?;
    if exists {
        return Err(format!("A project named \"{name}\" is already registered."));
    }
    Ok(())
}

fn task_worktree_path(
    workspace_path: &str,
    project_id: &str,
    task_id: &str,
) -> Result<PathBuf, String> {
    let project_id = Uuid::parse_str(project_id)
        .map_err(|_| "The project has an invalid identifier for a task worktree.")?
        .to_string();
    let task_id = Uuid::parse_str(task_id)
        .map_err(|_| "The task has an invalid identifier for a task worktree.")?
        .to_string();
    let repository = GitService::inspect_repository(Path::new(workspace_path))
        .map_err(|error| format!("Unable to resolve the project repository: {error}"))?;
    let repository_root = PathBuf::from(normalize_workspace_path(&repository.root_path));
    let parent = repository_root.parent().ok_or_else(|| {
        "The project repository has no parent directory for task worktrees.".to_owned()
    })?;
    Ok(parent
        .join(".orchestr-worktrees")
        .join(project_id)
        .join(task_id))
}

fn task_branch_name(task_id: &str, title: &str) -> String {
    let slug = title
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let slug = slug.chars().take(40).collect::<String>();
    format!(
        "task/{task_id}-{}",
        if slug.is_empty() { "task" } else { &slug }
    )
}

fn validate_project_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("A project name is required.".into());
    }
    if name.chars().count() > 120 {
        return Err("Project names cannot exceed 120 characters.".into());
    }
    Ok(name.to_owned())
}

fn validate_task_title(title: &str) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("A task title is required.".into());
    }
    if title.chars().count() > 200 {
        return Err("Task titles cannot exceed 200 characters.".into());
    }
    Ok(title.to_owned())
}

fn default_task_priority() -> String {
    "normal".into()
}

fn parse_task_priority(value: &str) -> Result<TaskPriority, String> {
    TaskPriority::parse(value).ok_or_else(|| "Unknown task priority.".into())
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    })
}

fn normalize_task_list(
    values: Vec<String>,
    field_name: &str,
    max_items: usize,
    max_item_length: usize,
) -> Result<Vec<String>, String> {
    let mut normalized = Vec::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() || normalized.iter().any(|existing| existing == value) {
            continue;
        }
        if value.chars().count() > max_item_length {
            return Err(format!(
                "{field_name} entries cannot exceed {max_item_length} characters."
            ));
        }
        normalized.push(value.to_owned());
        if normalized.len() > max_items {
            return Err(format!(
                "{field_name} cannot contain more than {max_items} entries."
            ));
        }
    }
    Ok(normalized)
}

struct ValidatedAgentInput {
    name: String,
    provider: String,
    role: String,
    model: Option<String>,
    system_prompt: Option<String>,
    skills: Vec<String>,
    max_concurrent_tasks: i64,
}

fn validate_agent_input(input: CreateAgentInput) -> Result<ValidatedAgentInput, String> {
    let name = validate_required_field(input.name, "Agent name", 120)?;
    let role = validate_required_field(input.role, "Agent role", 120)?;
    let provider = input.provider.trim().to_ascii_lowercase();
    if !matches!(provider.as_str(), "codex" | "claude" | "gemini" | "custom") {
        return Err("Choose a supported agent provider.".into());
    }
    if !(1..=32).contains(&input.max_concurrent_tasks) {
        return Err("Agent concurrency must be between 1 and 32.".into());
    }
    Ok(ValidatedAgentInput {
        name,
        provider,
        role,
        model: normalize_optional_text(input.model),
        system_prompt: normalize_optional_text(input.system_prompt),
        skills: normalize_task_list(input.skills, "Skills", 50, 120)?,
        max_concurrent_tasks: input.max_concurrent_tasks,
    })
}

fn validate_required_field(
    value: String,
    field_name: &str,
    max_length: usize,
) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!("{field_name} is required."));
    }
    if value.chars().count() > max_length {
        return Err(format!(
            "{field_name} cannot exceed {max_length} characters."
        ));
    }
    Ok(value.to_owned())
}

fn validate_assigned_agent(database: &Database, agent_id: Option<&str>) -> Result<(), String> {
    if let Some(agent_id) = agent_id {
        if !database
            .agent_exists(agent_id)
            .map_err(|error| format!("Unable to validate the selected agent: {error}"))?
        {
            return Err("The selected agent no longer exists.".into());
        }
    }
    Ok(())
}

fn build_task_prompt(task: &Task, agent: &Agent) -> String {
    let mut prompt = format!(
        "You are {name}, acting as {role} in this repository. Implement the task below. \
         Inspect the existing project instructions and conventions before making changes. \
         Work only within this workspace. Do not mark the task complete in Orchestr; a human will review the result.\n\n# Task\n{title}",
        name = agent.name,
        role = agent.role,
        title = task.title,
    );
    if let Some(description) = &task.description {
        prompt.push_str(&format!("\n\n## Context\n{description}"));
    }
    if !task.acceptance_criteria.is_empty() {
        prompt.push_str("\n\n## Acceptance criteria");
        for criterion in &task.acceptance_criteria {
            prompt.push_str(&format!("\n- {criterion}"));
        }
    }
    if let Some(notes) = &task.implementation_notes {
        prompt.push_str(&format!("\n\n## Implementation notes\n{notes}"));
    }
    if !task.relevant_paths.is_empty() {
        prompt.push_str("\n\n## Relevant paths");
        for path in &task.relevant_paths {
            prompt.push_str(&format!("\n- {path}"));
        }
    }
    if !task.dependency_ids.is_empty() {
        prompt.push_str("\n\n## Related task dependencies");
        for dependency_id in &task.dependency_ids {
            prompt.push_str(&format!("\n- {dependency_id}"));
        }
    }
    if let Some(system_prompt) = &agent.system_prompt {
        prompt.push_str(&format!("\n\n## Agent instructions\n{system_prompt}"));
    }
    if !agent.skills.is_empty() {
        prompt.push_str("\n\n## Declared skills");
        for skill in &agent.skills {
            prompt.push_str(&format!("\n- {skill}"));
        }
    }
    prompt.push_str(
        "\n\n## Completion contract\n\
         Before finishing, inspect `git status`. After validating your work, commit every task-related change on this task branch with a clear commit message. Do not leave staged or unstaged task changes behind. Do not commit unrelated pre-existing changes.\n\
         If a normal `git add` or `git commit` fails because repository metadata is not writable, stop and report the exact error. Do not modify filesystem permissions, use an alternate Git index, or invoke low-level Git plumbing as a workaround.\n\
         When finished, summarize the changes, validation performed, and the commit hash.",
    );
    prompt
}

fn build_agent_review_prompt(
    task: &Task,
    reviewer: &Agent,
    review: &orchestr_git::TaskReview,
    runs: &[Run],
    validations: &[ValidationAttempt],
) -> String {
    let acceptance_criteria = if task.acceptance_criteria.is_empty() {
        "- No acceptance criteria recorded.".to_owned()
    } else {
        task.acceptance_criteria
            .iter()
            .map(|criterion| format!("- {criterion}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let commits = if review.commits.is_empty() {
        "- No task-branch commits found.".to_owned()
    } else {
        review
            .commits
            .iter()
            .map(|commit| format!("- {} {}", commit.short_hash, commit.subject))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let run_summary = runs
        .first()
        .map(|run| {
            format!(
                "Latest implementation run: {} (exit {:?}).",
                run.status.as_str(),
                run.exit_code
            )
        })
        .unwrap_or_else(|| "No implementation run is recorded.".into());
    let validation_summary = if validations.is_empty() {
        "No implementation validation attempts are recorded.".to_owned()
    } else {
        validations
            .iter()
            .map(|attempt| {
                format!(
                    "- {}: {}{}",
                    attempt.stage.as_str(),
                    attempt.status.as_str(),
                    attempt
                        .error
                        .as_deref()
                        .map(|error| format!(" ({error})"))
                        .unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "You are {name}, a logically separate technical reviewer. You must not change files, run destructive commands, or approve your own implementation. Review this task only from the supplied evidence.\n\n# Task\n{title}\n{description}\n\n# Acceptance criteria\n{acceptance_criteria}\n\n# Relevant paths\n{paths}\n\n# Project decisions\nNo persisted architecture-decision registry is available yet. Treat the task specification and repository instructions as authoritative.\n\n# Implementation run\n{run_summary}\n\n# Implementation validation\n{validation_summary}\n\n# Branch evidence\nBranch: {branch}\nBase: {base}\nCommits:\n{commits}\n\n# Diff\n{diff}\n\nDecide whether the implementation satisfies the acceptance criteria and is safe to send to normal integration. Return exactly these two single-line fields, with no alternative decision wording:\nORCHESTR_REVIEW_DECISION: approve | request_changes\nORCHESTR_REVIEW_NOTES: concise evidence-based review notes",
        name = reviewer.name,
        title = task.title,
        description = task.description.as_deref().unwrap_or("No description provided."),
        paths = if task.relevant_paths.is_empty() {
            "No relevant paths recorded.".to_owned()
        } else {
            task.relevant_paths.join("\n")
        },
        branch = review.branch,
        base = review.base_branch,
        diff = if review.diff.is_empty() { "No diff available." } else { &review.diff },
    )
}

fn parse_agent_review_decision(output: &str) -> Option<(AgentReviewDecision, String)> {
    let messages = output
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter_map(|event| {
            event
                .get("item")
                .filter(|item| {
                    item.get("type").and_then(serde_json::Value::as_str) == Some("agent_message")
                })
                .and_then(|item| item.get("text").and_then(serde_json::Value::as_str))
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    let source = messages.join("\n");
    if source.is_empty() {
        return None;
    }
    let decision = review_protocol_field(&source, "ORCHESTR_REVIEW_DECISION")?;
    let decision = decision
        .trim_matches(|character: char| matches!(character, '*' | '`' | '_' | '.' | ';'))
        .to_ascii_lowercase()
        .replace([' ', '-'], "_");
    let decision = match decision.as_str() {
        "approve" => AgentReviewDecision::Approve,
        "request_changes" | "changes_requested" => AgentReviewDecision::RequestChanges,
        _ => return None,
    };
    let notes = review_protocol_field(&source, "ORCHESTR_REVIEW_NOTES")
        .filter(|notes| !notes.is_empty())?
        .chars()
        .take(4000)
        .collect();
    Some((decision, notes))
}

fn review_protocol_field(source: &str, field: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let normalized = line.trim();
        let position = normalized.to_ascii_uppercase().find(field)?;
        let value = normalized[position + field.len()..]
            .trim_start_matches(|character: char| {
                character.is_whitespace() || matches!(character, ':' | '*' | '`' | '_' | '|')
            })
            .trim_end_matches(['*', '`', '|'])
            .trim();
        (!value.is_empty()).then_some(value.to_owned())
    })
}

fn repository_observation(path: &Path) -> Option<RepositoryObservation> {
    GitService::repository_details(path)
        .ok()
        .map(|details| RepositoryObservation {
            changed_files: details
                .changed_files
                .into_iter()
                .map(|file| (file.path, file.status))
                .collect(),
            latest_commit: details.summary.latest_commit.map(|commit| commit.hash),
        })
}

fn record_repository_events(
    database: &mut Database,
    run_id: &str,
    workspace_path: &Path,
    before: Option<&RepositoryObservation>,
) {
    let Some(after) = repository_observation(workspace_path) else {
        return;
    };
    let before_files = before.map(|observation| &observation.changed_files);
    for (path, status) in &after.changed_files {
        if before_files.and_then(|files| files.get(path)) != Some(status) {
            let _ = database.append_run_event(
                run_id,
                NewRunEvent {
                    kind: "file.modified".into(),
                    message: format!("{status} {path}"),
                    command: None,
                    file_path: Some(path.clone()),
                    exit_code: None,
                },
            );
        }
    }
    if after.latest_commit != before.and_then(|observation| observation.latest_commit.clone()) {
        if let Some(commit) = after.latest_commit {
            let _ = database.append_run_event(
                run_id,
                NewRunEvent {
                    kind: "commit.created".into(),
                    message: format!("Created commit {}", &commit[..commit.len().min(12)]),
                    command: None,
                    file_path: None,
                    exit_code: None,
                },
            );
        }
    }
}

/// Converts Windows' internal extended-length paths into the normal paths a
/// person expects to see and can copy into other tools. `canonicalize` may
/// return paths such as `\\?\C:\projects\orchestr` on Windows.
fn normalize_workspace_path(path: &str) -> String {
    #[cfg(windows)]
    {
        if let Some(unc_path) = path.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{unc_path}");
        }
        return path.strip_prefix(r"\\?\").unwrap_or(path).to_owned();
    }

    #[cfg(not(windows))]
    {
        path.to_owned()
    }
}

fn create_workspace_directory(parent_path: &str, directory_name: &str) -> Result<PathBuf, String> {
    let parent = PathBuf::from(parent_path);
    if !parent.is_dir() {
        return Err("Choose an existing parent directory for the new repository.".into());
    }

    let directory_name = directory_name.trim();
    if directory_name.is_empty()
        || directory_name == "."
        || directory_name == ".."
        || Path::new(directory_name).components().count() != 1
    {
        return Err("The repository folder must be a single folder name.".into());
    }

    let path = parent.join(directory_name);
    if path.exists() {
        return Err("A file or directory already exists with that repository folder name.".into());
    }
    fs::create_dir(&path)
        .map_err(|error| format!("Unable to create repository directory: {error}"))?;
    Ok(path)
}

fn open_directory(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = Command::new("explorer.exe");
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = Command::new("xdg-open");

    command.arg(path);
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Unable to open the task worktree in the file manager: {error}"))
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;
            let database_path = app_data_dir.join("orchestr.db");
            let mut database = Database::open(&database_path)?;
            database.recover_interrupted_integrations()?;
            let database = Arc::new(Mutex::new(database));
            let local_worker_runs = Arc::new(Mutex::new(HashMap::new()));

            app.manage(AppState {
                database: Arc::clone(&database),
                local_worker_runs: Arc::clone(&local_worker_runs),
            });
            dispatch_queued_task_runs(app.handle().clone(), database, local_worker_runs)
                .map_err(std::io::Error::other)?;
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_setting,
            set_setting,
            list_projects,
            get_project,
            delete_project,
            create_project,
            register_project,
            get_repository_details,
            get_repository_diff,
            get_repository_file_preview,
            get_local_worker_profile,
            run_local_diagnostic,
            get_codex_provider_status,
            start_codex_login,
            logout_codex,
            test_codex_connection,
            cancel_local_worker_run,
            cancel_queued_task_run,
            list_task_runs,
            recover_task_run,
            resolve_failed_run,
            get_flow_state,
            update_flow_limits,
            export_task_run_log,
            get_task_review,
            list_agent_reviews,
            start_agent_review,
            approve_task_review,
            request_task_changes,
            list_integration_attempts,
            retry_integration_attempt,
            retry_integration_cleanup,
            list_revert_attempts,
            revert_integration,
            integrate_next_task,
            list_validation_commands,
            create_validation_command,
            delete_validation_command,
            list_validation_attempts,
            get_project_health,
            rerun_integration_validation,
            cleanup_task_worktree,
            open_task_worktree,
            start_task_run,
            list_agents,
            create_agent,
            update_agent,
            delete_agent,
            list_tasks,
            list_milestones,
            create_milestone,
            update_milestone_status,
            list_epics,
            create_epic,
            update_epic_status,
            get_project_progress,
            create_task,
            update_task,
            delete_task,
            move_task
        ])
        .run(tauri::generate_context!())
        .expect("error while running Orchestr desktop application");
}

#[cfg(test)]
mod tests {
    use super::{
        build_task_prompt, format_run_log, normalize_workspace_path, parse_agent_review_decision,
    };
    use orchestr_db::{
        Agent, AgentReviewDecision, Run, RunEvent, RunOutput, RunStatus, Task, TaskPriority,
        TaskStatus,
    };

    #[cfg(windows)]
    #[test]
    fn normalizes_windows_extended_length_paths() {
        assert_eq!(
            normalize_workspace_path(r"\\?\C:\Users\konta\Projects\repo-test"),
            r"C:\Users\konta\Projects\repo-test"
        );
        assert_eq!(
            normalize_workspace_path(r"\\?\UNC\server\share\repo"),
            r"\\server\share\repo"
        );
    }

    #[test]
    fn task_prompt_requires_a_clean_committed_task_worktree() {
        let prompt = build_task_prompt(
            &Task {
                id: "task-1".into(),
                project_id: "project-1".into(),
                title: "Commit task work".into(),
                description: None,
                acceptance_criteria: Vec::new(),
                implementation_notes: None,
                relevant_paths: Vec::new(),
                dependency_ids: Vec::new(),
                assigned_agent_id: None,
                branch: Some("task/commit-task-work".into()),
                worktree_path: Some("C:/work/task-1".into()),
                priority: TaskPriority::Normal,
                blocked_reason: None,
                readiness_blocked: false,
                milestone_id: None,
                epic_id: None,
                status: TaskStatus::InProgress,
                position: 0,
                created_at: String::new(),
                updated_at: String::new(),
            },
            &Agent {
                id: "agent-1".into(),
                name: "Codex".into(),
                provider: "codex".into(),
                role: "Engineer".into(),
                model: None,
                system_prompt: None,
                skills: Vec::new(),
                max_concurrent_tasks: 1,
                created_at: String::new(),
                updated_at: String::new(),
            },
        );
        assert!(prompt.contains("commit every task-related change"));
        assert!(prompt.contains("Do not leave staged or unstaged task changes behind"));
        assert!(prompt.contains("Do not modify filesystem permissions"));
        assert!(prompt.contains("low-level Git plumbing"));
    }

    #[test]
    fn parses_a_structured_architect_decision_from_codex_output() {
        let output = r#"{"type":"item.completed","item":{"type":"agent_message","text":"ORCHESTR_REVIEW_DECISION: request_changes\nORCHESTR_REVIEW_NOTES: The empty state has no accessible label."}}"#;
        assert_eq!(
            parse_agent_review_decision(output),
            Some((
                AgentReviewDecision::RequestChanges,
                "The empty state has no accessible label.".into()
            ))
        );
        let markdown_output = r#"{"type":"item.completed","item":{"type":"agent_message","text":"**ORCHESTR_REVIEW_DECISION:** changes requested\n**ORCHESTR_REVIEW_NOTES:** Add an accessible empty-state label."}}"#;
        assert_eq!(
            parse_agent_review_decision(markdown_output),
            Some((
                AgentReviewDecision::RequestChanges,
                "Add an accessible empty-state label.".into()
            ))
        );
        assert!(parse_agent_review_decision("ORCHESTR_REVIEW_DECISION: maybe").is_none());
    }

    #[test]
    fn raw_run_log_includes_process_output_and_event_context() {
        let log = format_run_log(&Run {
            id: "run-1".into(),
            task_id: "task-1".into(),
            agent_id: "agent-1".into(),
            worker_id: "local".into(),
            status: RunStatus::Failed,
            started_at: "2026-08-23 12:00:00".into(),
            completed_at: Some("2026-08-23 12:01:00".into()),
            exit_code: Some(1),
            error: Some("Codex exited with an error.".into()),
            output: vec![RunOutput {
                stream: "stderr".into(),
                text: "raw provider output".into(),
                created_at: "2026-08-23 12:00:30".into(),
            }],
            events: vec![RunEvent {
                id: 1,
                kind: "command.completed".into(),
                message: "command failed".into(),
                command: Some("git commit".into()),
                file_path: None,
                exit_code: Some(1),
                created_at: "2026-08-23 12:00:31".into(),
            }],
        });

        assert!(log.contains("[stderr] raw provider output"));
        assert!(log.contains("command: git commit"));
        assert!(log.contains("exit: 1"));
    }
}
