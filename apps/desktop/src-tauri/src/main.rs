use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
};

use orchestr_db::{
    Agent, AgentUpdate, Database, NewAgent, NewProject, NewRun, NewRunEvent, NewTask, Project, Run,
    RunEvent, RunOutput, RunStatus, Task, TaskStatus, TaskUpdate, Workspace,
};
use orchestr_git::{GitService, RepositoryDetails};
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
    status: String,
    position: i64,
    created_at: String,
    updated_at: String,
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
            status: task.status.as_str().to_owned(),
            position: task.position,
            created_at: task.created_at,
            updated_at: task.updated_at,
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
fn create_project(
    input: CreateProjectInput,
    state: State<'_, AppState>,
) -> Result<ProjectResponse, String> {
    let name = validate_project_name(&input.name)?;
    let directory = create_workspace_directory(&input.parent_path, &input.directory_name)?;
    let repository = GitService::initialize_repository(&directory)
        .map_err(|error| format!("Unable to initialize the Git repository: {error}"))?;

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
fn start_task_run(
    task_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<StartedTaskRunResponse, String> {
    let (task, agent, workspace_path) = {
        let database = state
            .database
            .lock()
            .map_err(|_| "The local run store is unavailable.".to_owned())?;
        let task = database
            .get_task(&task_id)
            .map_err(|error| format!("Unable to load task for execution: {error}"))?
            .ok_or_else(|| "The task no longer exists.".to_owned())?;
        if task.status != TaskStatus::Todo {
            return Err(
                "Only Todo tasks can be started. Move the task back to Todo to run it again."
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
        (task, agent, workspace_path)
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

    let request = CodexProvider
        .execution_request(AgentRunInput {
            model: agent.model.clone(),
            prompt: build_task_prompt(&task, &agent),
            working_directory: PathBuf::from(&workspace_path),
        })
        .map_err(|error| format!("Unable to prepare the Codex task: {error}"))?;
    let repository_before = repository_observation(Path::new(&workspace_path));
    let run_id = Uuid::new_v4().to_string();
    let (persisted_run, updated_task) = state
        .database
        .lock()
        .map_err(|_| "The local run store is unavailable.".to_owned())?
        .start_run(NewRun {
            id: run_id.clone(),
            task_id: task.id.clone(),
            agent_id: agent.id,
            worker_id: LOCAL_WORKER_ID.to_owned(),
        })
        .map_err(|error| format!("Unable to start the task run: {error}"))?;

    let run = match LocalWorker::start(request) {
        Ok(run) => run,
        Err(error) => {
            let _ = state.database.lock().ok().and_then(|mut database| {
                database
                    .finish_run(&run_id, RunStatus::Failed, None, Some(&error.to_string()))
                    .ok()
            });
            return Err(format!("Unable to start Codex for this task: {error}"));
        }
    };

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
    let database = Arc::clone(&state.database);
    let event_run_id = run_id.clone();
    thread::spawn(move || {
        for output in run.output {
            let event = CodexProvider::execution_event(&output.text);
            let text = event.message.clone();
            if let Ok(mut database) = database.lock() {
                let _ = database.append_run_event(
                    &event_run_id,
                    NewRunEvent {
                        kind: event.kind,
                        message: event.message,
                        command: event.command,
                        file_path: None,
                        exit_code: event.exit_code,
                    },
                );
            }
            let _ = app.emit(
                "worker://run-event",
                WorkerRunEvent {
                    run_id: event_run_id.clone(),
                    kind: "output".into(),
                    stream: Some(output.stream),
                    text: Some(text),
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
        let (status, kind, text, exit_code) = match result {
            Ok(exit_status) if cancelled => (
                RunStatus::Cancelled,
                "cancelled",
                Some("Codex task cancelled.".into()),
                exit_status.code(),
            ),
            Ok(exit_status) if exit_status.success() => {
                (RunStatus::Completed, "completed", None, exit_status.code())
            }
            Ok(exit_status) => (
                RunStatus::Failed,
                "failed",
                Some("Codex exited with an error.".into()),
                exit_status.code(),
            ),
            Err(error) => (RunStatus::Failed, "failed", Some(error.to_string()), None),
        };
        if let Ok(mut database) = database.lock() {
            record_repository_events(
                &mut database,
                &event_run_id,
                Path::new(&workspace_path),
                repository_before.as_ref(),
            );
            let _ = database.finish_run(&event_run_id, status, exit_code, text.as_deref());
        }
        let _ = app.emit(
            "worker://run-event",
            WorkerRunEvent {
                run_id: event_run_id,
                kind: kind.into(),
                stream: None,
                text,
                exit_code,
            },
        );
    });

    Ok(StartedTaskRunResponse {
        run: persisted_run.into(),
        task: updated_task.into(),
    })
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
fn create_task(input: CreateTaskInput, state: State<'_, AppState>) -> Result<TaskResponse, String> {
    let title = validate_task_title(&input.title)?;
    let assigned_agent_id = normalize_optional_text(input.assigned_agent_id);
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
            },
        )
        .map_err(|error| format!("Unable to update task: {error}"))?
        .map(Into::into)
        .ok_or_else(|| "The task no longer exists.".into())
}

#[tauri::command]
fn delete_task(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let deleted = state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
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
    prompt.push_str("\n\nWhen finished, summarize the changes and validation performed.");
    prompt
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

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;
            let database_path = app_data_dir.join("orchestr.db");
            let database = Database::open(&database_path)?;

            app.manage(AppState {
                database: Arc::new(Mutex::new(database)),
                local_worker_runs: Arc::new(Mutex::new(HashMap::new())),
            });
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_setting,
            set_setting,
            list_projects,
            get_project,
            create_project,
            register_project,
            get_repository_details,
            get_repository_diff,
            get_local_worker_profile,
            run_local_diagnostic,
            get_codex_provider_status,
            start_codex_login,
            logout_codex,
            test_codex_connection,
            cancel_local_worker_run,
            list_task_runs,
            start_task_run,
            list_agents,
            create_agent,
            update_agent,
            delete_agent,
            list_tasks,
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
    use super::normalize_workspace_path;

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
}
