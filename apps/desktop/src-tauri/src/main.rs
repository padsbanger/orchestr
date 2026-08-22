use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use orchestr_db::{Database, NewProject, Project, Workspace};
use orchestr_git::GitService;
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};
use uuid::Uuid;

const LOCAL_WORKER_ID: &str = "local";

struct AppState {
    database: Mutex<Database>,
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
            path: workspace.path,
            created_at: workspace.created_at,
            updated_at: workspace.updated_at,
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
            workspace_path,
        })
        .map_err(|error| format!("Unable to save the project: {error}"))?;
    Ok(project.into())
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

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    })
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
            register_project
        ])
        .run(tauri::generate_context!())
        .expect("error while running Orchestr desktop application");
}
