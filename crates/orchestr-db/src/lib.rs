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
    (
        12,
        include_str!("../migrations/0012_readiness_block_flag.sql"),
    ),
    (13, include_str!("../migrations/0013_project_outcomes.sql")),
    (14, include_str!("../migrations/0014_agent_reviews.sql")),
    (15, include_str!("../migrations/0015_flow_control.sql")),
    (16, include_str!("../migrations/0016_failure_recovery.sql")),
    (
        17,
        include_str!("../migrations/0017_project_blockers_needs_input.sql"),
    ),
    (
        18,
        include_str!("../migrations/0018_architecture_decisions.sql"),
    ),
    (19, include_str!("../migrations/0019_remote_workers.sql")),
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

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Backlog,
    Ready,
    InProgress,
    NeedsInput,
    Review,
    Approved,
    Integrating,
    Blocked,
    Done,
}

impl TaskStatus {
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|status| status.as_str() == value)
    }

    pub fn as_str(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    const ALL: [Self; 9] = [
        Self::Backlog,
        Self::Ready,
        Self::InProgress,
        Self::NeedsInput,
        Self::Review,
        Self::Approved,
        Self::Integrating,
        Self::Blocked,
        Self::Done,
    ];
    const NAMES: [&'static str; 9] = [
        "backlog",
        "ready",
        "in_progress",
        "needs_input",
        "review",
        "approved",
        "integrating",
        "blocked",
        "done",
    ];

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevertStatus {
    Running,
    Reverted,
    ValidationFailed,
    Failed,
}

impl RevertStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Reverted => "reverted",
            Self::ValidationFailed => "validation_failed",
            Self::Failed => "failed",
        }
    }

    fn from_database(value: String) -> Result<Self> {
        match value.as_str() {
            "running" => Ok(Self::Running),
            "reverted" => Ok(Self::Reverted),
            "validation_failed" => Ok(Self::ValidationFailed),
            "failed" => Ok(Self::Failed),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevertAttempt {
    pub id: String,
    pub project_id: String,
    pub original_task_id: String,
    pub integration_attempt_id: String,
    pub original_commit: String,
    pub status: RevertStatus,
    pub revert_commit: Option<String>,
    pub repair_task_id: Option<String>,
    pub error: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskInputRequest {
    pub id: String,
    pub task_id: String,
    pub requesting_run_id: Option<String>,
    pub requesting_agent_id: Option<String>,
    pub question: String,
    pub status: String,
    pub answer: Option<String>,
    pub requested_at: String,
    pub answered_at: Option<String>,
}

pub struct NewTaskInputRequest {
    pub id: String,
    pub task_id: String,
    pub requesting_run_id: Option<String>,
    pub question: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectBlocker {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub affects_all_tasks: bool,
    pub affected_task_ids: Vec<String>,
    pub status: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

pub struct NewProjectBlocker {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub affects_all_tasks: bool,
    pub affected_task_ids: Vec<String>,
}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchitectureDecisionStatus {
    Proposed,
    Accepted,
    Superseded,
    Rejected,
}

impl ArchitectureDecisionStatus {
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|status| status.as_str() == value)
    }

    pub fn as_str(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    const ALL: [Self; 4] = [
        Self::Proposed,
        Self::Accepted,
        Self::Superseded,
        Self::Rejected,
    ];
    const NAMES: [&'static str; 4] = ["proposed", "accepted", "superseded", "rejected"];

    fn from_database(value: String) -> Result<Self> {
        Self::parse(&value).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                7,
                rusqlite::types::Type::Text,
                format!("Unknown architecture decision status: {value}").into(),
            )
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchitectureDecision {
    pub id: String,
    pub project_id: String,
    pub decision_number: i64,
    pub title: String,
    pub context: String,
    pub decision: String,
    pub consequences: Option<String>,
    pub status: ArchitectureDecisionStatus,
    pub supersedes_decision_id: Option<String>,
    pub relevant_paths: Vec<String>,
    pub relevant_task_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub decided_at: Option<String>,
}

pub struct NewArchitectureDecision {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub context: String,
    pub decision: String,
    pub consequences: Option<String>,
    pub supersedes_decision_id: Option<String>,
    pub relevant_paths: Vec<String>,
    pub relevant_task_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct WorkerToolCapability {
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteWorkerWorkspace {
    pub project_id: String,
    pub workspace_path: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteWorker {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub token_environment_variable: String,
    pub ca_certificate_pem: Option<String>,
    pub os: String,
    pub architecture: String,
    pub status: String,
    pub protocol_version: i64,
    pub tools: Vec<WorkerToolCapability>,
    pub workspaces: Vec<RemoteWorkerWorkspace>,
    pub last_seen_at: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct NewRemoteWorker {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub token_environment_variable: String,
    pub ca_certificate_pem: Option<String>,
    pub os: String,
    pub architecture: String,
    pub protocol_version: i64,
    pub tools: Vec<WorkerToolCapability>,
    pub project_id: String,
    pub workspace_path: String,
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
    pub readiness_blocked: bool,
    pub milestone_id: Option<String>,
    pub epic_id: Option<String>,
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
    pub milestone_id: Option<String>,
    pub epic_id: Option<String>,
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
    pub milestone_id: Option<String>,
    pub epic_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Milestone {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub target_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct NewMilestone {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub target_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Epic {
    pub id: String,
    pub project_id: String,
    pub milestone_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct NewEpic {
    pub id: String,
    pub project_id: String,
    pub milestone_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskProgressCounts {
    pub total: i64,
    pub backlog: i64,
    pub ready: i64,
    pub in_progress: i64,
    pub needs_input: i64,
    pub review: i64,
    pub blocked: i64,
    pub done: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneProgress {
    pub milestone: Milestone,
    pub counts: TaskProgressCounts,
    pub epics: Vec<Epic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectProgress {
    pub counts: TaskProgressCounts,
    pub milestones: Vec<MilestoneProgress>,
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
pub enum AgentReviewStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl AgentReviewStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn from_database(value: String) -> Result<Self> {
        match value.as_str() {
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Text,
                format!("Unknown agent review status: {value}").into(),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentReviewDecision {
    Approve,
    RequestChanges,
}

impl AgentReviewDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::RequestChanges => "request_changes",
        }
    }

    fn from_database(value: String) -> Result<Self> {
        match value.as_str() {
            "approve" => Ok(Self::Approve),
            "request_changes" => Ok(Self::RequestChanges),
            _ => Err(rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                format!("Unknown agent review decision: {value}").into(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentReview {
    pub id: String,
    pub task_id: String,
    pub agent_id: String,
    pub status: AgentReviewStatus,
    pub decision: Option<AgentReviewDecision>,
    pub notes: Option<String>,
    pub raw_output: String,
    pub error: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

pub struct NewAgentReview {
    pub id: String,
    pub task_id: String,
    pub agent_id: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowLimits {
    pub project_id: String,
    pub worker_id: String,
    pub worker_max_concurrent_runs: i64,
    pub in_progress_limit: i64,
    pub review_limit: i64,
    pub approved_limit: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowState {
    pub limits: FlowLimits,
    pub active_worker_runs: i64,
    pub in_progress: i64,
    pub review: i64,
    pub approved: i64,
    pub integrating: i64,
    pub queued: i64,
    pub blocked_reason: Option<String>,
}

pub struct FlowLimitUpdate {
    pub worker_max_concurrent_runs: i64,
    pub in_progress_limit: i64,
    pub review_limit: i64,
    pub approved_limit: i64,
}

impl Database {
    pub fn open(database_path: &Path) -> Result<Self> {
        let connection = Connection::open(database_path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&connection)?;
        let mut database = Self { connection };
        database.recalculate_all_task_readiness()?;
        Ok(database)
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

    pub fn mark_project_health_broken(&mut self, project_id: &str, reason: &str) -> Result<()> {
        self.connection.execute(
            "INSERT INTO project_health (project_id, status, failing_gate) VALUES (?1, 'broken', ?2)
             ON CONFLICT(project_id) DO UPDATE SET status = 'broken', failing_gate = excluded.failing_gate, updated_at = CURRENT_TIMESTAMP",
            params![project_id, reason],
        )?;
        Ok(())
    }

    pub fn list_task_input_requests(&self, task_id: &str) -> Result<Vec<TaskInputRequest>> {
        let mut statement = self.connection.prepare(
            "SELECT id, task_id, requesting_run_id, requesting_agent_id, question, status,
                    answer, requested_at, answered_at
             FROM task_input_requests WHERE task_id = ?1
             ORDER BY requested_at DESC, id DESC",
        )?;
        let requests = statement
            .query_map([task_id], task_input_request_from_row)?
            .collect();
        requests
    }

    pub fn request_task_input(&mut self, request: NewTaskInputRequest) -> Result<TaskInputRequest> {
        if request.question.trim().is_empty() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let transaction = self.connection.transaction()?;
        let (project_id, task_status, assigned_agent_id): (String, String, Option<String>) =
            transaction.query_row(
                "SELECT project_id, status, assigned_agent_id FROM tasks WHERE id = ?1",
                [&request.task_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
        if task_status != TaskStatus::InProgress.as_str() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let requesting_agent_id = requesting_agent_for_run(
            &transaction,
            &request.task_id,
            request.requesting_run_id.as_deref(),
            assigned_agent_id,
        )?;
        move_task_in_transaction(
            &transaction,
            &request.task_id,
            &project_id,
            TaskStatus::InProgress,
            TaskStatus::NeedsInput,
            usize::MAX,
        )?;
        transaction.execute(
            "INSERT INTO task_input_requests
                (id, task_id, requesting_run_id, requesting_agent_id, question)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                request.id,
                request.task_id,
                request.requesting_run_id,
                requesting_agent_id,
                request.question.trim()
            ],
        )?;
        append_input_request_event(
            &transaction,
            request.requesting_run_id.as_deref(),
            "input.requested",
            request.question.trim(),
        )?;
        transaction.commit()?;
        self.task_input_request_by_id(&request.id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn answer_task_input(
        &mut self,
        request_id: &str,
        answer: &str,
    ) -> Result<Option<(TaskInputRequest, Task)>> {
        if answer.trim().is_empty() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let transaction = self.connection.transaction()?;
        let request = open_input_request_context(&transaction, request_id)?;
        let Some((task_id, requesting_run_id, project_id, task_status)) = request else {
            return Ok(None);
        };
        if task_status != TaskStatus::NeedsInput.as_str()
            || task_has_running_run(&transaction, &task_id)?
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        transaction.execute(
            "UPDATE task_input_requests
             SET status = 'answered', answer = ?1, answered_at = CURRENT_TIMESTAMP
             WHERE id = ?2 AND status = 'open'",
            params![answer.trim(), request_id],
        )?;
        move_task_in_transaction(
            &transaction,
            &task_id,
            &project_id,
            TaskStatus::NeedsInput,
            TaskStatus::InProgress,
            usize::MAX,
        )?;
        append_input_request_event(
            &transaction,
            requesting_run_id.as_deref(),
            "input.answered",
            answer.trim(),
        )?;
        transaction.commit()?;
        Ok(Some((
            self.task_input_request_by_id(request_id)?
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?,
            self.task_by_id(&task_id)?
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?,
        )))
    }

    pub fn list_project_blockers(&self, project_id: &str) -> Result<Vec<ProjectBlocker>> {
        let mut statement = self.connection.prepare(
            "SELECT id, project_id, title, description, affects_all_tasks, status,
                    created_at, resolved_at
             FROM project_blockers WHERE project_id = ?1
             ORDER BY CASE status WHEN 'active' THEN 0 ELSE 1 END, created_at DESC, id DESC",
        )?;
        let blockers = statement
            .query_map([project_id], project_blocker_from_row)?
            .collect::<Result<Vec<_>>>()?;
        blockers
            .into_iter()
            .map(|mut blocker| {
                blocker.affected_task_ids = self.blocker_task_ids(&blocker.id)?;
                Ok(blocker)
            })
            .collect()
    }

    pub fn create_project_blocker(&mut self, blocker: NewProjectBlocker) -> Result<ProjectBlocker> {
        validate_new_project_blocker(&self.connection, &blocker)?;
        let project_id = blocker.project_id.clone();
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO project_blockers
                (id, project_id, title, description, affects_all_tasks)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                blocker.id,
                blocker.project_id,
                blocker.title.trim(),
                blocker.description,
                blocker.affects_all_tasks
            ],
        )?;
        if !blocker.affects_all_tasks {
            for task_id in &blocker.affected_task_ids {
                transaction.execute(
                    "INSERT INTO project_blocker_tasks (blocker_id, task_id) VALUES (?1, ?2)",
                    params![blocker.id, task_id],
                )?;
            }
        }
        transaction.commit()?;
        self.recalculate_project_task_readiness(&project_id)?;
        self.project_blocker_by_id(&blocker.id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn resolve_project_blocker(&mut self, blocker_id: &str) -> Result<Option<ProjectBlocker>> {
        let project_id: Option<String> = self
            .connection
            .query_row(
                "SELECT project_id FROM project_blockers WHERE id = ?1 AND status = 'active'",
                [blocker_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(project_id) = project_id else {
            return Ok(None);
        };
        self.connection.execute(
            "UPDATE project_blockers
             SET status = 'resolved', resolved_at = CURRENT_TIMESTAMP
             WHERE id = ?1 AND status = 'active'",
            [blocker_id],
        )?;
        self.recalculate_project_task_readiness(&project_id)?;
        self.project_blocker_by_id(blocker_id)
    }

    pub fn list_architecture_decisions(
        &self,
        project_id: &str,
    ) -> Result<Vec<ArchitectureDecision>> {
        let mut statement = self.connection.prepare(
            "SELECT id, project_id, decision_number, title, context, decision, consequences,
                    status, supersedes_decision_id, relevant_paths, created_at, updated_at, decided_at
             FROM architecture_decisions WHERE project_id = ?1
             ORDER BY decision_number DESC",
        )?;
        let decisions = statement
            .query_map([project_id], architecture_decision_from_row)?
            .collect::<Result<Vec<_>>>()?;
        decisions
            .into_iter()
            .map(|decision| self.with_architecture_decision_tasks(decision))
            .collect()
    }

    pub fn list_relevant_architecture_decisions(
        &self,
        task_id: &str,
    ) -> Result<Vec<ArchitectureDecision>> {
        let task = self
            .task_by_id(task_id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        Ok(self
            .list_architecture_decisions(&task.project_id)?
            .into_iter()
            .filter(|decision| {
                decision.status == ArchitectureDecisionStatus::Accepted
                    && architecture_decision_applies(decision, &task)
            })
            .collect())
    }

    pub fn create_architecture_decision(
        &mut self,
        decision: NewArchitectureDecision,
    ) -> Result<ArchitectureDecision> {
        validate_new_architecture_decision(&self.connection, &decision)?;
        let relevant_paths = encode_string_list(&decision.relevant_paths)?;
        let transaction = self.connection.transaction()?;
        let decision_number: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(decision_number) + 1, 1)
             FROM architecture_decisions WHERE project_id = ?1",
            [&decision.project_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO architecture_decisions
                (id, project_id, decision_number, title, context, decision, consequences,
                 supersedes_decision_id, relevant_paths)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                decision.id,
                decision.project_id,
                decision_number,
                decision.title.trim(),
                decision.context.trim(),
                decision.decision.trim(),
                trimmed_optional(decision.consequences.as_deref()),
                decision.supersedes_decision_id,
                relevant_paths,
            ],
        )?;
        for task_id in &decision.relevant_task_ids {
            transaction.execute(
                "INSERT INTO architecture_decision_tasks (decision_id, task_id) VALUES (?1, ?2)",
                params![decision.id, task_id],
            )?;
        }
        transaction.commit()?;
        self.architecture_decision_by_id(&decision.id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn decide_architecture_decision(
        &mut self,
        decision_id: &str,
        status: ArchitectureDecisionStatus,
    ) -> Result<Option<ArchitectureDecision>> {
        if !matches!(
            status,
            ArchitectureDecisionStatus::Accepted | ArchitectureDecisionStatus::Rejected
        ) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let transaction = self.connection.transaction()?;
        let context = proposed_architecture_decision_context(&transaction, decision_id)?;
        let Some((project_id, supersedes_id)) = context else {
            return Ok(None);
        };
        if status == ArchitectureDecisionStatus::Accepted {
            supersede_previous_architecture_decision(
                &transaction,
                &project_id,
                supersedes_id.as_deref(),
            )?;
        }
        transaction.execute(
            "UPDATE architecture_decisions
             SET status = ?1, decided_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?2 AND status = 'proposed'",
            params![status.as_str(), decision_id],
        )?;
        transaction.commit()?;
        self.architecture_decision_by_id(decision_id)
    }

    pub fn list_remote_workers(&self) -> Result<Vec<RemoteWorker>> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, endpoint, token_environment_variable, ca_certificate_pem,
                    os, architecture, status, protocol_version, tools, last_seen_at,
                    created_at, updated_at
             FROM remote_workers ORDER BY name ASC, id ASC",
        )?;
        let workers = statement
            .query_map([], remote_worker_from_row)?
            .collect::<Result<Vec<_>>>()?;
        workers
            .into_iter()
            .map(|worker| self.with_remote_worker_workspaces(worker))
            .collect()
    }

    pub fn get_remote_worker(&self, worker_id: &str) -> Result<Option<RemoteWorker>> {
        let worker = self
            .connection
            .query_row(
                "SELECT id, name, endpoint, token_environment_variable, ca_certificate_pem,
                        os, architecture, status, protocol_version, tools, last_seen_at,
                        created_at, updated_at
                 FROM remote_workers WHERE id = ?1",
                [worker_id],
                remote_worker_from_row,
            )
            .optional()?;
        worker
            .map(|record| self.with_remote_worker_workspaces(record))
            .transpose()
    }

    pub fn remote_worker_for_project(&self, project_id: &str) -> Result<Option<RemoteWorker>> {
        let worker_id = self
            .connection
            .query_row(
                "SELECT worker_id FROM remote_worker_projects
                 WHERE project_id = ?1 AND enabled = 1",
                [project_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        worker_id
            .map(|worker_id| self.get_remote_worker(&worker_id))
            .transpose()
            .map(Option::flatten)
    }

    pub fn register_remote_worker(&mut self, worker: NewRemoteWorker) -> Result<RemoteWorker> {
        validate_new_remote_worker(&self.connection, &worker)?;
        let tools = serde_json::to_string(&worker.tools).map_err(json_conversion_error)?;
        let worker_id = worker.id.clone();
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO remote_workers
                (id, name, endpoint, token_environment_variable, ca_certificate_pem,
                 os, architecture, status, protocol_version, tools)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'online', ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                 name = excluded.name, endpoint = excluded.endpoint,
                 token_environment_variable = excluded.token_environment_variable,
                 ca_certificate_pem = excluded.ca_certificate_pem,
                 os = excluded.os, architecture = excluded.architecture,
                 status = 'online', protocol_version = excluded.protocol_version,
                 tools = excluded.tools, last_seen_at = CURRENT_TIMESTAMP,
                 updated_at = CURRENT_TIMESTAMP",
            params![
                worker.id,
                worker.name,
                worker.endpoint,
                worker.token_environment_variable,
                worker.ca_certificate_pem,
                worker.os,
                worker.architecture,
                worker.protocol_version,
                tools,
            ],
        )?;
        transaction.execute(
            "UPDATE remote_worker_projects SET enabled = 0, updated_at = CURRENT_TIMESTAMP
             WHERE project_id = ?1 AND worker_id <> ?2 AND enabled = 1",
            params![worker.project_id, worker_id],
        )?;
        transaction.execute(
            "INSERT INTO remote_worker_projects (worker_id, project_id, workspace_path, enabled)
             VALUES (?1, ?2, ?3, 1)
             ON CONFLICT(worker_id, project_id) DO UPDATE SET
                 workspace_path = excluded.workspace_path, enabled = 1,
                 updated_at = CURRENT_TIMESTAMP",
            params![worker_id, worker.project_id, worker.workspace_path],
        )?;
        transaction.commit()?;
        self.get_remote_worker(&worker_id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn mark_remote_worker_offline(&mut self, worker_id: &str) -> Result<bool> {
        Ok(self.connection.execute(
            "UPDATE remote_workers SET status = 'offline', updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            [worker_id],
        )? > 0)
    }

    pub fn delete_remote_worker(&mut self, worker_id: &str) -> Result<bool> {
        Ok(self
            .connection
            .execute("DELETE FROM remote_workers WHERE id = ?1", [worker_id])?
            > 0)
    }

    pub fn list_tasks(&self, project_id: &str) -> Result<Vec<Task>> {
        let mut statement = self.connection.prepare(
            "SELECT id, project_id, title, description, acceptance_criteria, implementation_notes,
                    relevant_paths, dependency_ids, assigned_agent_id, branch, worktree_path, priority, blocked_reason, readiness_blocked, milestone_id, epic_id, status, position, created_at, updated_at
             FROM tasks WHERE project_id = ?1
             ORDER BY CASE status
                 WHEN 'backlog' THEN 0
                 WHEN 'ready' THEN 1
                 WHEN 'in_progress' THEN 2
                 WHEN 'needs_input' THEN 3
                 WHEN 'review' THEN 4
                 WHEN 'approved' THEN 5
                 WHEN 'integrating' THEN 6
                 WHEN 'blocked' THEN 7
                 WHEN 'done' THEN 8
             END, position ASC",
        )?;
        let records = statement
            .query_map([project_id], task_from_row)?
            .collect::<Result<Vec<_>>>()?;
        Ok(records)
    }

    pub fn list_milestones(&self, project_id: &str) -> Result<Vec<Milestone>> {
        let mut statement = self.connection.prepare(
            "SELECT id, project_id, title, description, status, target_date, created_at, updated_at
             FROM milestones WHERE project_id = ?1 ORDER BY created_at ASC",
        )?;
        let milestones = statement
            .query_map([project_id], milestone_from_row)?
            .collect::<Result<Vec<_>>>()?;
        Ok(milestones)
    }

    pub fn create_milestone(&mut self, milestone: NewMilestone) -> Result<Milestone> {
        validate_outcome_status(&milestone.status)?;
        self.connection.execute(
            "INSERT INTO milestones (id, project_id, title, description, status, target_date)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                milestone.id,
                milestone.project_id,
                milestone.title,
                milestone.description,
                milestone.status,
                milestone.target_date
            ],
        )?;
        self.milestone_by_id(&milestone.id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn update_milestone_status(&mut self, id: &str, status: &str) -> Result<Option<Milestone>> {
        validate_outcome_status(status)?;
        if self.connection.execute(
            "UPDATE milestones SET status = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![status, id],
        )? == 0
        {
            return Ok(None);
        }
        self.milestone_by_id(id)
    }

    pub fn list_epics(&self, project_id: &str) -> Result<Vec<Epic>> {
        let mut statement = self.connection.prepare(
            "SELECT id, project_id, milestone_id, title, description, status, created_at, updated_at
             FROM epics WHERE project_id = ?1 ORDER BY created_at ASC",
        )?;
        let epics = statement
            .query_map([project_id], epic_from_row)?
            .collect::<Result<Vec<_>>>()?;
        Ok(epics)
    }

    pub fn create_epic(&mut self, epic: NewEpic) -> Result<Epic> {
        validate_outcome_status(&epic.status)?;
        self.validate_milestone_for_project(&epic.project_id, epic.milestone_id.as_deref())?;
        self.connection.execute(
            "INSERT INTO epics (id, project_id, milestone_id, title, description, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                epic.id,
                epic.project_id,
                epic.milestone_id,
                epic.title,
                epic.description,
                epic.status
            ],
        )?;
        self.epic_by_id(&epic.id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn update_epic_status(&mut self, id: &str, status: &str) -> Result<Option<Epic>> {
        validate_outcome_status(status)?;
        if self.connection.execute(
            "UPDATE epics SET status = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![status, id],
        )? == 0
        {
            return Ok(None);
        }
        self.epic_by_id(id)
    }

    pub fn project_progress(&self, project_id: &str) -> Result<ProjectProgress> {
        let milestones = self.list_milestones(project_id)?;
        let epics = self.list_epics(project_id)?;
        let milestone_progress = milestones
            .into_iter()
            .map(|milestone| {
                Ok(MilestoneProgress {
                    counts: self.task_progress_counts(project_id, Some(&milestone.id))?,
                    epics: epics
                        .iter()
                        .filter(|epic| epic.milestone_id.as_deref() == Some(milestone.id.as_str()))
                        .cloned()
                        .collect(),
                    milestone,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(ProjectProgress {
            counts: self.task_progress_counts(project_id, None)?,
            milestones: milestone_progress,
        })
    }

    pub fn create_task(&mut self, new_task: NewTask) -> Result<Task> {
        self.validate_task_dependencies(
            &new_task.id,
            &new_task.project_id,
            &new_task.dependency_ids,
        )?;
        self.validate_task_hierarchy(
            &new_task.project_id,
            new_task.milestone_id.as_deref(),
            new_task.epic_id.as_deref(),
        )?;
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
                                implementation_notes, relevant_paths, dependency_ids, assigned_agent_id, priority, milestone_id, epic_id, status, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'backlog', ?13)",
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
                new_task.priority.as_str(),
                new_task.milestone_id,
                new_task.epic_id,
                position
            ],
        )?;
        transaction.commit()?;
        self.task_by_id(&new_task.id)?
            .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn update_task(&mut self, id: &str, update: TaskUpdate) -> Result<Option<Task>> {
        let project_id = self.task_location(id)?.map(|(project_id, _)| project_id);
        let Some(project_id) = project_id else {
            return Ok(None);
        };
        self.validate_task_dependencies(id, &project_id, &update.dependency_ids)?;
        self.validate_task_hierarchy(
            &project_id,
            update.milestone_id.as_deref(),
            update.epic_id.as_deref(),
        )?;
        let acceptance_criteria = encode_string_list(&update.acceptance_criteria)?;
        let relevant_paths = encode_string_list(&update.relevant_paths)?;
        let dependency_ids = encode_string_list(&update.dependency_ids)?;
        let changed = self.connection.execute(
            "UPDATE tasks SET title = ?1, description = ?2, acceptance_criteria = ?3,
                              implementation_notes = ?4, relevant_paths = ?5, dependency_ids = ?6,
                              assigned_agent_id = ?7, priority = ?8, milestone_id = ?9, epic_id = ?10, updated_at = CURRENT_TIMESTAMP WHERE id = ?11",
            params![
                update.title,
                update.description,
                acceptance_criteria,
                update.implementation_notes,
                relevant_paths,
                dependency_ids,
                update.assigned_agent_id,
                update.priority.as_str(),
                update.milestone_id,
                update.epic_id,
                id
            ],
        )?;
        if changed == 0 {
            return Ok(None);
        }
        self.recalculate_project_task_readiness(&project_id)?;
        self.task_by_id(id)
    }

    pub fn delete_task(&mut self, id: &str) -> Result<bool> {
        let Some((project_id, status)) = self.task_location(id)? else {
            return Ok(false);
        };
        if self.task_has_dependents(id)? {
            return Err(rusqlite::Error::InvalidParameterName(
                "Remove this task from its dependents before deleting it.".into(),
            ));
        }
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

    pub fn list_agent_reviews(&self, task_id: &str) -> Result<Vec<AgentReview>> {
        let mut statement = self.connection.prepare(
            "SELECT id, task_id, agent_id, status, decision, notes, raw_output, error, started_at, completed_at
             FROM agent_reviews WHERE task_id = ?1 ORDER BY started_at DESC, id DESC",
        )?;
        let reviews = statement
            .query_map([task_id], agent_review_from_row)?
            .collect::<Result<Vec<_>>>()?;
        Ok(reviews)
    }

    pub fn get_agent_review(&self, id: &str) -> Result<Option<AgentReview>> {
        self.agent_review_by_id(id)
    }

    pub fn start_agent_review(&mut self, review: NewAgentReview) -> Result<AgentReview> {
        let (task_status, assigned_agent_id): (String, Option<String>) =
            self.connection.query_row(
                "SELECT status, assigned_agent_id FROM tasks WHERE id = ?1",
                [&review.task_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
        if task_status != TaskStatus::Review.as_str() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        if assigned_agent_id.as_deref() == Some(review.agent_id.as_str()) {
            return Err(rusqlite::Error::InvalidParameterName(
                "An implementation agent cannot review its own task.".into(),
            ));
        }
        if !self.agent_exists(&review.agent_id)? {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        self.connection.execute(
            "INSERT INTO agent_reviews (id, task_id, agent_id, status) VALUES (?1, ?2, ?3, 'running')",
            params![review.id, review.task_id, review.agent_id],
        )?;
        self.agent_review_by_id(&review.id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn append_agent_review_output(&mut self, id: &str, output: &str) -> Result<()> {
        self.connection.execute(
            "UPDATE agent_reviews
             SET raw_output = substr(raw_output || ?1, 1, 200000)
             WHERE id = ?2 AND status = 'running'",
            params![output, id],
        )?;
        Ok(())
    }

    pub fn finish_agent_review(
        &mut self,
        id: &str,
        status: AgentReviewStatus,
        decision: Option<AgentReviewDecision>,
        notes: Option<&str>,
        error: Option<&str>,
    ) -> Result<Option<AgentReview>> {
        self.connection.execute(
            "UPDATE agent_reviews
             SET status = ?1, decision = ?2, notes = ?3, error = ?4, completed_at = CURRENT_TIMESTAMP
             WHERE id = ?5 AND status = 'running'",
            params![
                status.as_str(),
                decision.map(AgentReviewDecision::as_str),
                notes,
                error,
                id
            ],
        )?;
        self.agent_review_by_id(id)
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

    pub fn list_running_remote_runs(&self, local_worker_id: &str) -> Result<Vec<Run>> {
        let run_ids = {
            let mut statement = self.connection.prepare(
                "SELECT id FROM runs WHERE status = 'running' AND worker_id <> ?1
                 ORDER BY started_at ASC, id ASC",
            )?;
            let run_ids = statement
                .query_map([local_worker_id], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>>>()?;
            run_ids
        };
        run_ids
            .into_iter()
            .map(|run_id| {
                self.get_run(&run_id)?
                    .ok_or(rusqlite::Error::QueryReturnedNoRows)
            })
            .collect()
    }

    pub fn worker_has_active_runs(&self, worker_id: &str) -> Result<bool> {
        self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM runs WHERE worker_id = ?1 AND status IN ('queued', 'running'))",
            [worker_id],
            |row| row.get(0),
        )
    }

    pub fn enqueue_run(&mut self, new_run: NewRun) -> Result<(Run, Task)> {
        let transaction = self.connection.transaction()?;
        let (project_id, task_status, assigned_agent_id): (String, String, Option<String>) =
            transaction.query_row(
                "SELECT project_id, status, assigned_agent_id FROM tasks WHERE id = ?1",
                [&new_run.task_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
        if task_status != TaskStatus::Ready.as_str()
            || assigned_agent_id.as_deref() != Some(new_run.agent_id.as_str())
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        if let Some(reason) = task_blocked_reason(&transaction, &new_run.task_id, &project_id)? {
            return Err(rusqlite::Error::InvalidParameterName(reason));
        }

        transaction.execute(
            "INSERT INTO runs (id, task_id, agent_id, worker_id, status, queued_at) VALUES (?1, ?2, ?3, ?4, 'queued', CURRENT_TIMESTAMP)",
            params![new_run.id, new_run.task_id, new_run.agent_id, new_run.worker_id],
        )?;
        transaction.execute(
            "INSERT INTO run_events (run_id, kind, message) VALUES (?1, 'run.queued', 'Agent run queued for worker execution.')",
            [&new_run.id],
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

    pub fn start_run(&mut self, new_run: NewRun) -> Result<(Run, Task)> {
        let worker_id = new_run.worker_id.clone();
        let run_id = new_run.id.clone();
        self.enqueue_run(new_run)?;
        match self.claim_next_run(&worker_id)? {
            Some((run, task)) if run.id == run_id => Ok((run, task)),
            _ => Err(rusqlite::Error::InvalidParameterName(
                "The run was queued because execution capacity is not currently available.".into(),
            )),
        }
    }

    pub fn claim_next_run(&mut self, worker_id: &str) -> Result<Option<(Run, Task)>> {
        let transaction = self.connection.transaction()?;
        let worker_limit = worker_limit_in_transaction(&transaction, worker_id)?;
        let active_worker_runs: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM runs WHERE worker_id = ?1 AND status = 'running'",
            [worker_id],
            |row| row.get(0),
        )?;
        if active_worker_runs >= worker_limit {
            return Ok(None);
        }

        let candidates = {
            let mut statement = transaction.prepare(
                "SELECT runs.id, runs.task_id, tasks.project_id, runs.agent_id, agents.max_concurrent_tasks
                 FROM runs
                 JOIN tasks ON tasks.id = runs.task_id
                 JOIN agents ON agents.id = runs.agent_id
                 WHERE runs.worker_id = ?1 AND runs.status = 'queued' AND tasks.status = 'ready'
                 ORDER BY CASE tasks.priority
                    WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'normal' THEN 2 ELSE 3 END,
                    runs.queued_at ASC, runs.id ASC",
            )?;
            let records = statement
                .query_map([worker_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                })?
                .collect::<Result<Vec<_>>>()?;
            records
        };

        let mut selected = None;
        for (run_id, task_id, project_id, agent_id, agent_limit) in candidates {
            if flow_blocked_reason_in_transaction(&transaction, &project_id)?.is_some() {
                continue;
            }
            if active_task_blocker_reason(&transaction, &task_id, &project_id)?.is_some() {
                continue;
            }
            let active_agent_runs: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM runs WHERE agent_id = ?1 AND status = 'running'",
                [&agent_id],
                |row| row.get(0),
            )?;
            if active_agent_runs < agent_limit {
                selected = Some((run_id, task_id, project_id));
                break;
            }
        }
        let Some((run_id, task_id, project_id)) = selected else {
            return Ok(None);
        };

        transaction.execute(
            "UPDATE runs SET status = 'running', started_at = CURRENT_TIMESTAMP WHERE id = ?1 AND status = 'queued'",
            [&run_id],
        )?;
        transaction.execute(
            "INSERT INTO run_events (run_id, kind, message) VALUES (?1, 'run.started', 'Agent run claimed by its execution worker.')",
            [&run_id],
        )?;
        move_task_in_transaction(
            &transaction,
            &task_id,
            &project_id,
            TaskStatus::Ready,
            TaskStatus::InProgress,
            usize::MAX,
        )?;
        transaction.commit()?;

        let run = self
            .get_run(&run_id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        let task = self
            .task_by_id(&task_id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        Ok(Some((run, task)))
    }

    pub fn list_queued_runs(&self, project_id: &str) -> Result<Vec<Run>> {
        let mut statement = self.connection.prepare(
            "SELECT runs.id, runs.task_id, runs.agent_id, runs.worker_id, runs.status,
                    runs.started_at, runs.completed_at, runs.exit_code, runs.error
             FROM runs JOIN tasks ON tasks.id = runs.task_id
             WHERE tasks.project_id = ?1 AND runs.status = 'queued'
             ORDER BY CASE tasks.priority
                WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'normal' THEN 2 ELSE 3 END,
                runs.queued_at ASC, runs.id ASC",
        )?;
        let records = statement
            .query_map([project_id], run_from_row)?
            .collect::<Result<Vec<_>>>()?;
        Ok(records)
    }

    pub fn cancel_queued_run(&mut self, run_id: &str) -> Result<bool> {
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE runs SET status = 'cancelled', completed_at = CURRENT_TIMESTAMP
             WHERE id = ?1 AND status = 'queued'",
            [run_id],
        )?;
        if changed > 0 {
            transaction.execute(
                "INSERT INTO run_events (run_id, kind, message) VALUES (?1, 'run.cancelled', 'Queued agent run cancelled.')",
                [run_id],
            )?;
        }
        transaction.commit()?;
        Ok(changed > 0)
    }

    pub fn queue_run_recovery(
        &mut self,
        source_run_id: &str,
        replacement_run_id: &str,
        recovery_id: &str,
        agent_id: &str,
        action: &str,
    ) -> Result<Option<(Run, Task)>> {
        if !matches!(action, "resume" | "restart_clean" | "reassign") {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let transaction = self.connection.transaction()?;
        let source = transaction
            .query_row(
                "SELECT runs.task_id, runs.status, tasks.project_id, tasks.status
             FROM runs JOIN tasks ON tasks.id = runs.task_id WHERE runs.id = ?1",
                [source_run_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((task_id, run_status, project_id, task_status)) = source else {
            return Ok(None);
        };
        if !matches!(run_status.as_str(), "failed" | "cancelled")
            || task_status != TaskStatus::InProgress.as_str()
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let valid_agent: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM agents WHERE id = ?1 AND provider = 'codex')",
            [agent_id],
            |row| row.get(0),
        )?;
        if !valid_agent {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let active_run: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM runs WHERE task_id = ?1 AND status IN ('queued', 'running'))",
            [&task_id],
            |row| row.get(0),
        )?;
        if active_run {
            return Err(rusqlite::Error::InvalidQuery);
        }
        transaction.execute(
            "UPDATE tasks SET assigned_agent_id = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![agent_id, task_id],
        )?;
        move_task_in_transaction(
            &transaction,
            &task_id,
            &project_id,
            TaskStatus::InProgress,
            TaskStatus::Ready,
            usize::MAX,
        )?;
        transaction.execute(
            "INSERT INTO runs (id, task_id, agent_id, worker_id, status, queued_at)
             VALUES (?1, ?2, ?3, 'local', 'queued', CURRENT_TIMESTAMP)",
            params![replacement_run_id, task_id, agent_id],
        )?;
        transaction.execute(
            "INSERT INTO run_recoveries (id, task_id, source_run_id, replacement_run_id, agent_id, action)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![recovery_id, task_id, source_run_id, replacement_run_id, agent_id, action],
        )?;
        transaction.execute(
            "INSERT INTO run_events (run_id, kind, message)
             VALUES (?1, 'recovery.queued', ?2)",
            params![
                replacement_run_id,
                format!("Recovery queued from run {source_run_id} using {action}.")
            ],
        )?;
        transaction.execute(
            "INSERT INTO run_events (run_id, kind, message)
             VALUES (?1, 'recovery.replaced', ?2)",
            params![
                source_run_id,
                format!("Recovery continued as run {replacement_run_id}.")
            ],
        )?;
        transaction.commit()?;
        Ok(Some((
            self.get_run(replacement_run_id)?
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?,
            self.task_by_id(&task_id)?
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?,
        )))
    }

    pub fn resolve_failed_run(
        &mut self,
        source_run_id: &str,
        recovery_id: &str,
        action: &str,
        note: Option<&str>,
    ) -> Result<Option<Task>> {
        if !matches!(action, "abandon" | "escalate") {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let transaction = self.connection.transaction()?;
        let source = transaction
            .query_row(
                "SELECT runs.task_id, runs.agent_id, runs.status, tasks.project_id, tasks.status
             FROM runs JOIN tasks ON tasks.id = runs.task_id WHERE runs.id = ?1",
                [source_run_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((task_id, agent_id, run_status, project_id, task_status)) = source else {
            return Ok(None);
        };
        if !matches!(run_status.as_str(), "failed" | "cancelled")
            || task_status != TaskStatus::InProgress.as_str()
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let target_status = if action == "abandon" {
            TaskStatus::Backlog
        } else {
            TaskStatus::Blocked
        };
        move_task_in_transaction(
            &transaction,
            &task_id,
            &project_id,
            TaskStatus::InProgress,
            target_status,
            usize::MAX,
        )?;
        if action == "escalate" {
            transaction.execute(
                "UPDATE tasks SET blocked_reason = ?1, readiness_blocked = 0, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
                params![note.unwrap_or("A failed agent run requires human recovery."), task_id],
            )?;
        }
        transaction.execute(
            "INSERT INTO run_recoveries (id, task_id, source_run_id, agent_id, action, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![recovery_id, task_id, source_run_id, agent_id, action, note],
        )?;
        transaction.execute(
            "INSERT INTO run_events (run_id, kind, message) VALUES (?1, ?2, ?3)",
            params![
                source_run_id,
                format!("recovery.{action}"),
                note.unwrap_or(if action == "abandon" {
                    "Run recovery abandoned; worktree preserved."
                } else {
                    "Run escalated for human recovery."
                })
            ],
        )?;
        transaction.commit()?;
        self.task_by_id(&task_id)
    }

    pub fn flow_state(&self, project_id: &str, worker_id: &str) -> Result<FlowState> {
        flow_state_for_connection(&self.connection, project_id, worker_id)
    }

    pub fn update_flow_limits(
        &mut self,
        project_id: &str,
        worker_id: &str,
        update: FlowLimitUpdate,
    ) -> Result<FlowState> {
        if [
            update.worker_max_concurrent_runs,
            update.in_progress_limit,
            update.review_limit,
            update.approved_limit,
        ]
        .into_iter()
        .any(|value| !(1..=32).contains(&value))
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO project_flow_limits (project_id, in_progress_limit, review_limit, approved_limit)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(project_id) DO UPDATE SET
                in_progress_limit = excluded.in_progress_limit,
                review_limit = excluded.review_limit,
                approved_limit = excluded.approved_limit,
                updated_at = CURRENT_TIMESTAMP",
            params![
                project_id,
                update.in_progress_limit,
                update.review_limit,
                update.approved_limit
            ],
        )?;
        transaction.execute(
            "INSERT INTO worker_flow_limits (worker_id, max_concurrent_runs) VALUES (?1, ?2)
             ON CONFLICT(worker_id) DO UPDATE SET
                max_concurrent_runs = excluded.max_concurrent_runs,
                updated_at = CURRENT_TIMESTAMP",
            params![worker_id, update.worker_max_concurrent_runs],
        )?;
        transaction.commit()?;
        self.flow_state(project_id, worker_id)
    }

    pub fn assign_task_worktree(
        &mut self,
        id: &str,
        branch: &str,
        worktree_path: &str,
    ) -> Result<Option<Task>> {
        let changed = self.connection.execute(
            "UPDATE tasks SET branch = ?1, worktree_path = ?2, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?3 AND status IN ('ready', 'in_progress') AND worktree_path IS NULL",
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
            TaskStatus::NeedsInput
                | TaskStatus::Approved
                | TaskStatus::Integrating
                | TaskStatus::Blocked
                | TaskStatus::Done
        ) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let Some((project_id, source_status)) = self.task_location(id)? else {
            return Ok(None);
        };
        if matches!(
            source_status,
            TaskStatus::NeedsInput
                | TaskStatus::Approved
                | TaskStatus::Integrating
                | TaskStatus::Blocked
                | TaskStatus::Done
        ) {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let transaction = self.connection.transaction()?;
        let blocked_reason = if target_status == TaskStatus::Ready {
            task_blocked_reason(&transaction, id, &project_id)?
        } else {
            None
        };
        let resulting_status = if blocked_reason.is_some() {
            TaskStatus::Blocked
        } else {
            target_status
        };
        move_task_in_transaction(
            &transaction,
            id,
            &project_id,
            source_status,
            resulting_status,
            target_position,
        )?;
        transaction.execute(
            "UPDATE tasks SET blocked_reason = ?1, readiness_blocked = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?3",
            params![blocked_reason, blocked_reason.is_some(), id],
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

    pub fn clear_integration_cleanup_error(&mut self, attempt_id: &str) -> Result<bool> {
        Ok(self.connection.execute(
            "UPDATE integration_attempts SET error = NULL, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1 AND status = 'merged'",
            [attempt_id],
        )? > 0)
    }

    pub fn recover_interrupted_integrations(&mut self) -> Result<usize> {
        let transaction = self.connection.transaction()?;
        let interrupted = {
            let mut statement = transaction.prepare(
                "SELECT attempts.id, attempts.task_id, tasks.project_id
                 FROM integration_attempts AS attempts
                 JOIN tasks ON tasks.id = attempts.task_id
                 JOIN project_integration_locks AS locks ON locks.attempt_id = attempts.id
                 WHERE attempts.status = 'integrating' AND tasks.status = 'integrating'",
            )?;
            let records = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>>>()?;
            records
        };
        for (attempt_id, task_id, project_id) in &interrupted {
            transaction.execute(
                "UPDATE integration_attempts SET status = 'failed', error = 'Integration was interrupted when Orchestr stopped. Repository state was preserved for inspection and retry.', completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
                [attempt_id],
            )?;
            move_task_in_transaction(
                &transaction,
                task_id,
                project_id,
                TaskStatus::Integrating,
                TaskStatus::Approved,
                usize::MAX,
            )?;
        }
        transaction.execute("DELETE FROM project_integration_locks", [])?;
        transaction.commit()?;
        Ok(interrupted.len())
    }

    pub fn list_revert_attempts(&self, project_id: &str) -> Result<Vec<RevertAttempt>> {
        let mut statement = self.connection.prepare(
            "SELECT id, project_id, original_task_id, integration_attempt_id, original_commit,
                    status, revert_commit, repair_task_id, error, started_at, completed_at
             FROM revert_attempts WHERE project_id = ?1 ORDER BY started_at DESC",
        )?;
        let records = statement
            .query_map([project_id], revert_attempt_from_row)?
            .collect();
        records
    }

    pub fn begin_revert(
        &mut self,
        id: &str,
        integration_attempt_id: &str,
    ) -> Result<Option<RevertAttempt>> {
        let transaction = self.connection.transaction()?;
        let integration = transaction.query_row(
            "SELECT tasks.project_id, attempts.task_id, attempts.merge_commit
             FROM integration_attempts AS attempts JOIN tasks ON tasks.id = attempts.task_id
             WHERE attempts.id = ?1 AND attempts.status = 'merged' AND attempts.merge_commit IS NOT NULL",
            [integration_attempt_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
        ).optional()?;
        let Some((project_id, task_id, commit)) = integration else {
            return Ok(None);
        };
        let already_reverted: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM revert_attempts WHERE integration_attempt_id = ?1 AND status IN ('reverted', 'validation_failed'))",
            [integration_attempt_id],
            |row| row.get(0),
        )?;
        if already_reverted {
            return Err(rusqlite::Error::InvalidQuery);
        }
        transaction.execute(
            "INSERT INTO revert_attempts (id, project_id, original_task_id, integration_attempt_id, original_commit, status)
             VALUES (?1, ?2, ?3, ?4, ?5, 'running')",
            params![id, project_id, task_id, integration_attempt_id, commit],
        )?;
        transaction.commit()?;
        self.revert_attempt_by_id(id)
    }

    pub fn finish_revert(
        &mut self,
        id: &str,
        status: RevertStatus,
        revert_commit: Option<&str>,
        error: Option<&str>,
        repair_task_id: Option<&str>,
    ) -> Result<Option<RevertAttempt>> {
        if status == RevertStatus::Running {
            return Err(rusqlite::Error::InvalidQuery);
        }
        self.connection.execute(
            "UPDATE revert_attempts SET status = ?1, revert_commit = ?2, error = ?3,
             repair_task_id = ?4, completed_at = CURRENT_TIMESTAMP WHERE id = ?5 AND status = 'running'",
            params![status.as_str(), revert_commit, error, repair_task_id, id],
        )?;
        self.revert_attempt_by_id(id)
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

    fn milestone_by_id(&self, id: &str) -> Result<Option<Milestone>> {
        self.connection.query_row(
            "SELECT id, project_id, title, description, status, target_date, created_at, updated_at
             FROM milestones WHERE id = ?1",
            [id],
            milestone_from_row,
        ).optional()
    }

    fn epic_by_id(&self, id: &str) -> Result<Option<Epic>> {
        self.connection.query_row(
            "SELECT id, project_id, milestone_id, title, description, status, created_at, updated_at
             FROM epics WHERE id = ?1",
            [id],
            epic_from_row,
        ).optional()
    }

    fn validate_milestone_for_project(
        &self,
        project_id: &str,
        milestone_id: Option<&str>,
    ) -> Result<()> {
        let Some(milestone_id) = milestone_id else {
            return Ok(());
        };
        let milestone_project = self
            .connection
            .query_row(
                "SELECT project_id FROM milestones WHERE id = ?1",
                [milestone_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if milestone_project.as_deref() == Some(project_id) {
            Ok(())
        } else {
            Err(rusqlite::Error::InvalidParameterName(
                "The selected milestone does not belong to this project.".into(),
            ))
        }
    }

    fn validate_task_hierarchy(
        &self,
        project_id: &str,
        milestone_id: Option<&str>,
        epic_id: Option<&str>,
    ) -> Result<()> {
        self.validate_milestone_for_project(project_id, milestone_id)?;
        let Some(epic_id) = epic_id else {
            return Ok(());
        };
        let epic = self
            .connection
            .query_row(
                "SELECT project_id, milestone_id FROM epics WHERE id = ?1",
                [epic_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        let Some((epic_project_id, epic_milestone_id)) = epic else {
            return Err(rusqlite::Error::InvalidParameterName(
                "The selected epic does not exist.".into(),
            ));
        };
        if epic_project_id != project_id {
            return Err(rusqlite::Error::InvalidParameterName(
                "The selected epic does not belong to this project.".into(),
            ));
        }
        if let (Some(task_milestone_id), Some(epic_milestone_id)) =
            (milestone_id, epic_milestone_id.as_deref())
        {
            if task_milestone_id != epic_milestone_id {
                return Err(rusqlite::Error::InvalidParameterName(
                    "A task's milestone must match its epic's milestone.".into(),
                ));
            }
        }
        Ok(())
    }

    fn task_progress_counts(
        &self,
        project_id: &str,
        milestone_id: Option<&str>,
    ) -> Result<TaskProgressCounts> {
        let mut statement = self.connection.prepare(
            "SELECT status, COUNT(*) FROM tasks WHERE project_id = ?1
             AND (?2 IS NULL OR milestone_id = ?2) GROUP BY status",
        )?;
        let mut counts = TaskProgressCounts::default();
        let rows = statement.query_map(params![project_id, milestone_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (status, count) = row?;
            counts.total += count;
            record_task_progress_count(&mut counts, &status, count);
        }
        Ok(counts)
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

    fn revert_attempt_by_id(&self, id: &str) -> Result<Option<RevertAttempt>> {
        self.connection
            .query_row(
                "SELECT id, project_id, original_task_id, integration_attempt_id, original_commit,
                    status, revert_commit, repair_task_id, error, started_at, completed_at
             FROM revert_attempts WHERE id = ?1",
                [id],
                revert_attempt_from_row,
            )
            .optional()
    }

    fn agent_review_by_id(&self, id: &str) -> Result<Option<AgentReview>> {
        self.connection
            .query_row(
                "SELECT id, task_id, agent_id, status, decision, notes, raw_output, error, started_at, completed_at
                 FROM agent_reviews WHERE id = ?1",
                [id],
                agent_review_from_row,
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
        transaction.execute(
            "UPDATE tasks SET blocked_reason = ?1, readiness_blocked = 0, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![
                (task_status == TaskStatus::Blocked).then_some(error),
                task_id
            ],
        )?;
        transaction.commit()?;
        if task_status == TaskStatus::Done {
            self.recalculate_project_task_readiness(&project_id)?;
        }
        self.task_by_id(&task_id)
    }

    fn task_by_id(&self, id: &str) -> Result<Option<Task>> {
        self.connection
            .query_row(
                "SELECT id, project_id, title, description, acceptance_criteria, implementation_notes,
                    relevant_paths, dependency_ids, assigned_agent_id, branch, worktree_path, priority, blocked_reason, readiness_blocked, milestone_id, epic_id, status, position, created_at, updated_at
                 FROM tasks WHERE id = ?1",
                [id],
                task_from_row,
            )
            .optional()
    }

    fn task_input_request_by_id(&self, id: &str) -> Result<Option<TaskInputRequest>> {
        self.connection
            .query_row(
                "SELECT id, task_id, requesting_run_id, requesting_agent_id, question, status,
                        answer, requested_at, answered_at
                 FROM task_input_requests WHERE id = ?1",
                [id],
                task_input_request_from_row,
            )
            .optional()
    }

    fn project_blocker_by_id(&self, id: &str) -> Result<Option<ProjectBlocker>> {
        let mut blocker = self
            .connection
            .query_row(
                "SELECT id, project_id, title, description, affects_all_tasks, status,
                        created_at, resolved_at
                 FROM project_blockers WHERE id = ?1",
                [id],
                project_blocker_from_row,
            )
            .optional()?;
        if let Some(record) = &mut blocker {
            record.affected_task_ids = self.blocker_task_ids(&record.id)?;
        }
        Ok(blocker)
    }

    fn architecture_decision_by_id(&self, id: &str) -> Result<Option<ArchitectureDecision>> {
        let decision = self
            .connection
            .query_row(
                "SELECT id, project_id, decision_number, title, context, decision, consequences,
                        status, supersedes_decision_id, relevant_paths, created_at, updated_at, decided_at
                 FROM architecture_decisions WHERE id = ?1",
                [id],
                architecture_decision_from_row,
            )
            .optional()?;
        decision
            .map(|record| self.with_architecture_decision_tasks(record))
            .transpose()
    }

    fn with_architecture_decision_tasks(
        &self,
        mut decision: ArchitectureDecision,
    ) -> Result<ArchitectureDecision> {
        let mut statement = self.connection.prepare(
            "SELECT task_id FROM architecture_decision_tasks
             WHERE decision_id = ?1 ORDER BY task_id",
        )?;
        decision.relevant_task_ids = statement
            .query_map([&decision.id], |row| row.get(0))?
            .collect::<Result<Vec<_>>>()?;
        Ok(decision)
    }

    fn with_remote_worker_workspaces(&self, mut worker: RemoteWorker) -> Result<RemoteWorker> {
        let mut statement = self.connection.prepare(
            "SELECT project_id, workspace_path, enabled FROM remote_worker_projects
             WHERE worker_id = ?1 ORDER BY created_at ASC",
        )?;
        worker.workspaces = statement
            .query_map([&worker.id], |row| {
                Ok(RemoteWorkerWorkspace {
                    project_id: row.get(0)?,
                    workspace_path: row.get(1)?,
                    enabled: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(worker)
    }

    fn blocker_task_ids(&self, blocker_id: &str) -> Result<Vec<String>> {
        let mut statement = self.connection.prepare(
            "SELECT task_id FROM project_blocker_tasks WHERE blocker_id = ?1 ORDER BY task_id",
        )?;
        let task_ids = statement
            .query_map([blocker_id], |row| row.get(0))?
            .collect();
        task_ids
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

    fn validate_task_dependencies(
        &self,
        task_id: &str,
        project_id: &str,
        dependency_ids: &[String],
    ) -> Result<()> {
        let unique_ids = dependency_ids.iter().collect::<HashSet<_>>();
        if unique_ids.len() != dependency_ids.len() {
            return Err(rusqlite::Error::InvalidParameterName(
                "A task cannot list the same dependency more than once.".into(),
            ));
        }
        if dependency_ids
            .iter()
            .any(|dependency_id| dependency_id == task_id)
        {
            return Err(rusqlite::Error::InvalidParameterName(
                "A task cannot depend on itself.".into(),
            ));
        }
        let mut statement = self
            .connection
            .prepare("SELECT id, project_id, dependency_ids FROM tasks")?;
        let tasks = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    decode_string_list(row.get(2)?)?,
                ))
            })?
            .collect::<Result<Vec<_>>>()?;
        let task_dependencies = tasks
            .into_iter()
            .map(|(id, task_project_id, dependencies)| (id, (task_project_id, dependencies)))
            .collect::<HashMap<_, _>>();
        for dependency_id in dependency_ids {
            match task_dependencies.get(dependency_id) {
                Some((dependency_project_id, _)) if dependency_project_id == project_id => {}
                Some(_) => {
                    return Err(rusqlite::Error::InvalidParameterName(
                        "Dependencies must belong to the same project.".into(),
                    ))
                }
                None => {
                    return Err(rusqlite::Error::InvalidParameterName(format!(
                        "Dependency {dependency_id} does not exist in this project."
                    )))
                }
            }
        }
        let mut visited = HashSet::new();
        let mut pending = dependency_ids.to_vec();
        while let Some(current_id) = pending.pop() {
            if current_id == task_id {
                return Err(rusqlite::Error::InvalidParameterName(
                    "This dependency would create a cycle.".into(),
                ));
            }
            if !visited.insert(current_id.clone()) {
                continue;
            }
            if let Some((_, dependencies)) = task_dependencies.get(&current_id) {
                pending.extend(dependencies.iter().cloned());
            }
        }
        Ok(())
    }

    fn task_has_dependents(&self, id: &str) -> Result<bool> {
        let mut statement = self
            .connection
            .prepare("SELECT dependency_ids FROM tasks WHERE id <> ?1")?;
        let dependencies = statement
            .query_map([id], |row| decode_string_list(row.get(0)?))?
            .collect::<Result<Vec<_>>>()?;
        Ok(dependencies.iter().any(|dependency_ids| {
            dependency_ids
                .iter()
                .any(|dependency_id| dependency_id == id)
        }))
    }

    fn recalculate_all_task_readiness(&mut self) -> Result<()> {
        let project_ids = {
            let mut statement = self.connection.prepare("SELECT id FROM projects")?;
            let project_ids = statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>>>()?;
            project_ids
        };
        for project_id in project_ids {
            self.recalculate_project_task_readiness(&project_id)?;
        }
        Ok(())
    }

    fn recalculate_project_task_readiness(&mut self, project_id: &str) -> Result<()> {
        let transaction = self.connection.transaction()?;
        let candidates = {
            let mut statement = transaction.prepare(
                "SELECT id, status, readiness_blocked FROM tasks
                 WHERE project_id = ?1
                 AND (status = 'ready' OR (status = 'blocked' AND readiness_blocked = 1))
                 AND NOT EXISTS (
                    SELECT 1 FROM runs WHERE runs.task_id = tasks.id
                    AND runs.status IN ('queued', 'running')
                 )",
            )?;
            let candidates = statement
                .query_map([project_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        TaskStatus::from_database(row.get(1)?)?,
                        row.get::<_, bool>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>>>()?;
            candidates
        };
        for (task_id, status, _) in candidates {
            let reason = task_blocked_reason(&transaction, &task_id, project_id)?;
            let target_status = if reason.is_some() {
                TaskStatus::Blocked
            } else {
                TaskStatus::Ready
            };
            if status != target_status {
                move_task_in_transaction(
                    &transaction,
                    &task_id,
                    project_id,
                    status,
                    target_status,
                    usize::MAX,
                )?;
            }
            transaction.execute(
                "UPDATE tasks SET blocked_reason = ?1, readiness_blocked = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?3",
                params![reason, reason.is_some(), task_id],
            )?;
        }
        transaction.commit()
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

fn task_input_request_from_row(row: &rusqlite::Row<'_>) -> Result<TaskInputRequest> {
    Ok(TaskInputRequest {
        id: row.get(0)?,
        task_id: row.get(1)?,
        requesting_run_id: row.get(2)?,
        requesting_agent_id: row.get(3)?,
        question: row.get(4)?,
        status: row.get(5)?,
        answer: row.get(6)?,
        requested_at: row.get(7)?,
        answered_at: row.get(8)?,
    })
}

fn project_blocker_from_row(row: &rusqlite::Row<'_>) -> Result<ProjectBlocker> {
    Ok(ProjectBlocker {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        affects_all_tasks: row.get(4)?,
        affected_task_ids: Vec::new(),
        status: row.get(5)?,
        created_at: row.get(6)?,
        resolved_at: row.get(7)?,
    })
}

fn architecture_decision_from_row(row: &rusqlite::Row<'_>) -> Result<ArchitectureDecision> {
    Ok(ArchitectureDecision {
        id: row.get(0)?,
        project_id: row.get(1)?,
        decision_number: row.get(2)?,
        title: row.get(3)?,
        context: row.get(4)?,
        decision: row.get(5)?,
        consequences: row.get(6)?,
        status: ArchitectureDecisionStatus::from_database(row.get(7)?)?,
        supersedes_decision_id: row.get(8)?,
        relevant_paths: decode_string_list(row.get(9)?)?,
        relevant_task_ids: Vec::new(),
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        decided_at: row.get(12)?,
    })
}

fn remote_worker_from_row(row: &rusqlite::Row<'_>) -> Result<RemoteWorker> {
    let tools = serde_json::from_str(&row.get::<_, String>(9)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, error.into())
    })?;
    Ok(RemoteWorker {
        id: row.get(0)?,
        name: row.get(1)?,
        endpoint: row.get(2)?,
        token_environment_variable: row.get(3)?,
        ca_certificate_pem: row.get(4)?,
        os: row.get(5)?,
        architecture: row.get(6)?,
        status: row.get(7)?,
        protocol_version: row.get(8)?,
        tools,
        workspaces: Vec::new(),
        last_seen_at: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn record_task_progress_count(counts: &mut TaskProgressCounts, status: &str, count: i64) {
    match status {
        "backlog" => counts.backlog = count,
        "ready" => counts.ready = count,
        "in_progress" => counts.in_progress = count,
        "needs_input" => counts.needs_input = count,
        "review" => counts.review = count,
        "blocked" => counts.blocked = count,
        "done" => counts.done = count,
        _ => {}
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
        priority: TaskPriority::from_database(row.get(11)?)?,
        blocked_reason: row.get(12)?,
        readiness_blocked: row.get(13)?,
        milestone_id: row.get(14)?,
        epic_id: row.get(15)?,
        status: TaskStatus::from_database(row.get(16)?)?,
        position: row.get(17)?,
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
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

fn agent_review_from_row(row: &rusqlite::Row<'_>) -> Result<AgentReview> {
    Ok(AgentReview {
        id: row.get(0)?,
        task_id: row.get(1)?,
        agent_id: row.get(2)?,
        status: AgentReviewStatus::from_database(row.get(3)?)?,
        decision: row
            .get::<_, Option<String>>(4)?
            .map(AgentReviewDecision::from_database)
            .transpose()?,
        notes: row.get(5)?,
        raw_output: row.get(6)?,
        error: row.get(7)?,
        started_at: row.get(8)?,
        completed_at: row.get(9)?,
    })
}

fn milestone_from_row(row: &rusqlite::Row<'_>) -> Result<Milestone> {
    Ok(Milestone {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        status: row.get(4)?,
        target_date: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn epic_from_row(row: &rusqlite::Row<'_>) -> Result<Epic> {
    Ok(Epic {
        id: row.get(0)?,
        project_id: row.get(1)?,
        milestone_id: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        status: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
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

fn revert_attempt_from_row(row: &rusqlite::Row<'_>) -> Result<RevertAttempt> {
    Ok(RevertAttempt {
        id: row.get(0)?,
        project_id: row.get(1)?,
        original_task_id: row.get(2)?,
        integration_attempt_id: row.get(3)?,
        original_commit: row.get(4)?,
        status: RevertStatus::from_database(row.get(5)?)?,
        revert_commit: row.get(6)?,
        repair_task_id: row.get(7)?,
        error: row.get(8)?,
        started_at: row.get(9)?,
        completed_at: row.get(10)?,
    })
}

fn json_conversion_error(error: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(error.into())
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

fn validate_outcome_status(status: &str) -> Result<()> {
    if matches!(status, "planned" | "active" | "completed" | "blocked") {
        Ok(())
    } else {
        Err(rusqlite::Error::InvalidParameterName(
            "Unknown milestone or epic status.".into(),
        ))
    }
}

fn flow_limits_for_connection(
    connection: &Connection,
    project_id: &str,
    worker_id: &str,
) -> Result<FlowLimits> {
    connection.query_row(
        "SELECT
            COALESCE((SELECT max_concurrent_runs FROM worker_flow_limits WHERE worker_id = ?2), 4),
            COALESCE((SELECT in_progress_limit FROM project_flow_limits WHERE project_id = ?1), 4),
            COALESCE((SELECT review_limit FROM project_flow_limits WHERE project_id = ?1), 3),
            COALESCE((SELECT approved_limit FROM project_flow_limits WHERE project_id = ?1), 2)",
        params![project_id, worker_id],
        |row| {
            Ok(FlowLimits {
                project_id: project_id.to_owned(),
                worker_id: worker_id.to_owned(),
                worker_max_concurrent_runs: row.get(0)?,
                in_progress_limit: row.get(1)?,
                review_limit: row.get(2)?,
                approved_limit: row.get(3)?,
            })
        },
    )
}

fn worker_limit_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    worker_id: &str,
) -> Result<i64> {
    transaction.query_row(
        "SELECT COALESCE((SELECT max_concurrent_runs FROM worker_flow_limits WHERE worker_id = ?1), 4)",
        [worker_id],
        |row| row.get(0),
    )
}

fn project_flow_counts(connection: &Connection, project_id: &str) -> Result<(i64, i64, i64, i64)> {
    connection.query_row(
        "SELECT
            COALESCE(SUM(CASE WHEN status = 'in_progress' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN status = 'review' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN status = 'approved' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN status = 'integrating' THEN 1 ELSE 0 END), 0)
         FROM tasks WHERE project_id = ?1",
        [project_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
}

fn requesting_agent_for_run(
    transaction: &rusqlite::Transaction<'_>,
    task_id: &str,
    run_id: Option<&str>,
    assigned_agent_id: Option<String>,
) -> Result<Option<String>> {
    let Some(run_id) = run_id else {
        return Ok(assigned_agent_id);
    };
    let run: Option<(String, String)> = transaction
        .query_row(
            "SELECT task_id, agent_id FROM runs WHERE id = ?1",
            [run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    match run {
        Some((run_task_id, agent_id)) if run_task_id == task_id => Ok(Some(agent_id)),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn append_input_request_event(
    transaction: &rusqlite::Transaction<'_>,
    run_id: Option<&str>,
    kind: &str,
    message: &str,
) -> Result<()> {
    if let Some(run_id) = run_id {
        transaction.execute(
            "INSERT INTO run_events (run_id, kind, message) VALUES (?1, ?2, ?3)",
            params![run_id, kind, message],
        )?;
    }
    Ok(())
}

fn open_input_request_context(
    transaction: &rusqlite::Transaction<'_>,
    request_id: &str,
) -> Result<Option<(String, Option<String>, String, String)>> {
    transaction
        .query_row(
            "SELECT requests.task_id, requests.requesting_run_id, tasks.project_id, tasks.status
             FROM task_input_requests AS requests
             JOIN tasks ON tasks.id = requests.task_id
             WHERE requests.id = ?1 AND requests.status = 'open'",
            [request_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
}

fn task_has_running_run(transaction: &rusqlite::Transaction<'_>, task_id: &str) -> Result<bool> {
    transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM runs WHERE task_id = ?1 AND status = 'running')",
        [task_id],
        |row| row.get(0),
    )
}

fn validate_new_project_blocker(
    connection: &Connection,
    blocker: &NewProjectBlocker,
) -> Result<()> {
    if blocker.title.trim().is_empty()
        || (!blocker.affects_all_tasks && blocker.affected_task_ids.is_empty())
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let project_exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
        [&blocker.project_id],
        |row| row.get(0),
    )?;
    if !project_exists {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let unique_tasks = blocker.affected_task_ids.iter().collect::<HashSet<_>>();
    if unique_tasks.len() != blocker.affected_task_ids.len() {
        return Err(rusqlite::Error::InvalidQuery);
    }
    for task_id in unique_tasks {
        let valid: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE id = ?1 AND project_id = ?2)",
            params![task_id, blocker.project_id],
            |row| row.get(0),
        )?;
        if !valid {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    Ok(())
}

fn validate_new_architecture_decision(
    connection: &Connection,
    decision: &NewArchitectureDecision,
) -> Result<()> {
    if decision.title.trim().is_empty()
        || decision.context.trim().is_empty()
        || decision.decision.trim().is_empty()
        || decision
            .relevant_paths
            .iter()
            .any(|path| path.trim().is_empty())
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    validate_architecture_decision_tasks(connection, decision)?;
    validate_superseded_architecture_decision(connection, decision)?;
    Ok(())
}

fn validate_new_remote_worker(connection: &Connection, worker: &NewRemoteWorker) -> Result<()> {
    if worker.id.trim().is_empty()
        || worker.name.trim().is_empty()
        || !worker.endpoint.starts_with("https://")
        || worker.token_environment_variable.trim().is_empty()
        || worker.workspace_path.trim().is_empty()
        || worker.protocol_version <= 0
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let project_exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
        [&worker.project_id],
        |row| row.get(0),
    )?;
    if project_exists {
        Ok(())
    } else {
        Err(rusqlite::Error::InvalidQuery)
    }
}

fn validate_architecture_decision_tasks(
    connection: &Connection,
    decision: &NewArchitectureDecision,
) -> Result<()> {
    let unique_tasks = decision.relevant_task_ids.iter().collect::<HashSet<_>>();
    let unique_paths = decision.relevant_paths.iter().collect::<HashSet<_>>();
    if unique_tasks.len() != decision.relevant_task_ids.len()
        || unique_paths.len() != decision.relevant_paths.len()
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    for task_id in unique_tasks {
        let valid: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE id = ?1 AND project_id = ?2)",
            params![task_id, decision.project_id],
            |row| row.get(0),
        )?;
        if !valid {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    Ok(())
}

fn validate_superseded_architecture_decision(
    connection: &Connection,
    decision: &NewArchitectureDecision,
) -> Result<()> {
    let Some(supersedes_id) = decision.supersedes_decision_id.as_deref() else {
        return Ok(());
    };
    let valid: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM architecture_decisions
            WHERE id = ?1 AND project_id = ?2 AND status = 'accepted'
         )",
        params![supersedes_id, decision.project_id],
        |row| row.get(0),
    )?;
    if valid {
        Ok(())
    } else {
        Err(rusqlite::Error::InvalidQuery)
    }
}

fn proposed_architecture_decision_context(
    transaction: &rusqlite::Transaction<'_>,
    decision_id: &str,
) -> Result<Option<(String, Option<String>)>> {
    transaction
        .query_row(
            "SELECT project_id, supersedes_decision_id FROM architecture_decisions
             WHERE id = ?1 AND status = 'proposed'",
            [decision_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
}

fn supersede_previous_architecture_decision(
    transaction: &rusqlite::Transaction<'_>,
    project_id: &str,
    supersedes_id: Option<&str>,
) -> Result<()> {
    let Some(supersedes_id) = supersedes_id else {
        return Ok(());
    };
    let changed = transaction.execute(
        "UPDATE architecture_decisions
         SET status = 'superseded', decided_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?1 AND project_id = ?2 AND status = 'accepted'",
        params![supersedes_id, project_id],
    )?;
    if changed == 1 {
        Ok(())
    } else {
        Err(rusqlite::Error::InvalidQuery)
    }
}

fn architecture_decision_applies(decision: &ArchitectureDecision, task: &Task) -> bool {
    let project_wide = decision.relevant_task_ids.is_empty() && decision.relevant_paths.is_empty();
    project_wide
        || decision
            .relevant_task_ids
            .iter()
            .any(|task_id| task_id == &task.id)
        || decision.relevant_paths.iter().any(|decision_path| {
            task.relevant_paths
                .iter()
                .any(|task_path| repository_paths_overlap(decision_path, task_path))
        })
}

fn repository_paths_overlap(left: &str, right: &str) -> bool {
    let left = normalized_repository_path(left);
    let right = normalized_repository_path(right);
    left == right
        || left
            .strip_prefix(&right)
            .is_some_and(|suffix| suffix.starts_with('/'))
        || right
            .strip_prefix(&left)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn normalized_repository_path(path: &str) -> String {
    path.trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_end_matches('/')
        .to_owned()
}

fn trimmed_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn active_task_blocker_reason(
    connection: &Connection,
    task_id: &str,
    project_id: &str,
) -> Result<Option<String>> {
    connection
        .query_row(
            "SELECT blockers.title FROM project_blockers AS blockers
             WHERE blockers.project_id = ?1 AND blockers.status = 'active'
             AND (blockers.affects_all_tasks = 1 OR EXISTS (
                SELECT 1 FROM project_blocker_tasks AS affected
                WHERE affected.blocker_id = blockers.id AND affected.task_id = ?2
             ))
             ORDER BY blockers.created_at ASC, blockers.id ASC LIMIT 1",
            params![project_id, task_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map(|title| title.map(|title| format!("Project blocker: {title}.")))
}

fn active_global_blocker_reason(
    connection: &Connection,
    project_id: &str,
) -> Result<Option<String>> {
    connection
        .query_row(
            "SELECT title FROM project_blockers
             WHERE project_id = ?1 AND status = 'active' AND affects_all_tasks = 1
             ORDER BY created_at ASC, id ASC LIMIT 1",
            [project_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map(|title| {
            title.map(|title| format!("Project blocker: {title}. Automatic starts are paused."))
        })
}

fn flow_blocked_reason_for_connection(
    connection: &Connection,
    project_id: &str,
) -> Result<Option<String>> {
    let limits = flow_limits_for_connection(connection, project_id, "local")?;
    let (in_progress, review, approved, integrating) = project_flow_counts(connection, project_id)?;
    let health: Option<String> = connection
        .query_row(
            "SELECT status FROM project_health WHERE project_id = ?1",
            [project_id],
            |row| row.get(0),
        )
        .optional()?;
    let global_blocker = active_global_blocker_reason(connection, project_id)?;
    let reason = if global_blocker.is_some() {
        global_blocker
    } else if health.as_deref() == Some("broken") {
        Some("The integration branch is broken; automatic starts are paused.".into())
    } else if review >= limits.review_limit {
        Some(format!(
            "Review is at its WIP limit ({review}/{}).",
            limits.review_limit
        ))
    } else if approved + integrating >= limits.approved_limit {
        Some(format!(
            "Approved and integrating work is at its WIP limit ({}/{}).",
            approved + integrating,
            limits.approved_limit
        ))
    } else if in_progress >= limits.in_progress_limit {
        Some(format!(
            "In Progress is at its WIP limit ({in_progress}/{}).",
            limits.in_progress_limit
        ))
    } else {
        None
    };
    Ok(reason)
}

fn flow_blocked_reason_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    project_id: &str,
) -> Result<Option<String>> {
    flow_blocked_reason_for_connection(transaction, project_id)
}

fn flow_state_for_connection(
    connection: &Connection,
    project_id: &str,
    worker_id: &str,
) -> Result<FlowState> {
    let limits = flow_limits_for_connection(connection, project_id, worker_id)?;
    let active_worker_runs = connection.query_row(
        "SELECT COUNT(*) FROM runs WHERE worker_id = ?1 AND status = 'running'",
        [worker_id],
        |row| row.get(0),
    )?;
    let queued = connection.query_row(
        "SELECT COUNT(*) FROM runs JOIN tasks ON tasks.id = runs.task_id
         WHERE tasks.project_id = ?1 AND runs.status = 'queued'",
        [project_id],
        |row| row.get(0),
    )?;
    let (in_progress, review, approved, integrating) = project_flow_counts(connection, project_id)?;
    let blocked_reason = if active_worker_runs >= limits.worker_max_concurrent_runs {
        Some(format!(
            "The local worker is at capacity ({active_worker_runs}/{}).",
            limits.worker_max_concurrent_runs
        ))
    } else {
        flow_blocked_reason_for_connection(connection, project_id)?
    };
    Ok(FlowState {
        limits,
        active_worker_runs,
        in_progress,
        review,
        approved,
        integrating,
        queued,
        blocked_reason,
    })
}

fn task_blocked_reason(
    transaction: &rusqlite::Transaction<'_>,
    task_id: &str,
    project_id: &str,
) -> Result<Option<String>> {
    match active_task_blocker_reason(transaction, task_id, project_id)? {
        Some(reason) => Ok(Some(reason)),
        None => task_readiness_requirement_reason(transaction, task_id, project_id),
    }
}

fn task_readiness_requirement_reason(
    transaction: &rusqlite::Transaction<'_>,
    task_id: &str,
    project_id: &str,
) -> Result<Option<String>> {
    let (criteria, dependency_ids): (String, String) = transaction.query_row(
        "SELECT acceptance_criteria, dependency_ids FROM tasks WHERE id = ?1 AND project_id = ?2",
        params![task_id, project_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if decode_string_list(criteria)?.is_empty() {
        return Ok(Some(
            "Add at least one acceptance criterion before starting work.".into(),
        ));
    }
    let dependencies = decode_string_list(dependency_ids)?;
    let mut waiting = Vec::new();
    for dependency_id in dependencies {
        let dependency = transaction
            .query_row(
                "SELECT title, status FROM tasks WHERE id = ?1 AND project_id = ?2",
                params![dependency_id, project_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        match dependency {
            Some((_, status)) if status == TaskStatus::Done.as_str() => {}
            Some((title, _)) => waiting.push(title),
            None => {
                return Ok(Some(format!(
                    "Dependency {dependency_id} no longer exists."
                )))
            }
        }
    }
    if waiting.is_empty() {
        Ok(None)
    } else {
        let display = waiting.into_iter().take(3).collect::<Vec<_>>().join(", ");
        Ok(Some(format!(
            "Waiting for completed dependencies: {display}."
        )))
    }
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
        AgentReviewDecision, AgentReviewStatus, AgentUpdate, ArchitectureDecisionStatus, Database,
        FlowLimitUpdate, IntegrationStatus, NewAgent, NewAgentReview, NewArchitectureDecision,
        NewEpic, NewMilestone, NewProject, NewProjectBlocker, NewRemoteWorker, NewRun, NewTask,
        NewTaskInputRequest, NewValidationCommand, ProjectDeletion, ProjectHealthStatus,
        RevertStatus, RunStatus, TaskPriority, TaskStatus, TaskUpdate, ValidationStage,
        ValidationStatus, WorkerToolCapability,
    };
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEMP_DATABASE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temporary_database_path() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after Unix epoch")
            .as_nanos();
        let sequence = TEMP_DATABASE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "orchestr-db-{}-{nonce}-{sequence}.sqlite",
            std::process::id(),
        ))
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
                    acceptance_criteria: vec!["Complete task".into()],
                    implementation_notes: None,
                    relevant_paths: Vec::new(),
                    dependency_ids: Vec::new(),
                    assigned_agent_id: None,
                    priority: TaskPriority::Normal,
                    milestone_id: None,
                    epic_id: None,
                })
                .expect("task saves");
        }

        database
            .move_task("task-1", TaskStatus::Ready, 0)
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
            TaskStatus::Ready
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
                    priority: TaskPriority::Normal,
                    milestone_id: None,
                    epic_id: None,
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
    fn task_readiness_requires_criteria_and_done_dependencies_without_cycles() {
        let database_path = temporary_database_path();
        let mut database = Database::open(&database_path).expect("database opens");
        database
            .create_project(NewProject {
                id: "project-1".into(),
                name: "Readiness project".into(),
                description: None,
                default_branch: "main".into(),
                workspace_id: "workspace-1".into(),
                worker_id: "local".into(),
                workspace_path: "C:/work/readiness-project".into(),
            })
            .expect("project saves");
        database
            .create_task(NewTask {
                id: "foundation".into(),
                project_id: "project-1".into(),
                title: "Foundation".into(),
                description: None,
                acceptance_criteria: vec!["Foundation works".into()],
                implementation_notes: None,
                relevant_paths: Vec::new(),
                dependency_ids: Vec::new(),
                assigned_agent_id: None,
                priority: TaskPriority::High,
                milestone_id: None,
                epic_id: None,
            })
            .expect("foundation saves");
        database
            .create_task(NewTask {
                id: "dependent".into(),
                project_id: "project-1".into(),
                title: "Dependent".into(),
                description: None,
                acceptance_criteria: vec!["Dependent works".into()],
                implementation_notes: None,
                relevant_paths: Vec::new(),
                dependency_ids: vec!["foundation".into()],
                assigned_agent_id: None,
                priority: TaskPriority::Normal,
                milestone_id: None,
                epic_id: None,
            })
            .expect("dependent saves");

        let blocked = database
            .move_task("dependent", TaskStatus::Ready, 0)
            .expect("move evaluates")
            .expect("task exists");
        assert_eq!(blocked.status, TaskStatus::Blocked);
        assert!(blocked
            .blocked_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("Foundation")));

        let ready = database
            .move_task("foundation", TaskStatus::Ready, 0)
            .expect("foundation moves")
            .expect("task exists");
        assert_eq!(ready.status, TaskStatus::Ready);
        database
            .connection
            .execute(
                "UPDATE tasks SET status = 'done' WHERE id = 'foundation'",
                [],
            )
            .expect("foundation completes");
        database
            .recalculate_project_task_readiness("project-1")
            .expect("readiness recalculates");
        assert_eq!(
            database
                .get_task("dependent")
                .expect("task loads")
                .expect("task exists")
                .status,
            TaskStatus::Ready
        );

        let cycle = database.update_task(
            "foundation",
            TaskUpdate {
                title: "Foundation".into(),
                description: None,
                acceptance_criteria: vec!["Foundation works".into()],
                implementation_notes: None,
                relevant_paths: Vec::new(),
                dependency_ids: vec!["dependent".into()],
                assigned_agent_id: None,
                priority: TaskPriority::Critical,
                milestone_id: None,
                epic_id: None,
            },
        );
        assert!(cycle.is_err());
        database.connection.execute("UPDATE tasks SET status = 'blocked', blocked_reason = 'Integration conflict: src/app.ts', readiness_blocked = 0 WHERE id = 'foundation'", []).expect("conflict records");
        database
            .recalculate_project_task_readiness("project-1")
            .expect("readiness recalculates");
        let conflict = database
            .get_task("foundation")
            .expect("task loads")
            .expect("task exists");
        assert_eq!(conflict.status, TaskStatus::Blocked);
        assert_eq!(
            conflict.blocked_reason.as_deref(),
            Some("Integration conflict: src/app.ts")
        );
        drop(database);
        fs::remove_file(database_path).expect("temporary database removes");
    }

    #[test]
    fn milestones_epics_and_project_progress_track_integrated_outcomes() {
        let database_path = temporary_database_path();
        let mut database = Database::open(&database_path).expect("database opens");
        database
            .create_project(NewProject {
                id: "project-1".into(),
                name: "Outcome project".into(),
                description: None,
                default_branch: "main".into(),
                workspace_id: "workspace-1".into(),
                worker_id: "local".into(),
                workspace_path: "C:/work/outcome-project".into(),
            })
            .expect("project saves");
        database
            .create_milestone(NewMilestone {
                id: "milestone-1".into(),
                project_id: "project-1".into(),
                title: "First usable release".into(),
                description: None,
                status: "active".into(),
                target_date: None,
            })
            .expect("milestone saves");
        database
            .create_milestone(NewMilestone {
                id: "milestone-2".into(),
                project_id: "project-1".into(),
                title: "Later release".into(),
                description: None,
                status: "planned".into(),
                target_date: None,
            })
            .expect("second milestone saves");
        database
            .create_epic(NewEpic {
                id: "epic-1".into(),
                project_id: "project-1".into(),
                milestone_id: Some("milestone-1".into()),
                title: "Project outcomes".into(),
                description: None,
                status: "active".into(),
            })
            .expect("epic saves");
        assert_eq!(
            database
                .update_milestone_status("milestone-1", "completed")
                .expect("milestone status updates")
                .expect("milestone exists")
                .status,
            "completed"
        );
        assert_eq!(
            database
                .update_epic_status("epic-1", "completed")
                .expect("epic status updates")
                .expect("epic exists")
                .status,
            "completed"
        );
        database
            .create_task(NewTask {
                id: "task-1".into(),
                project_id: "project-1".into(),
                title: "Ship the outcome".into(),
                description: None,
                acceptance_criteria: vec!["Available to users".into()],
                implementation_notes: None,
                relevant_paths: Vec::new(),
                dependency_ids: Vec::new(),
                assigned_agent_id: None,
                priority: TaskPriority::High,
                milestone_id: Some("milestone-1".into()),
                epic_id: Some("epic-1".into()),
            })
            .expect("linked task saves");

        let invalid_link = database.create_task(NewTask {
            id: "task-2".into(),
            project_id: "project-1".into(),
            title: "Mismatched hierarchy".into(),
            description: None,
            acceptance_criteria: vec!["Never saved".into()],
            implementation_notes: None,
            relevant_paths: Vec::new(),
            dependency_ids: Vec::new(),
            assigned_agent_id: None,
            priority: TaskPriority::Normal,
            milestone_id: Some("milestone-2".into()),
            epic_id: Some("epic-1".into()),
        });
        assert!(invalid_link.is_err());

        database
            .connection
            .execute("UPDATE tasks SET status = 'done' WHERE id = 'task-1'", [])
            .expect("task completes after integration");
        let progress = database
            .project_progress("project-1")
            .expect("project progress loads");
        assert_eq!(progress.counts.total, 1);
        assert_eq!(progress.counts.done, 1);
        assert_eq!(progress.milestones.len(), 2);
        assert_eq!(progress.milestones[0].counts.done, 1);
        assert_eq!(progress.milestones[0].epics[0].id, "epic-1");
        assert_eq!(progress.milestones[1].counts.total, 0);

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
                acceptance_criteria: vec!["Complete task".into()],
                implementation_notes: None,
                relevant_paths: Vec::new(),
                dependency_ids: Vec::new(),
                assigned_agent_id: Some("agent-1".into()),
                priority: TaskPriority::Normal,
                milestone_id: None,
                epic_id: None,
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
    fn architect_reviews_are_persisted_and_reject_self_review() {
        let database_path = temporary_database_path();
        let mut database = Database::open(&database_path).expect("database opens");
        database
            .create_project(NewProject {
                id: "project-1".into(),
                name: "Review project".into(),
                description: None,
                default_branch: "main".into(),
                workspace_id: "workspace-1".into(),
                worker_id: "local".into(),
                workspace_path: "C:/work/review-project".into(),
            })
            .expect("project saves");
        for (id, name) in [("implementer", "Implementer"), ("architect", "Architect")] {
            database
                .create_agent(NewAgent {
                    id: id.into(),
                    name: name.into(),
                    provider: "codex".into(),
                    role: "Engineer".into(),
                    model: None,
                    system_prompt: None,
                    skills: Vec::new(),
                    max_concurrent_tasks: 1,
                })
                .expect("agent saves");
        }
        database
            .create_task(NewTask {
                id: "task-1".into(),
                project_id: "project-1".into(),
                title: "Reviewable work".into(),
                description: None,
                acceptance_criteria: vec!["Works".into()],
                implementation_notes: None,
                relevant_paths: Vec::new(),
                dependency_ids: Vec::new(),
                assigned_agent_id: Some("implementer".into()),
                priority: TaskPriority::Normal,
                milestone_id: None,
                epic_id: None,
            })
            .expect("task saves");
        database
            .move_task("task-1", TaskStatus::Review, 0)
            .expect("task enters review");
        assert!(database
            .start_agent_review(NewAgentReview {
                id: "self-review".into(),
                task_id: "task-1".into(),
                agent_id: "implementer".into(),
            })
            .is_err());
        database
            .start_agent_review(NewAgentReview {
                id: "review-1".into(),
                task_id: "task-1".into(),
                agent_id: "architect".into(),
            })
            .expect("separate reviewer starts");
        database
            .append_agent_review_output("review-1", "Review evidence")
            .expect("output saves");
        let completed = database
            .finish_agent_review(
                "review-1",
                AgentReviewStatus::Completed,
                Some(AgentReviewDecision::Approve),
                Some("Acceptance criteria are covered."),
                None,
            )
            .expect("review completes")
            .expect("review exists");
        assert_eq!(completed.decision, Some(AgentReviewDecision::Approve));
        assert_eq!(completed.raw_output, "Review evidence");
        assert_eq!(
            database
                .list_agent_reviews("task-1")
                .expect("reviews load")
                .len(),
            1
        );
        assert_eq!(
            database
                .get_agent_review("review-1")
                .expect("review loads by id")
                .expect("review exists")
                .notes
                .as_deref(),
            Some("Acceptance criteria are covered.")
        );
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
                acceptance_criteria: vec!["Complete task".into()],
                implementation_notes: None,
                relevant_paths: Vec::new(),
                dependency_ids: Vec::new(),
                assigned_agent_id: Some("agent-1".into()),
                priority: TaskPriority::Normal,
                milestone_id: None,
                epic_id: None,
            })
            .expect("task saves");
        database
            .move_task("task-1", TaskStatus::Ready, 0)
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
        assert_eq!(runs[0].events[0].kind, "run.queued");
        assert!(runs[0]
            .events
            .iter()
            .any(|event| event.kind == "run.started"));
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
                acceptance_criteria: vec!["Complete task".into()],
                implementation_notes: None,
                relevant_paths: Vec::new(),
                dependency_ids: Vec::new(),
                assigned_agent_id: None,
                priority: TaskPriority::Normal,
                milestone_id: None,
                epic_id: None,
            })
            .expect("task saves");
        database
            .move_task("task-1", TaskStatus::Ready, 0)
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
                acceptance_criteria: vec!["Complete task".into()],
                implementation_notes: None,
                relevant_paths: Vec::new(),
                dependency_ids: Vec::new(),
                assigned_agent_id: None,
                priority: TaskPriority::Normal,
                milestone_id: None,
                epic_id: None,
            })
            .expect("task saves");
        database
            .move_task("task-1", TaskStatus::Ready, 0)
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

    #[test]
    fn execution_queue_respects_priority_agent_worker_and_downstream_limits() {
        let database_path = temporary_database_path();
        let mut database = Database::open(&database_path).expect("database opens");
        database
            .create_project(NewProject {
                id: "project-1".into(),
                name: "Flow-controlled project".into(),
                description: None,
                default_branch: "main".into(),
                workspace_id: "workspace-1".into(),
                worker_id: "local".into(),
                workspace_path: "C:/work/flow-project".into(),
            })
            .expect("project saves");
        for (id, limit) in [("agent-1", 1), ("agent-2", 2)] {
            database
                .create_agent(NewAgent {
                    id: id.into(),
                    name: id.into(),
                    provider: "codex".into(),
                    role: "Engineer".into(),
                    model: None,
                    system_prompt: None,
                    skills: Vec::new(),
                    max_concurrent_tasks: limit,
                })
                .expect("agent saves");
        }
        for (id, agent_id, priority) in [
            ("task-normal-a", "agent-1", TaskPriority::Normal),
            ("task-critical", "agent-1", TaskPriority::Critical),
            ("task-high", "agent-2", TaskPriority::High),
            ("task-normal-b", "agent-2", TaskPriority::Normal),
        ] {
            database
                .create_task(NewTask {
                    id: id.into(),
                    project_id: "project-1".into(),
                    title: id.into(),
                    description: None,
                    acceptance_criteria: vec!["Complete the task".into()],
                    implementation_notes: None,
                    relevant_paths: Vec::new(),
                    dependency_ids: Vec::new(),
                    assigned_agent_id: Some(agent_id.into()),
                    priority,
                    milestone_id: None,
                    epic_id: None,
                })
                .expect("task saves");
            database
                .move_task(id, TaskStatus::Ready, usize::MAX)
                .expect("task becomes ready");
            database
                .enqueue_run(NewRun {
                    id: format!("run-{id}"),
                    task_id: id.into(),
                    agent_id: agent_id.into(),
                    worker_id: "local".into(),
                })
                .expect("run queues");
        }
        database
            .update_flow_limits(
                "project-1",
                "local",
                FlowLimitUpdate {
                    worker_max_concurrent_runs: 2,
                    in_progress_limit: 4,
                    review_limit: 1,
                    approved_limit: 2,
                },
            )
            .expect("flow limits save");
        let queued_task_ids = database
            .list_queued_runs("project-1")
            .expect("execution queue loads")
            .into_iter()
            .map(|run| run.task_id)
            .collect::<Vec<_>>();
        assert_eq!(
            queued_task_ids,
            [
                "task-critical",
                "task-high",
                "task-normal-a",
                "task-normal-b"
            ]
        );
        assert!(database
            .move_task("task-normal-a", TaskStatus::Backlog, 0)
            .is_err());

        let first = database
            .claim_next_run("local")
            .expect("queue claims")
            .expect("critical task is available");
        assert_eq!(first.1.id, "task-critical");
        let second = database
            .claim_next_run("local")
            .expect("queue claims")
            .expect("second agent is available");
        assert_eq!(second.1.id, "task-high");
        assert!(database
            .claim_next_run("local")
            .expect("worker capacity checks")
            .is_none());

        database
            .finish_run(&second.0.id, RunStatus::Completed, Some(0), None)
            .expect("run completes");
        let state = database
            .flow_state("project-1", "local")
            .expect("flow state loads");
        assert_eq!(state.active_worker_runs, 1);
        assert_eq!(state.review, 1);
        assert_eq!(state.queued, 2);
        assert_eq!(
            state.blocked_reason.as_deref(),
            Some("Review is at its WIP limit (1/1).")
        );
        assert!(database
            .claim_next_run("local")
            .expect("backpressure checks")
            .is_none());
        database
            .update_flow_limits(
                "project-1",
                "local",
                FlowLimitUpdate {
                    worker_max_concurrent_runs: 2,
                    in_progress_limit: 4,
                    review_limit: 3,
                    approved_limit: 1,
                },
            )
            .expect("downstream limits update");
        database
            .connection
            .execute(
                "UPDATE tasks SET status = 'approved' WHERE id = 'task-high'",
                [],
            )
            .expect("reviewed work is approved");
        assert_eq!(
            database
                .flow_state("project-1", "local")
                .expect("approved pressure loads")
                .blocked_reason
                .as_deref(),
            Some("Approved and integrating work is at its WIP limit (1/1).")
        );
        assert!(database
            .cancel_queued_run("run-task-normal-a")
            .expect("queued run cancels"));
        assert!(database
            .move_task("task-normal-a", TaskStatus::Backlog, 0)
            .expect("cancelled queued task can move")
            .is_some());

        drop(database);
        fs::remove_file(database_path).expect("temporary database removes");
    }

    #[test]
    fn failed_runs_can_resume_reassign_escalate_and_preserve_history() {
        let database_path = temporary_database_path();
        let mut database = Database::open(&database_path).expect("database opens");
        database
            .create_project(NewProject {
                id: "project-1".into(),
                name: "Recovery".into(),
                description: None,
                default_branch: "main".into(),
                workspace_id: "workspace-1".into(),
                worker_id: "local".into(),
                workspace_path: "C:/work/recovery".into(),
            })
            .expect("project saves");
        for (id, name) in [("agent-1", "Implementer"), ("agent-2", "Recovery agent")] {
            database
                .create_agent(NewAgent {
                    id: id.into(),
                    name: name.into(),
                    provider: "codex".into(),
                    role: "Engineer".into(),
                    model: None,
                    system_prompt: None,
                    skills: Vec::new(),
                    max_concurrent_tasks: 1,
                })
                .expect("agent saves");
        }
        database
            .create_task(NewTask {
                id: "task-1".into(),
                project_id: "project-1".into(),
                title: "Recover me".into(),
                description: None,
                acceptance_criteria: vec!["Recovered".into()],
                implementation_notes: None,
                relevant_paths: Vec::new(),
                dependency_ids: Vec::new(),
                assigned_agent_id: Some("agent-1".into()),
                priority: TaskPriority::High,
                milestone_id: None,
                epic_id: None,
            })
            .expect("task saves");
        database
            .move_task("task-1", TaskStatus::Ready, 0)
            .expect("task readies");
        database
            .start_run(NewRun {
                id: "run-1".into(),
                task_id: "task-1".into(),
                agent_id: "agent-1".into(),
                worker_id: "local".into(),
            })
            .expect("run starts");
        database
            .finish_run("run-1", RunStatus::Failed, Some(1), Some("provider failed"))
            .expect("run fails");

        let (replacement, ready_task) = database
            .queue_run_recovery("run-1", "run-2", "recovery-1", "agent-2", "reassign")
            .expect("recovery queues")
            .expect("run exists");
        assert_eq!(replacement.status, RunStatus::Queued);
        assert_eq!(ready_task.status, TaskStatus::Ready);
        assert_eq!(ready_task.assigned_agent_id.as_deref(), Some("agent-2"));
        assert!(database
            .get_run("run-1")
            .expect("source loads")
            .expect("source exists")
            .events
            .iter()
            .any(|event| event.kind == "recovery.replaced"));
        let (recovery_run, _) = database
            .claim_next_run("local")
            .expect("recovery claims")
            .expect("run available");
        database
            .finish_run(
                &recovery_run.id,
                RunStatus::Failed,
                Some(2),
                Some("still failing"),
            )
            .expect("recovery fails");
        let escalated = database
            .resolve_failed_run(
                "run-2",
                "recovery-2",
                "escalate",
                Some("Human decision required"),
            )
            .expect("run escalates")
            .expect("task exists");
        assert_eq!(escalated.status, TaskStatus::Blocked);
        assert_eq!(
            escalated.blocked_reason.as_deref(),
            Some("Human decision required")
        );
        drop(database);
        fs::remove_file(database_path).expect("temporary database removes");
    }

    #[test]
    fn interrupted_integration_recovers_and_reverts_remain_traceable() {
        let database_path = temporary_database_path();
        let mut database = Database::open(&database_path).expect("database opens");
        database
            .create_project(NewProject {
                id: "project-1".into(),
                name: "Integration recovery".into(),
                description: None,
                default_branch: "main".into(),
                workspace_id: "workspace-1".into(),
                worker_id: "local".into(),
                workspace_path: "C:/work/recovery".into(),
            })
            .expect("project saves");
        database
            .create_task(NewTask {
                id: "task-1".into(),
                project_id: "project-1".into(),
                title: "Integrated task".into(),
                description: None,
                acceptance_criteria: vec!["Integrated".into()],
                implementation_notes: None,
                relevant_paths: Vec::new(),
                dependency_ids: Vec::new(),
                assigned_agent_id: None,
                priority: TaskPriority::Normal,
                milestone_id: None,
                epic_id: None,
            })
            .expect("task saves");
        database
            .connection
            .execute(
                "UPDATE tasks SET status = 'review', branch = 'task/recovery' WHERE id = 'task-1'",
                [],
            )
            .expect("task enters review");
        database
            .approve_task_review("task-1", "integration-1")
            .expect("task approves");
        database
            .claim_next_integration("project-1")
            .expect("integration claims");
        assert_eq!(
            database
                .recover_interrupted_integrations()
                .expect("lock recovers"),
            1
        );
        let interrupted = database
            .get_integration_attempt("integration-1")
            .expect("attempt loads")
            .expect("attempt exists");
        assert_eq!(interrupted.status, IntegrationStatus::Failed);
        assert_eq!(
            database
                .get_task("task-1")
                .expect("task loads")
                .expect("task exists")
                .status,
            TaskStatus::Approved
        );
        database
            .retry_integration("integration-1", "integration-2")
            .expect("retry queues");
        database
            .claim_next_integration("project-1")
            .expect("retry claims");
        database
            .complete_integration("integration-2", "abc123")
            .expect("integration completes");
        let revert = database
            .begin_revert("revert-1", "integration-2")
            .expect("revert starts")
            .expect("revert exists");
        assert_eq!(revert.original_commit, "abc123");
        database
            .finish_revert(
                "revert-1",
                RevertStatus::Reverted,
                Some("def456"),
                None,
                None,
            )
            .expect("revert completes");
        let history = database
            .list_revert_attempts("project-1")
            .expect("history loads");
        assert_eq!(history[0].status, RevertStatus::Reverted);
        assert_eq!(history[0].revert_commit.as_deref(), Some("def456"));
        assert!(database.begin_revert("revert-2", "integration-2").is_err());
        drop(database);
        fs::remove_file(database_path).expect("temporary database removes");
    }

    #[test]
    fn needs_input_and_project_blockers_pause_work_without_losing_context() {
        let database_path = temporary_database_path();
        let mut database = Database::open(&database_path).expect("database opens");
        database
            .create_project(NewProject {
                id: "project-1".into(),
                name: "Human decisions".into(),
                description: None,
                default_branch: "main".into(),
                workspace_id: "workspace-1".into(),
                worker_id: "local".into(),
                workspace_path: "C:/work/human-decisions".into(),
            })
            .expect("project saves");
        database
            .create_agent(NewAgent {
                id: "agent-1".into(),
                name: "Implementer".into(),
                provider: "codex".into(),
                role: "developer".into(),
                model: None,
                system_prompt: None,
                skills: Vec::new(),
                max_concurrent_tasks: 2,
            })
            .expect("agent saves");
        for (id, title) in [
            ("task-input", "Clarify behavior"),
            ("task-blocked", "Use SDK"),
            ("task-readiness", "Wait for service"),
        ] {
            database
                .create_task(NewTask {
                    id: id.into(),
                    project_id: "project-1".into(),
                    title: title.into(),
                    description: None,
                    acceptance_criteria: vec!["Outcome is verified".into()],
                    implementation_notes: None,
                    relevant_paths: Vec::new(),
                    dependency_ids: Vec::new(),
                    assigned_agent_id: Some("agent-1".into()),
                    priority: TaskPriority::Normal,
                    milestone_id: None,
                    epic_id: None,
                })
                .expect("task saves");
            database
                .move_task(id, TaskStatus::Ready, usize::MAX)
                .expect("task becomes ready");
        }

        let (_, running_task) = database
            .start_run(NewRun {
                id: "run-input".into(),
                task_id: "task-input".into(),
                agent_id: "agent-1".into(),
                worker_id: "local".into(),
            })
            .expect("input task starts");
        assert_eq!(running_task.status, TaskStatus::InProgress);
        let request = database
            .request_task_input(NewTaskInputRequest {
                id: "input-1".into(),
                task_id: "task-input".into(),
                requesting_run_id: Some("run-input".into()),
                question: "Which authentication flow should be authoritative?".into(),
            })
            .expect("input request saves");
        assert_eq!(request.requesting_agent_id.as_deref(), Some("agent-1"));
        assert_eq!(
            database
                .get_task("task-input")
                .expect("task loads")
                .unwrap()
                .status,
            TaskStatus::NeedsInput
        );
        assert!(database
            .answer_task_input("input-1", "Use device authorization.")
            .is_err());
        database
            .finish_run(
                "run-input",
                RunStatus::Cancelled,
                None,
                Some("Paused for input."),
            )
            .expect("run pauses");
        let (answered, resumed_task) = database
            .answer_task_input("input-1", "Use device authorization.")
            .expect("answer saves")
            .expect("request exists");
        assert_eq!(answered.status, "answered");
        assert_eq!(resumed_task.status, TaskStatus::InProgress);
        let events = database
            .get_run("run-input")
            .expect("run loads")
            .unwrap()
            .events;
        assert!(events.iter().any(|event| event.kind == "input.requested"));
        assert!(events.iter().any(|event| event.kind == "input.answered"));

        database
            .create_project_blocker(NewProjectBlocker {
                id: "blocker-readiness".into(),
                project_id: "project-1".into(),
                title: "External service unavailable".into(),
                description: None,
                affects_all_tasks: false,
                affected_task_ids: vec!["task-readiness".into()],
            })
            .expect("readiness blocker saves");
        let blocked_task = database
            .get_task("task-readiness")
            .expect("blocked task loads")
            .unwrap();
        assert_eq!(blocked_task.status, TaskStatus::Blocked);
        assert!(blocked_task
            .blocked_reason
            .unwrap()
            .contains("External service"));
        database
            .resolve_project_blocker("blocker-readiness")
            .expect("readiness blocker resolves");
        assert_eq!(
            database
                .get_task("task-readiness")
                .expect("task loads")
                .unwrap()
                .status,
            TaskStatus::Ready
        );

        database
            .enqueue_run(NewRun {
                id: "run-blocked".into(),
                task_id: "task-blocked".into(),
                agent_id: "agent-1".into(),
                worker_id: "local".into(),
            })
            .expect("second run queues");
        let blocker = database
            .create_project_blocker(NewProjectBlocker {
                id: "blocker-1".into(),
                project_id: "project-1".into(),
                title: "Required SDK unavailable".into(),
                description: Some("Wait for the platform package.".into()),
                affects_all_tasks: false,
                affected_task_ids: vec!["task-blocked".into()],
            })
            .expect("blocker saves");
        assert_eq!(blocker.affected_task_ids, ["task-blocked"]);
        assert!(database
            .claim_next_run("local")
            .expect("scheduler checks blockers")
            .is_none());
        database
            .resolve_project_blocker("blocker-1")
            .expect("blocker resolves")
            .expect("blocker exists");
        let (claimed, _) = database
            .claim_next_run("local")
            .expect("scheduler resumes")
            .expect("run is eligible");
        assert_eq!(claimed.id, "run-blocked");

        let global = database
            .create_project_blocker(NewProjectBlocker {
                id: "blocker-global".into(),
                project_id: "project-1".into(),
                title: "Product decision pending".into(),
                description: None,
                affects_all_tasks: true,
                affected_task_ids: Vec::new(),
            })
            .expect("global blocker saves");
        assert!(global.affects_all_tasks);
        assert!(database
            .flow_state("project-1", "local")
            .expect("flow loads")
            .blocked_reason
            .unwrap()
            .contains("Product decision pending"));

        drop(database);
        let reopened = Database::open(&database_path).expect("database reopens");
        let requests = reopened
            .list_task_input_requests("task-input")
            .expect("input history persists");
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].answer.as_deref(),
            Some("Use device authorization.")
        );
        let blockers = reopened
            .list_project_blockers("project-1")
            .expect("blockers persist");
        assert_eq!(blockers.len(), 3);
        assert_eq!(blockers[0].id, "blocker-global");
        drop(reopened);
        fs::remove_file(database_path).expect("temporary database removes");
    }

    #[test]
    fn architecture_decisions_are_scoped_accepted_and_superseded_explicitly() {
        let database_path = temporary_database_path();
        let mut database = Database::open(&database_path).expect("database opens");
        database
            .create_project(NewProject {
                id: "project-1".into(),
                name: "Knowledge project".into(),
                description: None,
                default_branch: "main".into(),
                workspace_id: "workspace-1".into(),
                worker_id: "local".into(),
                workspace_path: "C:/work/knowledge-project".into(),
            })
            .expect("project saves");
        for (id, path) in [
            ("task-auth", "src/auth/session.ts"),
            ("task-docs", "docs/guide.md"),
        ] {
            database
                .create_task(NewTask {
                    id: id.into(),
                    project_id: "project-1".into(),
                    title: id.into(),
                    description: None,
                    acceptance_criteria: vec!["Outcome is verified".into()],
                    implementation_notes: None,
                    relevant_paths: vec![path.into()],
                    dependency_ids: Vec::new(),
                    assigned_agent_id: None,
                    priority: TaskPriority::Normal,
                    milestone_id: None,
                    epic_id: None,
                })
                .expect("task saves");
        }

        let original = database
            .create_architecture_decision(NewArchitectureDecision {
                id: "adr-global".into(),
                project_id: "project-1".into(),
                title: "Use SQLite".into(),
                context: "Project state must remain local-first.".into(),
                decision: "Persist control-plane metadata in SQLite.".into(),
                consequences: Some("Schema changes require migrations.".into()),
                supersedes_decision_id: None,
                relevant_paths: Vec::new(),
                relevant_task_ids: Vec::new(),
            })
            .expect("proposal saves");
        assert_eq!(original.decision_number, 1);
        assert_eq!(original.status, ArchitectureDecisionStatus::Proposed);
        assert!(database
            .list_relevant_architecture_decisions("task-auth")
            .expect("context loads")
            .is_empty());
        database
            .decide_architecture_decision("adr-global", ArchitectureDecisionStatus::Accepted)
            .expect("proposal accepts")
            .expect("proposal exists");

        let path_decision = database
            .create_architecture_decision(NewArchitectureDecision {
                id: "adr-auth".into(),
                project_id: "project-1".into(),
                title: "Centralize sessions".into(),
                context: "Authentication has multiple consumers.".into(),
                decision: "Keep session behavior under src/auth.".into(),
                consequences: None,
                supersedes_decision_id: None,
                relevant_paths: vec!["src/auth".into()],
                relevant_task_ids: Vec::new(),
            })
            .expect("path proposal saves");
        assert_eq!(path_decision.decision_number, 2);
        database
            .decide_architecture_decision("adr-auth", ArchitectureDecisionStatus::Accepted)
            .expect("path proposal accepts");
        let auth_context = database
            .list_relevant_architecture_decisions("task-auth")
            .expect("auth context loads");
        assert_eq!(auth_context.len(), 2);
        assert_eq!(
            database
                .list_relevant_architecture_decisions("task-docs")
                .expect("docs context loads")
                .len(),
            1
        );

        database
            .create_architecture_decision(NewArchitectureDecision {
                id: "adr-replacement".into(),
                project_id: "project-1".into(),
                title: "Use encrypted SQLite".into(),
                context: "Stored project metadata may be sensitive.".into(),
                decision: "Use the encrypted SQLite adapter.".into(),
                consequences: None,
                supersedes_decision_id: Some("adr-global".into()),
                relevant_paths: Vec::new(),
                relevant_task_ids: Vec::new(),
            })
            .expect("replacement proposal saves");
        database
            .decide_architecture_decision("adr-replacement", ArchitectureDecisionStatus::Accepted)
            .expect("replacement accepts");
        let history = database
            .list_architecture_decisions("project-1")
            .expect("history loads");
        assert_eq!(history[0].status, ArchitectureDecisionStatus::Accepted);
        assert_eq!(history[2].status, ArchitectureDecisionStatus::Superseded);

        drop(database);
        let mut reopened = Database::open(&database_path).expect("database reopens");
        let persisted = reopened
            .list_relevant_architecture_decisions("task-docs")
            .expect("persisted context loads");
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].id, "adr-replacement");
        assert_eq!(
            reopened
                .delete_project("project-1")
                .expect("project with ADR history deletes"),
            ProjectDeletion::Deleted
        );
        drop(reopened);
        fs::remove_file(database_path).expect("temporary database removes");
    }

    #[test]
    fn remote_worker_registration_persists_capabilities_and_project_workspace() {
        let database_path = temporary_database_path();
        let mut database = Database::open(&database_path).expect("database opens");
        database
            .create_project(NewProject {
                id: "project-1".into(),
                name: "Distributed project".into(),
                description: None,
                default_branch: "main".into(),
                workspace_id: "workspace-local".into(),
                worker_id: "local".into(),
                workspace_path: "C:/work/distributed-project".into(),
            })
            .expect("project saves");
        let worker = database
            .register_remote_worker(NewRemoteWorker {
                id: "worker-linux".into(),
                name: "Linux Builder".into(),
                endpoint: "https://worker.example:9443".into(),
                token_environment_variable: "ORCHESTR_LINUX_WORKER_TOKEN".into(),
                ca_certificate_pem: Some("test-ca".into()),
                os: "linux".into(),
                architecture: "x64".into(),
                protocol_version: 1,
                tools: vec![WorkerToolCapability {
                    name: "git".into(),
                    installed: true,
                    version: Some("git version 2.50".into()),
                }],
                project_id: "project-1".into(),
                workspace_path: "/srv/orchestr/project".into(),
            })
            .expect("worker registers");
        assert_eq!(worker.status, "online");
        assert_eq!(worker.tools[0].name, "git");
        assert_eq!(worker.workspaces[0].project_id, "project-1");
        assert_eq!(
            database
                .remote_worker_for_project("project-1")
                .expect("routing loads")
                .expect("worker is assigned")
                .id,
            "worker-linux"
        );
        database
            .mark_remote_worker_offline("worker-linux")
            .expect("worker goes offline");
        assert_eq!(
            database
                .get_remote_worker("worker-linux")
                .expect("worker loads")
                .unwrap()
                .status,
            "offline"
        );
        drop(database);

        let mut reopened = Database::open(&database_path).expect("database reopens");
        assert_eq!(
            reopened.list_remote_workers().expect("workers load").len(),
            1
        );
        assert_eq!(
            reopened
                .delete_project("project-1")
                .expect("project with a remote mapping deletes"),
            ProjectDeletion::Deleted
        );
        assert!(reopened
            .get_remote_worker("worker-linux")
            .expect("worker remains registered")
            .expect("worker remains")
            .workspaces
            .is_empty());
        assert!(reopened
            .delete_remote_worker("worker-linux")
            .expect("worker deletes"));
        assert!(reopened
            .remote_worker_for_project("project-1")
            .expect("routing reloads")
            .is_none());
        drop(reopened);
        fs::remove_file(database_path).expect("temporary database removes");
    }
}
