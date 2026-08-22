use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
};

use orchestr_db::{
    Database, NewProject, NewTask, Project, Task, TaskStatus, TaskUpdate, Workspace,
};
use orchestr_git::{GitService, RepositoryDetails};
use orchestr_worker::{LocalWorker, OutputStream, ProcessRequest, WorkerHandle, WorkerProfile};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

const LOCAL_WORKER_ID: &str = "local";

struct AppState {
    database: Mutex<Database>,
    local_worker_runs: Arc<Mutex<HashMap<String, ActiveLocalRun>>>,
}

struct ActiveLocalRun {
    handle: WorkerHandle,
    cancel_requested: bool,
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
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateTaskInput {
    id: String,
    title: String,
    description: Option<String>,
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
    status: String,
    position: i64,
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
            status: task.status.as_str().to_owned(),
            position: task.position,
            created_at: task.created_at,
            updated_at: task.updated_at,
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
    let run = LocalWorker::start(ProcessRequest {
        program: "git".into(),
        arguments: vec!["--version".into()],
        working_directory: None,
    })
    .map_err(|error| format!("Unable to start the local worker diagnostic: {error}"))?;
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
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .create_task(NewTask {
            id: Uuid::new_v4().to_string(),
            project_id: input.project_id,
            title,
            description: normalize_optional_text(input.description),
        })
        .map(Into::into)
        .map_err(|error| format!("Unable to create task: {error}"))
}

#[tauri::command]
fn update_task(input: UpdateTaskInput, state: State<'_, AppState>) -> Result<TaskResponse, String> {
    let title = validate_task_title(&input.title)?;
    state
        .database
        .lock()
        .map_err(|_| "The local project store is unavailable.".to_owned())?
        .update_task(
            &input.id,
            TaskUpdate {
                title,
                description: normalize_optional_text(input.description),
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
                database: Mutex::new(database),
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
            cancel_local_worker_run,
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
