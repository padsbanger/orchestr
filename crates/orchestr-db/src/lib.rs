//! SQLite-backed local metadata storage and migrations.
//!
//! This crate owns the persistence boundary. UI and Tauri command handlers use
//! repositories here instead of issuing SQLite statements themselves.

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use rusqlite::{params, Connection, OptionalExtension, Result};

const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../migrations/0001_settings.sql")),
    (2, include_str!("../migrations/0002_projects.sql")),
    (3, include_str!("../migrations/0003_tasks.sql")),
    (
        4,
        include_str!("../migrations/0004_task_specifications.sql"),
    ),
    (5, include_str!("../migrations/0005_agents.sql")),
    (6, include_str!("../migrations/0006_runs.sql")),
    (7, include_str!("../migrations/0007_run_events.sql")),
    (8, include_str!("../migrations/0008_task_worktrees.sql")),
    (9, include_str!("../migrations/0009_integration_queue.sql")),
    (10, include_str!("../migrations/0010_quality_gates.sql")),
    (11, include_str!("../migrations/0011_task_readiness.sql")),
];

pub struct Database {
    connection: Connection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub default_branch: String,
    pub created_at: String,
    pub updated_at: String,
    pub workspaces: Vec<Workspace>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub id: String,
    pub project_id: String,
    pub worker_id: String,
    pub path: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct NewProject {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub default_branch: String,
    pub workspace_id: String,
    pub worker_id: String,
    pub workspace_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectDeletion {
    Deleted,
    NotFound,
    HasAttachedWorktrees,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Backlog,
    Ready,
    InProgress,
    Review,
    Approved,
    Integrating,
    Blocked,
    Done,
}

impl TaskStatus {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "backlog" => Some(Self::Backlog),
            "ready" => Some(Self::Ready),
            "in_progress" => Some(Self::InProgress),
            "review" => Some(Self::Review),
            "approved" => Some(Self::Approved),
            "integrating" => Some(Self::Integrating),
            "blocked" => Some(Self::Blocked),
            "done" => Some(Self::Done),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Backlog => "backlog",
            Self::Ready => "ready",
            Self::InProgress => "in_progress",
            Self::Review => "review",
            Self::Approved => "approved",
            Self::Integrating => "integrating",
            Self::Blocked => "blocked",
            Self::Done => "done",
        }
    }

    fn from_database(value: String) -> Result<Self> {
        Self::parse(&value).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Text,
                format!("Unknown task status: {value}").into(),
            )
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPriority {
    Critical,
    High,
    Normal,
    Low,
}

impl TaskPriority {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "critical" => Some(Self::Critical),
            "high" => Some(Self::High),
            "normal" => Some(Self::Normal),
            "low" => Some(Self::Low),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Normal => "normal",
            Self::Low => "low",
        }
    }

    fn from_database(value: String) -> Result<Self> {
        Self::parse(&value).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                12,
                rusqlite::types::Type::Text,
                format!("Unknown task priority: {value}").into(),
            )
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationStatus {
    Queued,
    Integrating,
    Conflict,
    Merged,
    Failed,
}

/// The point in the delivery pipeline at which a validation command runs.
/// Commands are represented as program/argument arrays so the worker never has
/// to evaluate a project supplied shell string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationStage {
    Implementation,
    Integration,
}

impl ValidationStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Implementation => "implementation",
            Self::Integration => "integration",
        }
    }

    fn from_database(value: String) -> Result<Self> {
        match value.as_str() {
            "implementation" => Ok(Self::Implementation),
            "integration" => Ok(Self::Integration),
            _ => Err(rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Text,
                format!("Unknown validation stage: {value}").into(),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationStatus {
    Running,
    Passed,
    Failed,
    Cancelled,
}

impl ValidationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn from_database(value: String) -> Result<Self> {
        match value.as_str() {
            "running" => Ok(Self::Running),
            "passed" => Ok(Self::Passed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Text,
                format!("Unknown validation status: {value}").into(),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectHealthStatus {
    Unknown,
    Healthy,
    Degraded,
    Broken,
}

impl ProjectHealthStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Broken => "broken",
        }
    }

    fn from_database(value: String) -> Result<Self> {
        match value.as_str() {
            "unknown" => Ok(Self::Unknown),
            "healthy" => Ok(Self::Healthy),
            "degraded" => Ok(Self::Degraded),
            "broken" => Ok(Self::Broken),
            _ => Err(rusqlite::Error::FromSqlConversionFailure(
                1,
                rusqlite::types::Type::Text,
                format!("Unknown project health status: {value}").into(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationCommand {
    pub id: String,
    pub project_id: String,
    pub stage: ValidationStage,
    pub name: String,
    pub program: String,
    pub arguments: Vec<String>,
    pub position: i64,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

pub struct NewValidationCommand {
    pub id: String,
    pub project_id: String,
    pub stage: ValidationStage,
    pub name: String,
    pub program: String,
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationAttempt {
    pub id: String,
    pub project_id: String,
    pub task_id: Option<String>,
    pub integration_attempt_id: Option<String>,
    pub stage: ValidationStage,
    pub status: ValidationStatus,
    pub error: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub events: Vec<ValidationEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationEvent {
    pub id: i64,
    pub command_id: Option<String>,
    pub kind: String,
    pub message: String,
    pub stream: Option<String>,
    pub exit_code: Option<i32>,
    pub created_at: String,
}

pub struct NewValidationEvent {
    pub command_id: Option<String>,
    pub kind: String,
    pub message: String,
    pub stream: Option<String>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectHealth {
    pub project_id: String,
    pub status: ProjectHealthStatus,
    pub last_validation_attempt_id: Option<String>,
    pub last_successful_validation_at: Option<String>,
    pub last_integration_at: Option<String>,
    pub failing_gate: Option<String>,
    pub updated_at: String,
}

impl IntegrationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Integrating => "integrating",
            Self::Conflict => "conflict",
            Self::Merged => "merged",
            Self::Failed => "failed",
        }
    }

    fn from_database(value: String) -> Result<Self> {
        match value.as_str() {
            "queued" => Ok(Self::Queued),
            "integrating" => Ok(Self::Integrating),
            "conflict" => Ok(Self::Conflict),
            "merged" => Ok(Self::Merged),
            "failed" => Ok(Self::Failed),
            _ => Err(rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                format!("Unknown integration status: {value}").into(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationAttempt {
    pub id: String,
    pub task_id: String,
    pub source_branch: String,
    pub target_branch: String,
    pub status: IntegrationStatus,
    pub queue_position: i64,
    pub merge_commit: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub acceptance_criteria: Vec<String>,
    pub implementation_notes: Option<String>,
    pub relevant_paths: Vec<String>,
    pub dependency_ids: Vec<String>,
    pub assigned_agent_id: Option<String>,
    pub branch: Option<String>,
    pub worktree_path: Option<String>,
    pub priority: TaskPriority,
    pub blocked_reason: Option<String>,
    pub status: TaskStatus,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

pub struct NewTask {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub acceptance_criteria: Vec<String>,
    pub implementation_notes: Option<String>,
    pub relevant_paths: Vec<String>,
    pub dependency_ids: Vec<String>,
    pub assigned_agent_id: Option<String>,
    pub priority: TaskPriority,
}

pub struct TaskUpdate {
    pub title: String,
    pub description: Option<String>,
    pub acceptance_criteria: Vec<String>,
    pub implementation_notes: Option<String>,
    pub relevant_paths: Vec<String>,
    pub dependency_ids: Vec<String>,
    pub assigned_agent_id: Option<String>,
    pub priority: TaskPriority,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub role: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub skills: Vec<String>,
    pub max_concurrent_tasks: i64,
    pub created_at: String,
    pub updated_at: String,
}

pub struct NewAgent {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub role: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub skills: Vec<String>,
    pub max_concurrent_tasks: i64,
}

pub struct AgentUpdate {
    pub name: String,
    pub provider: String,
    pub role: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub skills: Vec<String>,
    pub max_concurrent_tasks: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Queued,
    Running,
    Failed,
    Completed,
    Cancelled,
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Failed => "failed",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }

    fn from_database(value: String) -> Result<Self> {
        match value.as_str() {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "failed" => Ok(Self::Failed),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                format!("Unknown run status: {value}").into(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutput {
    pub stream: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub id: String,
    pub task_id: String,
    pub agent_id: String,
    pub worker_id: String,
    pub status: RunStatus,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    pub output: Vec<RunOutput>,
    pub events: Vec<RunEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunEvent {
    pub id: i64,
    pub kind: String,
    pub message: String,
    pub command: Option<String>,
    pub file_path: Option<String>,
    pub exit_code: Option<i32>,
    pub created_at: String,
}

pub struct NewRunEvent {
    pub kind: String,
    pub message: String,
    pub command: Option<String>,
    pub file_path: Option<String>,
    pub exit_code: Option<i32>,
}

pub struct NewRun {
    pub id: String,
    pub task_id: String,
    pub agent_id: String,
    pub worker_id: String,
}

impl Database {
    pub fn open(database_path: &Path) -> Result<Self> {
        let connection = Connection::open(database_path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&connection)?;
        Ok(Self { connection })
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        self.connection
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
    }

    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        self.connection.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let projects = {
            let mut statement = self.connection.prepare(
                "SELECT id, name, description, default_branch, created_at, updated_at
                 FROM projects ORDER BY updated_at DESC, name COLLATE NOCASE ASC",
            )?;
            let records = statement
                .query_map([], |row| {
                    Ok(Project {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        default_branch: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                        workspaces: Vec::new(),
                    })
                })?
                .collect::<Result<Vec<_>>>()?;
            records
        };

        projects
            .into_iter()
            .map(|mut project| {
                project.workspaces = self.list_workspaces(&project.id)?;
                Ok(project)
            })
            .collect()
    }

    pub fn project_name_exists(&self, name: &str) -> Result<bool> {
        self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE name = ?1 COLLATE NOCASE)",
            [name],
            |row| row.get(0),
        )
    }

    pub fn create_project(&mut self, new_project: NewProject) -> Result<Project> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO projects (id, name, description, default_branch)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                new_project.id,
                new_project.name,
                new_project.description,
                new_project.default_branch
            ],
        )?;
        transaction.execute(
            "INSERT INTO workspaces (id, project_id, worker_id, path)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                new_project.workspace_id,
                new_project.id,
                new_project.worker_id,
                new_project.workspace_path
            ],
        )?;
        transaction.execute(
            "INSERT INTO project_health (project_id) VALUES (?1)",
            [&new_project.id],
        )?;
        transaction.commit()?;

        self.project_by_id(&new_project.id)?
            .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn get_project(&self, id: &str) -> Result<Option<Project>> {
        self.project_by_id(id)
    }

    pub fn get_task(&self, id: &str) -> Result<Option<Task>> {
        self.task_by_id(id)
    }

    pub fn get_agent(&self, id: &str) -> Result<Option<Agent>> {
        self.agent_by_id(id)
    }

    pub fn list_validation_commands(
        &self,
        project_id: &str,
        stage: ValidationStage,
    ) -> Result<Vec<ValidationCommand>> {
        let mut statement = self.connection.prepare(
            "SELECT id, project_id, stage, name, program, arguments, position, enabled, created_at, updated_at
             FROM validation_commands WHERE project_id = ?1 AND stage = ?2
             ORDER BY position ASC, created_at ASC",
        )?;
        let commands = statement
            .query_map(
                params![project_id, stage.as_str()],
                validation_command_from_row,
            )?
            .collect::<Result<Vec<_>>>()?;
        Ok(commands)
    }

    pub fn create_validation_command(
        &mut self,
        command: NewValidationCommand,
    ) -> Result<ValidationCommand> {
        let arguments = encode_string_list(&command.arguments)?;
        let position: i64 = self.connection.query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM validation_commands
             WHERE project_id = ?1 AND stage = ?2",
            params![command.project_id, command.stage.as_str()],
            |row| row.get(0),
        )?;
        self.connection.execute(
            "INSERT INTO validation_commands (id, project_id, stage, name, program, arguments, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                command.id,
                command.project_id,
                command.stage.as_str(),
                command.name,
                command.program,
                arguments,
                position,
            ],
        )?;
        self.validation_command_by_id(&command.id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn delete_validation_command(&mut self, id: &str) -> Result<bool> {
        Ok(self
            .connection
            .execute("DELETE FROM validation_commands WHERE id = ?1", [id])?
            > 0)
    }

    pub fn get_project_health(&self, project_id: &str) -> Result<ProjectHealth> {
        self.connection.query_row(
            "SELECT project_id, status, last_validation_attempt_id, last_successful_validation_at,
                    last_integration_at, failing_gate, updated_at
             FROM project_health WHERE project_id = ?1",
            [project_id],
            project_health_from_row,
        )
    }

    pub fn start_validation_attempt(
        &mut self,
        id: &str,
        project_id: &str,
        task_id: Option<&str>,
        integration_attempt_id: Option<&str>,
        stage: ValidationStage,
    ) -> Result<ValidationAttempt> {
        self.connection.execute(
            "INSERT INTO validation_attempts (id, project_id, task_id, integration_attempt_id, stage, status)
             VALUES (?1, ?2, ?3, ?4, ?5, 'running')",
            params![id, project_id, task_id, integration_attempt_id, stage.as_str()],
        )?;
        self.validation_attempt_by_id(id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn append_validation_event(
        &mut self,
        attempt_id: &str,
        event: NewValidationEvent,
    ) -> Result<()> {
        self.connection.execute(
            "INSERT INTO validation_events (validation_attempt_id, validation_command_id, kind, message, stream, exit_code)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![attempt_id, event.command_id, event.kind, event.message, event.stream, event.exit_code],
        )?;
        Ok(())
    }

    pub fn finish_validation_attempt(
        &mut self,
        id: &str,
        status: ValidationStatus,
        error: Option<&str>,
    ) -> Result<Option<ValidationAttempt>> {
        self.connection.execute(
            "UPDATE validation_attempts SET status = ?1, error = ?2, completed_at = CURRENT_TIMESTAMP WHERE id = ?3 AND status = 'running'",
            params![status.as_str(), error, id],
        )?;
        self.validation_attempt_by_id(id)
    }

    pub fn list_validation_attempts(
        &self,
        project_id: &str,
        limit: usize,
    ) -> Result<Vec<ValidationAttempt>> {
        let mut statement = self.connection.prepare(
            "SELECT id, project_id, task_id, integration_attempt_id, stage, status, error, started_at, completed_at
             FROM validation_attempts WHERE project_id = ?1 ORDER BY started_at DESC LIMIT ?2",
        )?;
        let ids = statement
            .query_map(params![project_id, limit as i64], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>>>()?;
        ids.into_iter()
            .map(|id| {
                self.validation_attempt_by_id(&id)?
                    .ok_or(rusqlite::Error::QueryReturnedNoRows)
            })
            .collect()
    }

    pub fn record_project_validation(
        &mut self,
        project_id: &str,
        attempt_id: &str,
        status: ValidationStatus,
        failing_gate: Option<&str>,
        integration_completed: bool,
    ) -> Result<()> {
        let health = match status {
            ValidationStatus::Passed => ProjectHealthStatus::Healthy,
            ValidationStatus::Failed | ValidationStatus::Cancelled => ProjectHealthStatus::Broken,
            ValidationStatus::Running => ProjectHealthStatus::Unknown,
        };
        self.connection.execute(
            "INSERT INTO project_health (project_id, status, last_validation_attempt_id, last_successful_validation_at, last_integration_at, failing_gate)
             VALUES (?1, ?2, ?3,
                 CASE WHEN ?2 = 'healthy' THEN CURRENT_TIMESTAMP ELSE NULL END,
                 CASE WHEN ?4 THEN CURRENT_TIMESTAMP ELSE NULL END, ?5)
             ON CONFLICT(project_id) DO UPDATE SET
                 status = excluded.status,
                 last_validation_attempt_id = excluded.last_validation_attempt_id,
                 last_successful_validation_at = CASE WHEN excluded.status = 'healthy' THEN CURRENT_TIMESTAMP ELSE project_health.last_successful_validation_at END,
                 last_integration_at = CASE WHEN ?4 THEN CURRENT_TIMESTAMP ELSE project_health.last_integration_at END,
                 failing_gate = excluded.failing_gate,
                 updated_at = CURRENT_TIMESTAMP",
            params![project_id, health.as_str(), attempt_id, integration_completed, failing_gate],
        )?;
        Ok(())
    }

    pub fn list_tasks(&self, project_id: &str) -> Result<Vec<Task>> {
        let mut statement = self.connection.prepare(
            "SELECT id, project_id, title, description, acceptance_criteria, implementation_notes,
                    relevant_paths, dependency_ids, assigned_agent_id, branch, worktree_path, status, position, created_at, updated_at
             FROM tasks WHERE project_id = ?1
             ORDER BY CASE status
                 WHEN 'backlog' THEN 0
                 WHEN 'todo' THEN 1
                 WHEN 'in_progress' THEN 2
                 WHEN 'review' THEN 3
                 WHEN 'approved' THEN 4
                 WHEN 'integrating' THEN 5
                 WHEN 'blocked' THEN 6
                 WHEN 'done' THEN 7
             END, position ASC",
        )?;
        let records = statement
            .query_map([project_id], task_from_row)?
            .collect::<Result<Vec<_>>>()?;
        Ok(records)
    }

    pub fn create_task(&mut self, new_task: NewTask) -> Result<Task> {
        let transaction = self.connection.transaction()?;
        let position: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM tasks
             WHERE project_id = ?1 AND status = 'backlog'",
            [&new_task.project_id],
            |row| row.get(0),
        )?;
        let acceptance_criteria = encode_string_list(&new_task.acceptance_criteria)?;
        let relevant_paths = encode_string_list(&new_task.relevant_paths)?;
        let dependency_ids = encode_string_list(&new_task.dependency_ids)?;
        transaction.execute(
            "INSERT INTO tasks (id, project_id, title, description, acceptance_criteria,
                                implementation_notes, relevant_paths, dependency_ids, assigned_agent_id, status, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'backlog', ?10)",
            params![
                new_task.id,
                new_task.project_id,
                new_task.title,
                new_task.description,
                acceptance_criteria,
                new_task.implementation_notes,
                relevant_paths,
                dependency_ids,
                new_task.assigned_agent_id,
                position
            ],
        )?;
        transaction.commit()?;
        self.task_by_id(&new_task.id)?
            .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn update_task(&mut self, id: &str, update: TaskUpdate) -> Result<Option<Task>> {
        let acceptance_criteria = encode_string_list(&update.acceptance_criteria)?;
        let relevant_paths = encode_string_list(&update.relevant_paths)?;
        let dependency_ids = encode_string_list(&update.dependency_ids)?;
        let changed = self.connection.execute(
            "UPDATE tasks SET title = ?1, description = ?2, acceptance_criteria = ?3,
                              implementation_notes = ?4, relevant_paths = ?5, dependency_ids = ?6,
                              assigned_agent_id = ?7, updated_at = CURRENT_TIMESTAMP WHERE id = ?8",
            params![
                update.title,
                update.description,
                acceptance_criteria,
                update.implementation_notes,
                relevant_paths,
                dependency_ids,
                update.assigned_agent_id,
                id
            ],
        )?;
        if changed == 0 {
            return Ok(None);
        }
        self.task_by_id(id)
    }

    pub fn delete_task(&mut self, id: &str) -> Result<bool> {
        let Some((project_id, status)) = self.task_location(id)? else {
            return Ok(false);
        };
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM tasks WHERE id = ?1", [id])?;
        normalize_positions(&transaction, &project_id, status)?;
        transaction.commit()?;
        Ok(true)
    }

    pub fn list_agents(&self) -> Result<Vec<Agent>> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, provider, role, model, system_prompt, skills, max_concurrent_tasks, created_at, updated_at
             FROM agents ORDER BY name COLLATE NOCASE ASC",
        )?;
        let agents = statement
            .query_map([], agent_from_row)?
            .collect::<Result<Vec<_>>>()?;
        Ok(agents)
    }

    pub fn create_agent(&mut self, new_agent: NewAgent) -> Result<Agent> {
        let skills = encode_string_list(&new_agent.skills)?;
        self.connection.execute(
            "INSERT INTO agents (id, name, provider, role, model, system_prompt, skills, max_concurrent_tasks)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![new_agent.id, new_agent.name, new_agent.provider, new_agent.role, new_agent.model, new_agent.system_prompt, skills, new_agent.max_concurrent_tasks],
        )?;
        self.agent_by_id(&new_agent.id)?
            .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn update_agent(&mut self, id: &str, update: AgentUpdate) -> Result<Option<Agent>> {
        let skills = encode_string_list(&update.skills)?;
        let changed = self.connection.execute(
            "UPDATE agents SET name = ?1, provider = ?2, role = ?3, model = ?4, system_prompt = ?5,
                               skills = ?6, max_concurrent_tasks = ?7, updated_at = CURRENT_TIMESTAMP WHERE id = ?8",
            params![update.name, update.provider, update.role, update.model, update.system_prompt, skills, update.max_concurrent_tasks, id],
        )?;
        if changed == 0 {
            return Ok(None);
        }
        self.agent_by_id(id)
    }

    pub fn delete_agent(&mut self, id: &str) -> Result<bool> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "UPDATE tasks SET assigned_agent_id = NULL, updated_at = CURRENT_TIMESTAMP WHERE assigned_agent_id = ?1",
            [id],
        )?;
        let deleted = transaction.execute("DELETE FROM agents WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(deleted > 0)
    }

    pub fn delete_project(&mut self, id: &str) -> Result<ProjectDeletion> {
        let transaction = self.connection.transaction()?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
            [id],
            |row| row.get(0),
        )?;
        if !exists {
            return Ok(ProjectDeletion::NotFound);
        }
        let has_worktree: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE project_id = ?1 AND worktree_path IS NOT NULL)",
            [id],
            |row| row.get(0),
        )?;
        if has_worktree {
            return Ok(ProjectDeletion::HasAttachedWorktrees);
        }
        transaction.execute("DELETE FROM tasks WHERE project_id = ?1", [id])?;
        transaction.execute("DELETE FROM workspaces WHERE project_id = ?1", [id])?;
        transaction.execute("DELETE FROM projects WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(ProjectDeletion::Deleted)
    }

    pub fn agent_exists(&self, id: &str) -> Result<bool> {
        self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM agents WHERE id = ?1)",
            [id],
            |row| row.get(0),
        )
    }

    pub fn list_runs_for_task(&self, task_id: &str) -> Result<Vec<Run>> {
        let mut statement = self.connection.prepare(
            "SELECT id, task_id, agent_id, worker_id, status, started_at, completed_at, exit_code, error
             FROM runs WHERE task_id = ?1 ORDER BY started_at DESC, id DESC",
        )?;
        let runs = statement
            .query_map([task_id], run_from_row)?
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .map(|mut run| {
                run.output = self.list_run_output(&run.id)?;
                run.events = self.list_run_events(&run.id)?;
                Ok(run)
            })
            .collect();
        runs
    }

    pub fn start_run(&mut self, new_run: NewRun) -> Result<(Run, Task)> {
        let transaction = self.connection.transaction()?;
        let (project_id, task_status, assigned_agent_id): (String, String, Option<String>) =
            transaction.query_row(
                "SELECT project_id, status, assigned_agent_id FROM tasks WHERE id = ?1",
                [&new_run.task_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
        if task_status != TaskStatus::Todo.as_str()
            || assigned_agent_id.as_deref() != Some(new_run.agent_id.as_str())
        {
            return Err(rusqlite::Error::InvalidQuery);
        }

        transaction.execute(
            "INSERT INTO runs (id, task_id, agent_id, worker_id, status) VALUES (?1, ?2, ?3, ?4, 'running')",
            params![new_run.id, new_run.task_id, new_run.agent_id, new_run.worker_id],
        )?;
        transaction.execute(
            "INSERT INTO run_events (run_id, kind, message) VALUES (?1, 'run.started', 'Agent run started.')",
            [&new_run.id],
        )?;
        move_task_in_transaction(
            &transaction,
            &new_run.task_id,
            &project_id,
            TaskStatus::Todo,
            TaskStatus::InProgress,
            usize::MAX,
        )?;
        transaction.commit()?;

        let run = self
            .get_run(&new_run.id)?
            .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;
        let task = self
            .task_by_id(&new_run.task_id)?
            .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;
        Ok((run, task))
    }

    pub fn assign_task_worktree(
        &mut self,
        id: &str,
        branch: &str,
        worktree_path: &str,
    ) -> Result<Option<Task>> {
        let changed = self.connection.execute(
            "UPDATE tasks SET branch = ?1, worktree_path = ?2, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?3 AND status = 'todo' AND worktree_path IS NULL",
            params![branch, worktree_path, id],
        )?;
        if changed == 0 {
            return Ok(None);
        }
        self.task_by_id(id)
    }

    pub fn release_task_worktree(&mut self, id: &str) -> Result<Option<Task>> {
        let transaction = self.connection.transaction()?;
        let worktree_path: Option<String> = transaction
            .query_row(
                "SELECT worktree_path FROM tasks WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()?;
        if worktree_path.is_none() {
            return Ok(None);
        }
        let has_active_run: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM runs WHERE task_id = ?1 AND status = 'running')",
            [id],
            |row| row.get(0),
        )?;
        if has_active_run {
            return Err(rusqlite::Error::InvalidQuery);
        }
        transaction.execute(
            "UPDATE tasks SET worktree_path = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
            [id],
        )?;
        transaction.commit()?;
        self.task_by_id(id)
    }

    pub fn append_run_output(&mut self, run_id: &str, stream: &str, text: &str) -> Result<()> {
        self.connection.execute(
            "INSERT INTO run_output (run_id, stream, text) VALUES (?1, ?2, ?3)",
            params![run_id, stream, text],
        )?;
        Ok(())
    }

    pub fn append_run_event(&mut self, run_id: &str, event: NewRunEvent) -> Result<()> {
        self.connection.execute(
            "INSERT INTO run_events (run_id, kind, message, command, file_path, exit_code)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                run_id,
                event.kind,
                event.message,
                event.command,
                event.file_path,
                event.exit_code,
            ],
        )?;
        Ok(())
    }

    pub fn finish_run(
        &mut self,
        run_id: &str,
        status: RunStatus,
        exit_code: Option<i32>,
        error: Option<&str>,
    ) -> Result<Option<(Run, Task)>> {
        let transaction = self.connection.transaction()?;
        let run = transaction
            .query_row(
                "SELECT task_id, status FROM runs WHERE id = ?1",
                [run_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((task_id, current_status)) = run else {
            return Ok(None);
        };
        if current_status != RunStatus::Running.as_str() {
            return Ok(None);
        }

        transaction.execute(
            "UPDATE runs SET status = ?1, completed_at = CURRENT_TIMESTAMP, exit_code = ?2, error = ?3 WHERE id = ?4",
            params![status.as_str(), exit_code, error, run_id],
        )?;
        let (kind, message) = match status {
            RunStatus::Completed => ("run.completed", "Agent run completed."),
            RunStatus::Cancelled => ("run.cancelled", "Agent run cancelled."),
            RunStatus::Failed => ("run.failed", "Agent run failed."),
            RunStatus::Queued | RunStatus::Running => ("run.updated", "Agent run updated."),
        };
        transaction.execute(
            "INSERT INTO run_events (run_id, kind, message, exit_code) VALUES (?1, ?2, ?3, ?4)",
            params![run_id, kind, message, exit_code],
        )?;
        if status == RunStatus::Completed {
            let (project_id, task_status): (String, String) = transaction.query_row(
                "SELECT project_id, status FROM tasks WHERE id = ?1",
                [&task_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            if task_status == TaskStatus::InProgress.as_str() {
                move_task_in_transaction(
                    &transaction,
                    &task_id,
                    &project_id,
                    TaskStatus::InProgress,
                    TaskStatus::Review,
                    usize::MAX,
                )?;
            }
        }
        transaction.commit()?;

        let run = self
            .get_run(run_id)?
            .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;
        let task = self
            .task_by_id(&task_id)?
            .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;
        Ok(Some((run, task)))
    }

    pub fn move_task(
        &mut self,
        id: &str,
        target_status: TaskStatus,
        target_position: usize,
    ) -> Result<Option<Task>> {
        if matches!(
            target_status,
            TaskStatus::Approved | TaskStatus::Integrating | TaskStatus::Blocked | TaskStatus::Done
        ) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let Some((project_id, source_status)) = self.task_location(id)? else {
            return Ok(None);
        };
        if matches!(
            source_status,
            TaskStatus::Approved | TaskStatus::Integrating | TaskStatus::Blocked | TaskStatus::Done
        ) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let transaction = self.connection.transaction()?;
        move_task_in_transaction(
            &transaction,
            id,
            &project_id,
            source_status,
            target_status,
            target_position,
        )?;
        transaction.commit()?;
        self.task_by_id(id)
    }

    pub fn approve_task_review(&mut self, id: &str, attempt_id: &str) -> Result<Option<Task>> {
        let transaction = self.connection.transaction()?;
        let task = transaction
            .query_row(
                "SELECT project_id, status, branch FROM tasks WHERE id = ?1",
                [id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((project_id, status, source_branch)) = task else {
            return Ok(None);
        };
        if status != TaskStatus::Review.as_str() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let source_branch = source_branch.ok_or(rusqlite::Error::InvalidQuery)?;
        let target_branch: String = transaction.query_row(
            "SELECT default_branch FROM projects WHERE id = ?1",
            [&project_id],
            |row| row.get(0),
        )?;
        let queue_position: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(queue_position) + 1, 0)
             FROM integration_attempts
             WHERE task_id IN (SELECT id FROM tasks WHERE project_id = ?1)",
            [&project_id],
            |row| row.get(0),
        )?;
        move_task_in_transaction(
            &transaction,
            id,
            &project_id,
            TaskStatus::Review,
            TaskStatus::Approved,
            usize::MAX,
        )?;
        transaction.execute(
            "INSERT INTO integration_attempts (id, task_id, source_branch, target_branch, status, queue_position)
             VALUES (?1, ?2, ?3, ?4, 'queued', ?5)",
            params![attempt_id, id, source_branch, target_branch, queue_position],
        )?;
        transaction.commit()?;
        self.task_by_id(id)
    }

    pub fn request_task_changes(&mut self, id: &str) -> Result<Option<Task>> {
        self.transition_task_from_review(id, TaskStatus::InProgress)
    }

    pub fn list_integration_attempts(&self, project_id: &str) -> Result<Vec<IntegrationAttempt>> {
        let mut statement = self.connection.prepare(
            "SELECT attempts.id, attempts.task_id, attempts.source_branch, attempts.target_branch,
                    attempts.status, attempts.queue_position, attempts.merge_commit, attempts.error,
                    attempts.created_at, attempts.started_at, attempts.completed_at
             FROM integration_attempts AS attempts
             JOIN tasks ON tasks.id = attempts.task_id
             WHERE tasks.project_id = ?1
             ORDER BY CASE attempts.status
                 WHEN 'integrating' THEN 0
                 WHEN 'queued' THEN 1
                 WHEN 'conflict' THEN 2
                 WHEN 'failed' THEN 3
                 WHEN 'merged' THEN 4
             END, attempts.queue_position ASC, attempts.created_at DESC",
        )?;
        let attempts = statement
            .query_map([project_id], integration_attempt_from_row)?
            .collect::<Result<Vec<_>>>()?;
        Ok(attempts)
    }

    pub fn get_integration_attempt(&self, id: &str) -> Result<Option<IntegrationAttempt>> {
        self.integration_attempt_by_id(id)
    }

    pub fn claim_next_integration(
        &mut self,
        project_id: &str,
    ) -> Result<Option<IntegrationAttempt>> {
        let transaction = self.connection.transaction()?;
        let locked: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM project_integration_locks WHERE project_id = ?1)",
            [project_id],
            |row| row.get(0),
        )?;
        if locked {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let attempt = transaction
            .query_row(
                "SELECT attempts.id, attempts.task_id
                 FROM integration_attempts AS attempts
                 JOIN tasks ON tasks.id = attempts.task_id
                 WHERE tasks.project_id = ?1 AND attempts.status = 'queued' AND tasks.status = 'approved'
                 ORDER BY attempts.queue_position ASC, attempts.created_at ASC
                 LIMIT 1",
                [project_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((attempt_id, task_id)) = attempt else {
            return Ok(None);
        };
        transaction.execute(
            "INSERT INTO project_integration_locks (project_id, attempt_id) VALUES (?1, ?2)",
            params![project_id, attempt_id],
        )?;
        transaction.execute(
            "UPDATE integration_attempts SET status = 'integrating', started_at = CURRENT_TIMESTAMP,
             completed_at = NULL, error = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
            [&attempt_id],
        )?;
        move_task_in_transaction(
            &transaction,
            &task_id,
            project_id,
            TaskStatus::Approved,
            TaskStatus::Integrating,
            usize::MAX,
        )?;
        transaction.commit()?;
        self.integration_attempt_by_id(&attempt_id)
    }

    pub fn block_integration(&mut self, attempt_id: &str, error: &str) -> Result<Option<Task>> {
        self.finish_integration(
            attempt_id,
            IntegrationStatus::Conflict,
            TaskStatus::Blocked,
            None,
            error,
        )
    }

    pub fn fail_integration(&mut self, attempt_id: &str, error: &str) -> Result<Option<Task>> {
        self.finish_integration(
            attempt_id,
            IntegrationStatus::Failed,
            TaskStatus::Approved,
            None,
            error,
        )
    }

    pub fn complete_integration(
        &mut self,
        attempt_id: &str,
        merge_commit: &str,
    ) -> Result<Option<Task>> {
        self.finish_integration(
            attempt_id,
            IntegrationStatus::Merged,
            TaskStatus::Done,
            Some(merge_commit),
            "",
        )
    }

    pub fn record_integration_cleanup_error(
        &mut self,
        attempt_id: &str,
        error: &str,
    ) -> Result<()> {
        self.connection.execute(
            "UPDATE integration_attempts SET error = ?1, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?2 AND status = 'merged'",
            params![error, attempt_id],
        )?;
        Ok(())
    }

    pub fn retry_integration(
        &mut self,
        attempt_id: &str,
        retry_attempt_id: &str,
    ) -> Result<Option<Task>> {
        let transaction = self.connection.transaction()?;
        let attempt = transaction
            .query_row(
                "SELECT attempts.task_id, attempts.source_branch, attempts.target_branch, attempts.status,
                        tasks.project_id, tasks.status
                 FROM integration_attempts AS attempts
                 JOIN tasks ON tasks.id = attempts.task_id
                 WHERE attempts.id = ?1",
                [attempt_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .optional()?;
        let Some((task_id, source_branch, target_branch, status, project_id, task_status)) =
            attempt
        else {
            return Ok(None);
        };
        if !matches!(status.as_str(), "conflict" | "failed") {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let queue_position: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(queue_position) + 1, 0)
             FROM integration_attempts
             WHERE task_id IN (SELECT id FROM tasks WHERE project_id = ?1)",
            [&project_id],
            |row| row.get(0),
        )?;
        if task_status == TaskStatus::Blocked.as_str() {
            move_task_in_transaction(
                &transaction,
                &task_id,
                &project_id,
                TaskStatus::Blocked,
                TaskStatus::Approved,
                usize::MAX,
            )?;
        } else if task_status != TaskStatus::Approved.as_str() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        transaction.execute(
            "INSERT INTO integration_attempts (id, task_id, source_branch, target_branch, status, queue_position)
             VALUES (?1, ?2, ?3, ?4, 'queued', ?5)",
            params![retry_attempt_id, task_id, source_branch, target_branch, queue_position],
        )?;
        transaction.commit()?;
        self.task_by_id(&task_id)
    }

    fn project_by_id(&self, id: &str) -> Result<Option<Project>> {
        let project = self
            .connection
            .query_row(
                "SELECT id, name, description, default_branch, created_at, updated_at
                 FROM projects WHERE id = ?1",
                [id],
                |row| {
                    Ok(Project {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        default_branch: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                        workspaces: Vec::new(),
                    })
                },
            )
            .optional()?;
        project
            .map(|mut record| {
                record.workspaces = self.list_workspaces(&record.id)?;
                Ok(record)
            })
            .transpose()
    }

    fn list_workspaces(&self, project_id: &str) -> Result<Vec<Workspace>> {
        let mut statement = self.connection.prepare(
            "SELECT id, project_id, worker_id, path, created_at, updated_at
             FROM workspaces WHERE project_id = ?1 ORDER BY created_at ASC",
        )?;
        let records = statement
            .query_map([project_id], |row| {
                Ok(Workspace {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    worker_id: row.get(2)?,
                    path: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(records)
    }

    fn agent_by_id(&self, id: &str) -> Result<Option<Agent>> {
        self.connection
            .query_row(
                "SELECT id, name, provider, role, model, system_prompt, skills, max_concurrent_tasks, created_at, updated_at
                 FROM agents WHERE id = ?1",
                [id],
                agent_from_row,
            )
            .optional()
    }

    fn integration_attempt_by_id(&self, id: &str) -> Result<Option<IntegrationAttempt>> {
        self.connection
            .query_row(
                "SELECT id, task_id, source_branch, target_branch, status, queue_position, merge_commit,
                        error, created_at, started_at, completed_at
                 FROM integration_attempts WHERE id = ?1",
                [id],
                integration_attempt_from_row,
            )
            .optional()
    }

    fn validation_command_by_id(&self, id: &str) -> Result<Option<ValidationCommand>> {
        self.connection
            .query_row(
                "SELECT id, project_id, stage, name, program, arguments, position, enabled, created_at, updated_at
                 FROM validation_commands WHERE id = ?1",
                [id],
                validation_command_from_row,
            )
            .optional()
    }

    fn validation_attempt_by_id(&self, id: &str) -> Result<Option<ValidationAttempt>> {
        let mut attempt = self.connection.query_row(
            "SELECT id, project_id, task_id, integration_attempt_id, stage, status, error, started_at, completed_at
             FROM validation_attempts WHERE id = ?1",
            [id],
            validation_attempt_from_row,
        ).optional()?;
        if let Some(record) = &mut attempt {
            record.events = self.list_validation_events(&record.id)?;
        }
        Ok(attempt)
    }

    fn list_validation_events(&self, attempt_id: &str) -> Result<Vec<ValidationEvent>> {
        let mut statement = self.connection.prepare(
            "SELECT id, validation_command_id, kind, message, stream, exit_code, created_at
             FROM validation_events WHERE validation_attempt_id = ?1 ORDER BY id ASC",
        )?;
        let events = statement
            .query_map([attempt_id], validation_event_from_row)?
            .collect::<Result<Vec<_>>>()?;
        Ok(events)
    }

    fn finish_integration(
        &mut self,
        attempt_id: &str,
        integration_status: IntegrationStatus,
        task_status: TaskStatus,
        merge_commit: Option<&str>,
        error: &str,
    ) -> Result<Option<Task>> {
        let transaction = self.connection.transaction()?;
        let attempt = transaction
            .query_row(
                "SELECT attempts.task_id, tasks.project_id, tasks.status
                 FROM integration_attempts AS attempts
                 JOIN tasks ON tasks.id = attempts.task_id
                 WHERE attempts.id = ?1 AND attempts.status = 'integrating'",
                [attempt_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((task_id, project_id, current_task_status)) = attempt else {
            return Ok(None);
        };
        if current_task_status != TaskStatus::Integrating.as_str() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        transaction.execute(
            "UPDATE integration_attempts
             SET status = ?1, merge_commit = ?2, error = ?3, completed_at = CURRENT_TIMESTAMP,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?4",
            params![
                integration_status.as_str(),
                merge_commit,
                (!error.is_empty()).then_some(error),
                attempt_id
            ],
        )?;
        transaction.execute(
            "DELETE FROM project_integration_locks WHERE attempt_id = ?1",
            [attempt_id],
        )?;
        move_task_in_transaction(
            &transaction,
            &task_id,
            &project_id,
            TaskStatus::Integrating,
            task_status,
            usize::MAX,
        )?;
        transaction.commit()?;
        self.task_by_id(&task_id)
    }

    fn task_by_id(&self, id: &str) -> Result<Option<Task>> {
        self.connection
            .query_row(
                "SELECT id, project_id, title, description, acceptance_criteria, implementation_notes,
                        relevant_paths, dependency_ids, assigned_agent_id, branch, worktree_path, status, position, created_at, updated_at
                 FROM tasks WHERE id = ?1",
                [id],
                task_from_row,
            )
            .optional()
    }

    pub fn get_run(&self, id: &str) -> Result<Option<Run>> {
        let mut run = self
            .connection
            .query_row(
                "SELECT id, task_id, agent_id, worker_id, status, started_at, completed_at, exit_code, error
                 FROM runs WHERE id = ?1",
                [id],
                run_from_row,
            )
            .optional()?;
        if let Some(record) = &mut run {
            record.output = self.list_run_output(&record.id)?;
            record.events = self.list_run_events(&record.id)?;
        }
        Ok(run)
    }

    fn list_run_output(&self, run_id: &str) -> Result<Vec<RunOutput>> {
        let mut statement = self.connection.prepare(
            "SELECT stream, text, created_at FROM run_output WHERE run_id = ?1 ORDER BY id ASC",
        )?;
        let output = statement
            .query_map([run_id], |row| {
                Ok(RunOutput {
                    stream: row.get(0)?,
                    text: row.get(1)?,
                    created_at: row.get(2)?,
                })
            })?
            .collect();
        output
    }

    fn list_run_events(&self, run_id: &str) -> Result<Vec<RunEvent>> {
        let mut statement = self.connection.prepare(
            "SELECT id, kind, message, command, file_path, exit_code, created_at
             FROM run_events WHERE run_id = ?1 ORDER BY id ASC",
        )?;
        let events = statement
            .query_map([run_id], |row| {
                Ok(RunEvent {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    message: row.get(2)?,
                    command: row.get(3)?,
                    file_path: row.get(4)?,
                    exit_code: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .collect();
        events
    }

    fn task_location(&self, id: &str) -> Result<Option<(String, TaskStatus)>> {
        self.connection
            .query_row(
                "SELECT project_id, status FROM tasks WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, TaskStatus::from_database(row.get(1)?)?)),
            )
            .optional()
    }

    fn transition_task_from_review(
        &mut self,
        id: &str,
        target_status: TaskStatus,
    ) -> Result<Option<Task>> {
        let Some((project_id, source_status)) = self.task_location(id)? else {
            return Ok(None);
        };
        if source_status != TaskStatus::Review {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let transaction = self.connection.transaction()?;
        move_task_in_transaction(
            &transaction,
            id,
            &project_id,
            TaskStatus::Review,
            target_status,
            usize::MAX,
        )?;
        transaction.commit()?;
        self.task_by_id(id)
    }
}

fn task_from_row(row: &rusqlite::Row<'_>) -> Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        acceptance_criteria: decode_string_list(row.get(4)?)?,
        implementation_notes: row.get(5)?,
        relevant_paths: decode_string_list(row.get(6)?)?,
        dependency_ids: decode_string_list(row.get(7)?)?,
        assigned_agent_id: row.get(8)?,
        branch: row.get(9)?,
        worktree_path: row.get(10)?,
        status: TaskStatus::from_database(row.get(11)?)?,
        position: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn integration_attempt_from_row(row: &rusqlite::Row<'_>) -> Result<IntegrationAttempt> {
    Ok(IntegrationAttempt {
        id: row.get(0)?,
        task_id: row.get(1)?,
        source_branch: row.get(2)?,
        target_branch: row.get(3)?,
        status: IntegrationStatus::from_database(row.get(4)?)?,
        queue_position: row.get(5)?,
        merge_commit: row.get(6)?,
        error: row.get(7)?,
        created_at: row.get(8)?,
        started_at: row.get(9)?,
        completed_at: row.get(10)?,
    })
}

fn validation_command_from_row(row: &rusqlite::Row<'_>) -> Result<ValidationCommand> {
    Ok(ValidationCommand {
        id: row.get(0)?,
        project_id: row.get(1)?,
        stage: ValidationStage::from_database(row.get(2)?)?,
        name: row.get(3)?,
        program: row.get(4)?,
        arguments: decode_string_list(row.get(5)?)?,
        position: row.get(6)?,
        enabled: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn validation_attempt_from_row(row: &rusqlite::Row<'_>) -> Result<ValidationAttempt> {
    Ok(ValidationAttempt {
        id: row.get(0)?,
        project_id: row.get(1)?,
        task_id: row.get(2)?,
        integration_attempt_id: row.get(3)?,
        stage: ValidationStage::from_database(row.get(4)?)?,
        status: ValidationStatus::from_database(row.get(5)?)?,
        error: row.get(6)?,
        started_at: row.get(7)?,
        completed_at: row.get(8)?,
        events: Vec::new(),
    })
}

fn validation_event_from_row(row: &rusqlite::Row<'_>) -> Result<ValidationEvent> {
    Ok(ValidationEvent {
        id: row.get(0)?,
        command_id: row.get(1)?,
        kind: row.get(2)?,
        message: row.get(3)?,
        stream: row.get(4)?,
        exit_code: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn project_health_from_row(row: &rusqlite::Row<'_>) -> Result<ProjectHealth> {
    Ok(ProjectHealth {
        project_id: row.get(0)?,
        status: ProjectHealthStatus::from_database(row.get(1)?)?,
        last_validation_attempt_id: row.get(2)?,
        last_successful_validation_at: row.get(3)?,
        last_integration_at: row.get(4)?,
        failing_gate: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn agent_from_row(row: &rusqlite::Row<'_>) -> Result<Agent> {
    Ok(Agent {
        id: row.get(0)?,
        name: row.get(1)?,
        provider: row.get(2)?,
        role: row.get(3)?,
        model: row.get(4)?,
        system_prompt: row.get(5)?,
        skills: decode_string_list(row.get(6)?)?,
        max_concurrent_tasks: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn run_from_row(row: &rusqlite::Row<'_>) -> Result<Run> {
    Ok(Run {
        id: row.get(0)?,
        task_id: row.get(1)?,
        agent_id: row.get(2)?,
        worker_id: row.get(3)?,
        status: RunStatus::from_database(row.get(4)?)?,
        started_at: row.get(5)?,
        completed_at: row.get(6)?,
        exit_code: row.get(7)?,
        error: row.get(8)?,
        output: Vec::new(),
        events: Vec::new(),
    })
}

fn encode_string_list(values: &[String]) -> Result<String> {
    serde_json::to_string(values)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(error.into()))
}

fn decode_string_list(value: String) -> Result<Vec<String>> {
    serde_json::from_str(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, error.into())
    })
}

fn task_ids_for_status(
    transaction: &rusqlite::Transaction<'_>,
    project_id: &str,
    status: TaskStatus,
) -> Result<Vec<String>> {
    let mut statement = transaction.prepare(
        "SELECT id FROM tasks WHERE project_id = ?1 AND status = ?2 ORDER BY position ASC",
    )?;
    let records = statement
        .query_map(params![project_id, status.as_str()], |row| row.get(0))?
        .collect::<Result<Vec<_>>>()?;
    Ok(records)
}

fn move_task_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    id: &str,
    project_id: &str,
    source_status: TaskStatus,
    target_status: TaskStatus,
    target_position: usize,
) -> Result<()> {
    let mut source_ids = task_ids_for_status(transaction, project_id, source_status)?;
    if source_status == target_status {
        source_ids.retain(|task_id| task_id != id);
        let insert_at = target_position.min(source_ids.len());
        source_ids.insert(insert_at, id.to_owned());
        return set_positions(transaction, &source_ids);
    }

    source_ids.retain(|task_id| task_id != id);
    let mut target_ids = task_ids_for_status(transaction, project_id, target_status)?;
    let insert_at = target_position.min(target_ids.len());
    target_ids.insert(insert_at, id.to_owned());
    transaction.execute(
        "UPDATE tasks SET status = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
        params![target_status.as_str(), id],
    )?;
    set_positions(transaction, &source_ids)?;
    set_positions(transaction, &target_ids)
}

fn normalize_positions(
    transaction: &rusqlite::Transaction<'_>,
    project_id: &str,
    status: TaskStatus,
) -> Result<()> {
    let ids = task_ids_for_status(transaction, project_id, status)?;
    set_positions(transaction, &ids)
}

fn set_positions(transaction: &rusqlite::Transaction<'_>, ids: &[String]) -> Result<()> {
    for (position, id) in ids.iter().enumerate() {
        transaction.execute(
            "UPDATE tasks SET position = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![position as i64, id],
        )?;
    }
    Ok(())
}

fn migrate(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );",
    )?;
    let current_version: Option<i64> = connection
        .query_row(
            "SELECT version FROM schema_migrations ORDER BY version DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;

    for (version, migration) in MIGRATIONS {
        if *version > current_version.unwrap_or_default() {
            connection.execute_batch(migration)?;
            connection.execute(
                "INSERT INTO schema_migrations (version) VALUES (?1)",
                [version],
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AgentUpdate, Database, IntegrationStatus, NewAgent, NewProject, NewRun, NewTask,
        NewValidationCommand, ProjectDeletion, ProjectHealthStatus, RunStatus, TaskStatus,
        TaskUpdate, ValidationStage, ValidationStatus,
    };
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temporary_database_path() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("orchestr-db-{nonce}.sqlite"))
    }

    #[test]
    fn settings_persist_when_the_database_is_reopened() {
        let database_path = temporary_database_path();
        let repository = Database::open(&database_path).expect("database opens");
        repository
            .set("ui.sidebar.collapsed", "true")
            .expect("setting saves");
        drop(repository);

        let reopened = Database::open(&database_path).expect("database reopens");
        assert_eq!(
            reopened.get("ui.sidebar.collapsed").expect("setting loads"),
            Some("true".into())
        );
        drop(reopened);

        fs::remove_file(database_path).expect("temporary database removes");
    }

    #[test]
    fn projects_and_workspaces_persist_when_the_database_is_reopened() {
        let database_path = temporary_database_path();
        let mut database = Database::open(&database_path).expect("database opens");
        database
            .create_project(NewProject {
                id: "project-1".into(),
                name: "Repository monitor".into(),
                description: Some("Observes repository state".into()),
                default_branch: "main".into(),
                workspace_id: "workspace-1".into(),
                worker_id: "local".into(),
                workspace_path: "C:/work/repository-monitor".into(),
            })
            .expect("project saves");
        drop(database);

        let reopened = Database::open(&database_path).expect("database reopens");
        let projects = reopened.list_projects().expect("projects load");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "Repository monitor");
        assert_eq!(projects[0].workspaces[0].worker_id, "local");
        assert!(reopened
            .project_name_exists("repository MONITOR")
            .expect("project name checks"));
        assert!(!reopened
            .project_name_exists("another project")
            .expect("project name checks"));
        drop(reopened);

        fs::remove_file(database_path).expect("temporary database removes");
    }

    #[test]
    fn validation_commands_attempts_and_project_health_are_persisted() {
        let database_path = temporary_database_path();
        let mut database = Database::open(&database_path).expect("database opens");
        database
            .create_project(NewProject {
                id: "project-1".into(),
                name: "Quality project".into(),
                description: None,
                default_branch: "main".into(),
                workspace_id: "workspace-1".into(),
                worker_id: "local".into(),
                workspace_path: "C:/work/quality-project".into(),
            })
            .expect("project saves");
        let command = database
            .create_validation_command(NewValidationCommand {
                id: "gate-1".into(),
                project_id: "project-1".into(),
                stage: ValidationStage::Integration,
                name: "Build".into(),
                program: "cargo".into(),
                arguments: vec!["test".into()],
            })
            .expect("gate saves");
        assert_eq!(command.arguments, ["test"]);
        let attempt = database
            .start_validation_attempt(
                "validation-1",
                "project-1",
                None,
                None,
                ValidationStage::Integration,
            )
            .expect("attempt starts");
        database
            .append_validation_event(
                &attempt.id,
                super::NewValidationEvent {
                    command_id: Some(command.id),
                    kind: "command.completed".into(),
                    message: "Build passed.".into(),
                    stream: None,
                    exit_code: Some(0),
                },
            )
            .expect("event saves");
        database
            .finish_validation_attempt(&attempt.id, ValidationStatus::Passed, None)
            .expect("attempt finishes");
        database
            .record_project_validation(
                "project-1",
                &attempt.id,
                ValidationStatus::Passed,
                None,
                true,
            )
            .expect("health saves");
        let health = database
            .get_project_health("project-1")
            .expect("health loads");
        assert_eq!(health.status, ProjectHealthStatus::Healthy);
        assert_eq!(
            health.last_validation_attempt_id.as_deref(),
            Some("validation-1")
        );
        assert!(health.last_integration_at.is_some());
        drop(database);
        fs::remove_file(database_path).expect("temporary database removes");
    }

    #[test]
    fn tasks_move_between_columns_and_keep_contiguous_ordering() {
        let database_path = temporary_database_path();
        let mut database = Database::open(&database_path).expect("database opens");
        database
            .create_project(NewProject {
                id: "project-1".into(),
                name: "Board project".into(),
                description: None,
                default_branch: "main".into(),
                workspace_id: "workspace-1".into(),
                worker_id: "local".into(),
                workspace_path: "C:/work/board-project".into(),
            })
            .expect("project saves");

        for (id, title) in [
            ("task-1", "First"),
            ("task-2", "Second"),
            ("task-3", "Third"),
        ] {
            database
                .create_task(NewTask {
                    id: id.into(),
                    project_id: "project-1".into(),
                    title: title.into(),
                    description: None,
                    acceptance_criteria: Vec::new(),
                    implementation_notes: None,
                    relevant_paths: Vec::new(),
                    dependency_ids: Vec::new(),
                    assigned_agent_id: None,
                })
                .expect("task saves");
        }

        database
            .move_task("task-1", TaskStatus::Todo, 0)
            .expect("task moves")
            .expect("task exists");
        database
            .move_task("task-3", TaskStatus::Backlog, 0)
            .expect("task reorders")
            .expect("task exists");

        let tasks = database.list_tasks("project-1").expect("tasks load");
        let backlog = tasks
            .iter()
            .filter(|task| task.status == TaskStatus::Backlog)
            .collect::<Vec<_>>();
        assert_eq!(
            backlog
                .iter()
                .map(|task| task.id.as_str())
                .collect::<Vec<_>>(),
            ["task-3", "task-2"]
        );
        assert_eq!(
            backlog.iter().map(|task| task.position).collect::<Vec<_>>(),
            [0, 1]
        );
        assert_eq!(
            tasks
                .iter()
                .find(|task| task.id == "task-1")
                .expect("task exists")
                .status,
            TaskStatus::Todo
        );

        let updated = database
            .update_task(
                "task-1",
                TaskUpdate {
                    title: "First, revised".into(),
                    description: Some("Updated description".into()),
                    acceptance_criteria: vec!["Task can be revised".into()],
                    implementation_notes: Some("Keep the ordering intact.".into()),
                    relevant_paths: vec!["src/tasks.rs".into()],
                    dependency_ids: vec!["task-2".into()],
                    assigned_agent_id: None,
                },
            )
            .expect("task updates")
            .expect("task exists");
        assert_eq!(updated.title, "First, revised");
        assert_eq!(updated.acceptance_criteria, ["Task can be revised"]);
        assert_eq!(
            updated.implementation_notes.as_deref(),
            Some("Keep the ordering intact.")
        );
        assert_eq!(updated.relevant_paths, ["src/tasks.rs"]);
        assert_eq!(updated.dependency_ids, ["task-2"]);
        assert!(database.delete_task("task-3").expect("task deletes"));
        let tasks = database.list_tasks("project-1").expect("tasks reload");
        assert_eq!(
            tasks
                .iter()
                .find(|task| task.id == "task-2")
                .expect("remaining backlog task")
                .position,
            0
        );

        drop(database);
        fs::remove_file(database_path).expect("temporary database removes");
    }

    #[test]
    fn agents_persist_and_are_removed_from_assigned_tasks() {
        let database_path = temporary_database_path();
        let mut database = Database::open(&database_path).expect("database opens");
        database
            .create_project(NewProject {
                id: "project-1".into(),
                name: "Agent project".into(),
                description: None,
                default_branch: "main".into(),
                workspace_id: "workspace-1".into(),
                worker_id: "local".into(),
                workspace_path: "C:/work/agent-project".into(),
            })
            .expect("project saves");
        database
            .create_agent(NewAgent {
                id: "agent-1".into(),
                name: "Codex Terra".into(),
                provider: "codex".into(),
                role: "Frontend engineer".into(),
                model: Some("gpt-5.6-terra".into()),
                system_prompt: None,
                skills: vec!["react".into()],
                max_concurrent_tasks: 2,
            })
            .expect("agent saves");
        database
            .create_task(NewTask {
                id: "task-1".into(),
                project_id: "project-1".into(),
                title: "Build dashboard".into(),
                description: None,
                acceptance_criteria: Vec::new(),
                implementation_notes: None,
                relevant_paths: Vec::new(),
                dependency_ids: Vec::new(),
                assigned_agent_id: Some("agent-1".into()),
            })
            .expect("task saves");

        let updated = database
            .update_agent(
                "agent-1",
                AgentUpdate {
                    name: "Codex Terra".into(),
                    provider: "codex".into(),
                    role: "UI engineer".into(),
                    model: None,
                    system_prompt: Some("Use accessible components.".into()),
                    skills: vec!["react".into(), "typescript".into()],
                    max_concurrent_tasks: 1,
                },
            )
            .expect("agent updates")
            .expect("agent exists");
        assert_eq!(updated.role, "UI engineer");
        assert_eq!(updated.skills, ["react", "typescript"]);
        assert!(database.agent_exists("agent-1").expect("agent check"));
        assert!(database.delete_agent("agent-1").expect("agent deletes"));
        assert!(database.list_tasks("project-1").expect("tasks load")[0]
            .assigned_agent_id
            .is_none());

        drop(database);
        fs::remove_file(database_path).expect("temporary database removes");
    }

    #[test]
    fn completed_run_persists_output_and_moves_its_task_to_review() {
        let database_path = temporary_database_path();
        let mut database = Database::open(&database_path).expect("database opens");
        database
            .create_project(NewProject {
                id: "project-1".into(),
                name: "Run project".into(),
                description: None,
                default_branch: "main".into(),
                workspace_id: "workspace-1".into(),
                worker_id: "local".into(),
                workspace_path: "C:/work/run-project".into(),
            })
            .expect("project saves");
        database
            .create_agent(NewAgent {
                id: "agent-1".into(),
                name: "Codex Terra".into(),
                provider: "codex".into(),
                role: "Engineer".into(),
                model: None,
                system_prompt: None,
                skills: Vec::new(),
                max_concurrent_tasks: 1,
            })
            .expect("agent saves");
        database
            .create_task(NewTask {
                id: "task-1".into(),
                project_id: "project-1".into(),
                title: "Implement run".into(),
                description: None,
                acceptance_criteria: Vec::new(),
                implementation_notes: None,
                relevant_paths: Vec::new(),
                dependency_ids: Vec::new(),
                assigned_agent_id: Some("agent-1".into()),
            })
            .expect("task saves");
        database
            .move_task("task-1", TaskStatus::Todo, 0)
            .expect("task moves");
        database
            .assign_task_worktree(
                "task-1",
                "task/run",
                "C:/work/.orchestr-worktrees/project-1/task-1",
            )
            .expect("task worktree assigns");

        let (run, started_task) = database
            .start_run(NewRun {
                id: "run-1".into(),
                task_id: "task-1".into(),
                agent_id: "agent-1".into(),
                worker_id: "local".into(),
            })
            .expect("run starts");
        assert_eq!(run.status, RunStatus::Running);
        assert_eq!(started_task.status, TaskStatus::InProgress);
        database
            .append_run_output("run-1", "stdout", "Task complete")
            .expect("run output saves");
        let (_, completed_task) = database
            .finish_run("run-1", RunStatus::Completed, Some(0), None)
            .expect("run completes")
            .expect("run exists");
        assert_eq!(completed_task.status, TaskStatus::Review);
        drop(database);

        let mut reopened = Database::open(&database_path).expect("database reopens");
        let runs = reopened.list_runs_for_task("task-1").expect("runs load");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, RunStatus::Completed);
        assert_eq!(runs[0].output[0].text, "Task complete");
        assert_eq!(runs[0].events[0].kind, "run.started");
        assert!(runs[0]
            .events
            .iter()
            .any(|event| event.kind == "run.completed"));
        assert_eq!(
            reopened
                .request_task_changes("task-1")
                .expect("review changes request")
                .expect("task exists")
                .status,
            TaskStatus::InProgress
        );
        reopened
            .move_task("task-1", TaskStatus::Review, 0)
            .expect("task returns to review");
        assert_eq!(
            reopened
                .approve_task_review("task-1", "integration-1")
                .expect("review approves")
                .expect("task exists")
                .status,
            TaskStatus::Approved
        );
        assert_eq!(
            reopened
                .list_integration_attempts("project-1")
                .expect("integration attempts load")[0]
                .status,
            IntegrationStatus::Queued
        );
        let claimed = reopened
            .claim_next_integration("project-1")
            .expect("integration queue claims")
            .expect("queued integration exists");
        assert_eq!(claimed.status, IntegrationStatus::Integrating);
        assert!(
            reopened.claim_next_integration("project-1").is_err(),
            "project integration lock rejects concurrent claim"
        );
        assert_eq!(
            reopened
                .block_integration(&claimed.id, "shared.txt")
                .expect("integration conflict records")
                .expect("task exists")
                .status,
            TaskStatus::Blocked
        );
        assert_eq!(
            reopened
                .retry_integration(&claimed.id, "integration-2")
                .expect("integration retry queues")
                .expect("task exists")
                .status,
            TaskStatus::Approved
        );
        let attempts = reopened
            .list_integration_attempts("project-1")
            .expect("integration history loads");
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0].status, IntegrationStatus::Queued);
        assert_eq!(attempts[1].status, IntegrationStatus::Conflict);
        drop(reopened);
        fs::remove_file(database_path).expect("temporary database removes");
    }

    #[test]
    fn task_worktree_ownership_persists_and_can_be_released() {
        let database_path = temporary_database_path();
        let mut database = Database::open(&database_path).expect("database opens");
        database
            .create_project(NewProject {
                id: "project-1".into(),
                name: "Worktree project".into(),
                description: None,
                default_branch: "main".into(),
                workspace_id: "workspace-1".into(),
                worker_id: "local".into(),
                workspace_path: "C:/work/worktree-project".into(),
            })
            .expect("project saves");
        database
            .create_task(NewTask {
                id: "task-1".into(),
                project_id: "project-1".into(),
                title: "Isolate implementation".into(),
                description: None,
                acceptance_criteria: Vec::new(),
                implementation_notes: None,
                relevant_paths: Vec::new(),
                dependency_ids: Vec::new(),
                assigned_agent_id: None,
            })
            .expect("task saves");
        database
            .move_task("task-1", TaskStatus::Todo, 0)
            .expect("task moves");

        let assigned = database
            .assign_task_worktree(
                "task-1",
                "task/task-1-isolate-implementation",
                "C:/work/.orchestr-worktrees/project-1/task-1",
            )
            .expect("worktree assigns")
            .expect("task exists");
        assert_eq!(
            assigned.branch.as_deref(),
            Some("task/task-1-isolate-implementation")
        );
        assert_eq!(
            assigned.worktree_path.as_deref(),
            Some("C:/work/.orchestr-worktrees/project-1/task-1")
        );
        assert!(database
            .assign_task_worktree("task-1", "task/another", "C:/work/another")
            .expect("repeat assignment checks")
            .is_none());

        let released = database
            .release_task_worktree("task-1")
            .expect("worktree releases")
            .expect("task exists");
        assert_eq!(
            released.branch.as_deref(),
            Some("task/task-1-isolate-implementation")
        );
        assert!(released.worktree_path.is_none());

        drop(database);
        fs::remove_file(database_path).expect("temporary database removes");
    }

    #[test]
    fn deleting_a_project_removes_metadata_but_rejects_attached_worktrees() {
        let database_path = temporary_database_path();
        let mut database = Database::open(&database_path).expect("database opens");
        database
            .create_project(NewProject {
                id: "project-1".into(),
                name: "Disposable project".into(),
                description: None,
                default_branch: "main".into(),
                workspace_id: "workspace-1".into(),
                worker_id: "local".into(),
                workspace_path: "C:/work/disposable-project".into(),
            })
            .expect("project saves");
        database
            .create_task(NewTask {
                id: "task-1".into(),
                project_id: "project-1".into(),
                title: "Attached task".into(),
                description: None,
                acceptance_criteria: Vec::new(),
                implementation_notes: None,
                relevant_paths: Vec::new(),
                dependency_ids: Vec::new(),
                assigned_agent_id: None,
            })
            .expect("task saves");
        database
            .move_task("task-1", TaskStatus::Todo, 0)
            .expect("task moves");
        database
            .assign_task_worktree("task-1", "task/attached", "C:/work/attached")
            .expect("worktree assigns");
        assert_eq!(
            database
                .delete_project("project-1")
                .expect("project deletion checks"),
            ProjectDeletion::HasAttachedWorktrees
        );

        database
            .release_task_worktree("task-1")
            .expect("worktree releases");
        assert_eq!(
            database
                .delete_project("project-1")
                .expect("project deletes"),
            ProjectDeletion::Deleted
        );
        assert!(database
            .get_project("project-1")
            .expect("project loads")
            .is_none());
        assert!(database
            .list_tasks("project-1")
            .expect("tasks load")
            .is_empty());

        drop(database);
        fs::remove_file(database_path).expect("temporary database removes");
    }
}
