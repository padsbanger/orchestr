//! Provider integrations for Orchestr.
//!
//! Providers describe installed runtimes and create structured worker commands.
//! They never read, return, or persist authentication credentials.

use std::{fmt, path::PathBuf};

use orchestr_worker::{LocalWorker, ProcessRequest};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub authentication: AuthenticationStatus,
    pub readiness: ProviderReadiness,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticationStatus {
    Authenticated,
    Unauthenticated,
    Unavailable,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderReadiness {
    Ready,
    NeedsAuthentication,
    Unavailable,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub enum ProviderAction {
    Login,
    Logout,
    CheckConnection,
}

#[derive(Debug)]
pub struct ProviderError(String);

impl fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ProviderError {}

pub type Result<T> = std::result::Result<T, ProviderError>;

pub trait AgentProvider {
    fn id(&self) -> &'static str;
    fn inspect(&self) -> Result<ProviderStatus>;
    fn action_request(&self, action: ProviderAction) -> ProcessRequest;
    fn execution_request(&self, input: AgentRunInput) -> Result<ProcessRequest>;
}

#[derive(Debug, Clone)]
pub struct AgentRunInput {
    pub model: Option<String>,
    pub prompt: String,
    pub working_directory: PathBuf,
    /// Provider-specific runtime metadata directories needed alongside the
    /// isolated task worktree. The desktop host derives these from Git rather
    /// than accepting them from task text or the UI.
    pub additional_writable_directories: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionEvent {
    pub kind: String,
    pub message: String,
    pub command: Option<String>,
    pub exit_code: Option<i32>,
}

pub struct CodexProvider;

impl CodexProvider {
    /// Converts Codex's `exec --json` protocol records into concise terminal
    /// entries. The raw JSON protocol remains a provider concern, not UI data.
    pub fn format_execution_output(line: &str) -> String {
        Self::execution_event(line).message
    }

    pub fn execution_event(line: &str) -> ExecutionEvent {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            return ExecutionEvent {
                kind: "command.output".into(),
                message: line.to_owned(),
                command: None,
                exit_code: None,
            };
        };
        match event.get("type").and_then(Value::as_str) {
            Some("thread.started") => {
                event_record("agent.session_started", "Codex session started.")
            }
            Some("turn.started") => event_record("agent.started", "Codex started working."),
            Some("turn.completed") => event_record("agent.completed", "Codex finished working."),
            Some("error") => ExecutionEvent {
                kind: "provider.error".into(),
                message: event
                    .get("message")
                    .and_then(Value::as_str)
                    .map(|message| format!("Error: {message}"))
                    .unwrap_or_else(|| "Codex reported an error.".into()),
                command: None,
                exit_code: None,
            },
            Some("item.completed") | Some("item.started") => {
                event.get("item").map(codex_item_event).unwrap_or_else(|| {
                    event_record("agent.updated", "Codex updated its execution state.")
                })
            }
            Some(event_type) => {
                event_record("provider.event", &format!("Codex event: {event_type}"))
            }
            None => event_record("command.output", line),
        }
    }
}

fn event_record(kind: &str, message: &str) -> ExecutionEvent {
    ExecutionEvent {
        kind: kind.into(),
        message: message.into(),
        command: None,
        exit_code: None,
    }
}

fn codex_item_event(item: &Value) -> ExecutionEvent {
    match item.get("type").and_then(Value::as_str) {
        Some("command_execution") => {
            let command = item
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or("command");
            let status = item
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("completed");
            let exit_code = item
                .get("exit_code")
                .and_then(Value::as_i64)
                .map(|code| code as i32);
            let is_validation = is_validation_command(command);
            let kind = match (is_validation, status) {
                (true, "in_progress") => "validation.started",
                (true, _) => "validation.completed",
                (false, "in_progress") => "command.started",
                (false, _) => "command.completed",
            };
            let message = item
                .get("aggregated_output")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{status}: {command}"));
            ExecutionEvent {
                kind: kind.into(),
                message,
                command: Some(command.to_owned()),
                exit_code,
            }
        }
        Some("agent_message") => item
            .get("text")
            .and_then(Value::as_str)
            .map(|message| event_record("agent.message", message))
            .unwrap_or_else(|| event_record("agent.message", "Codex sent a message.")),
        Some("reasoning") => event_record("agent.reasoning", "Codex is reasoning..."),
        Some("file_change") => event_record("file.modified", "Codex updated files."),
        Some(item_type) => event_record("provider.event", &format!("Codex event: {item_type}")),
        None => event_record("agent.updated", "Codex updated its execution state."),
    }
}

fn is_validation_command(command: &str) -> bool {
    let command = command.to_ascii_lowercase();
    [
        " test",
        " lint",
        " typecheck",
        " check",
        " build",
        " cargo test",
        " cargo check",
    ]
    .iter()
    .any(|needle| command.contains(needle))
}

impl AgentProvider for CodexProvider {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn inspect(&self) -> Result<ProviderStatus> {
        let installation = LocalWorker::inspect_tool("codex", &["--version"]);
        if !installation.installed {
            return Ok(ProviderStatus {
                id: self.id().into(),
                name: "Codex".into(),
                installed: false,
                version: None,
                authentication: AuthenticationStatus::Unavailable,
                readiness: ProviderReadiness::Unavailable,
                detail: "Codex CLI is not installed on this worker.".into(),
            });
        }

        let (succeeded, output) = run_codex_status()?;
        let authentication = classify_authentication(succeeded, &output);
        let (readiness, detail) = match authentication {
            AuthenticationStatus::Authenticated => (
                ProviderReadiness::Ready,
                "Codex CLI is installed and authenticated on this worker.".into(),
            ),
            AuthenticationStatus::Unauthenticated => (
                ProviderReadiness::NeedsAuthentication,
                "Codex CLI is installed but needs sign-in on this worker.".into(),
            ),
            AuthenticationStatus::Unknown => (
                ProviderReadiness::Unknown,
                "Codex CLI is installed, but authentication status could not be confirmed.".into(),
            ),
            AuthenticationStatus::Unavailable => (
                ProviderReadiness::Unavailable,
                "Codex CLI is unavailable on this worker.".into(),
            ),
        };

        Ok(ProviderStatus {
            id: self.id().into(),
            name: "Codex".into(),
            installed: true,
            version: installation.version,
            authentication,
            readiness,
            detail,
        })
    }

    fn action_request(&self, action: ProviderAction) -> ProcessRequest {
        let arguments = match action {
            ProviderAction::Login => vec!["login".into(), "--device-auth".into()],
            ProviderAction::Logout => vec!["logout".into()],
            ProviderAction::CheckConnection => vec!["login".into(), "status".into()],
        };
        ProcessRequest {
            program: "codex".into(),
            arguments,
            working_directory: None,
            standard_input: None,
        }
    }

    fn execution_request(&self, input: AgentRunInput) -> Result<ProcessRequest> {
        if input.prompt.trim().is_empty() {
            return Err(ProviderError(
                "A task prompt is required to run Codex.".into(),
            ));
        }
        if !input.working_directory.is_dir() {
            return Err(ProviderError(
                "The selected workspace no longer exists on this worker.".into(),
            ));
        }
        for directory in &input.additional_writable_directories {
            if !directory.is_dir() {
                return Err(ProviderError(format!(
                    "A required writable runtime directory no longer exists: {}",
                    directory.display()
                )));
            }
        }

        let mut arguments = vec![
            "exec".into(),
            "--json".into(),
            "--color".into(),
            "never".into(),
            "--sandbox".into(),
            "workspace-write".into(),
        ];
        for directory in input.additional_writable_directories {
            arguments.push("--add-dir".into());
            arguments.push(directory.to_string_lossy().into_owned());
        }
        if let Some(model) = input.model.filter(|model| !model.trim().is_empty()) {
            if !model.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
            }) {
                return Err(ProviderError(
                    "The selected Codex model identifier is invalid.".into(),
                ));
            }
            arguments.push("--model".into());
            arguments.push(model);
        }
        arguments.push("-".into());
        Ok(ProcessRequest {
            program: "codex".into(),
            arguments,
            working_directory: Some(input.working_directory),
            standard_input: Some(input.prompt),
        })
    }
}

fn run_codex_status() -> Result<(bool, String)> {
    let provider = CodexProvider;
    let run = LocalWorker::start(provider.action_request(ProviderAction::CheckConnection))
        .map_err(|error| ProviderError(format!("Unable to inspect Codex: {error}")))?;
    let output = run
        .output
        .into_iter()
        .map(|event| event.text)
        .collect::<Vec<_>>()
        .join("\n");
    let status = run
        .handle
        .wait()
        .map_err(|error| ProviderError(format!("Unable to inspect Codex: {error}")))?;
    Ok((status.success(), output))
}

fn classify_authentication(succeeded: bool, output: &str) -> AuthenticationStatus {
    let output = output.to_ascii_lowercase();
    if output.contains("not logged") || output.contains("not authenticated") {
        AuthenticationStatus::Unauthenticated
    } else if succeeded && output.contains("logged in") {
        AuthenticationStatus::Authenticated
    } else {
        AuthenticationStatus::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_authentication, AgentProvider, AgentRunInput, AuthenticationStatus, CodexProvider,
        ProviderAction,
    };

    #[test]
    fn codex_actions_use_structured_cli_arguments() {
        let provider = CodexProvider;
        assert_eq!(
            provider.action_request(ProviderAction::Login).arguments,
            ["login", "--device-auth"]
        );
        assert_eq!(
            provider.action_request(ProviderAction::Logout).arguments,
            ["logout"]
        );
    }

    #[test]
    fn login_status_classification_prioritizes_unauthenticated_output() {
        assert!(matches!(
            classify_authentication(false, "Not logged in"),
            AuthenticationStatus::Unauthenticated
        ));
        assert!(matches!(
            classify_authentication(true, "Logged in using ChatGPT"),
            AuthenticationStatus::Authenticated
        ));
    }

    #[test]
    fn codex_execution_uses_structured_workspace_write_arguments() {
        let working_directory = std::env::current_dir().expect("current directory");
        let additional_directories = vec![working_directory.clone(), std::env::temp_dir()];
        let request = CodexProvider
            .execution_request(AgentRunInput {
                model: Some("gpt-5.6-terra".into()),
                prompt: "Implement the task.".into(),
                working_directory,
                additional_writable_directories: additional_directories.clone(),
            })
            .expect("request builds");
        assert_eq!(request.program, "codex");
        assert_eq!(request.arguments[0], "exec");
        assert!(request
            .arguments
            .windows(2)
            .any(|pair| pair == ["--model", "gpt-5.6-terra"]));
        assert!(request
            .arguments
            .windows(2)
            .any(|pair| pair == ["--sandbox", "workspace-write"]));
        let writable_arguments = request
            .arguments
            .windows(2)
            .filter(|pair| pair[0] == "--add-dir")
            .map(|pair| pair[1].as_str())
            .collect::<Vec<_>>();
        assert_eq!(writable_arguments.len(), 2);
        for directory in additional_directories {
            assert!(writable_arguments.contains(&directory.to_string_lossy().as_ref()));
        }
        assert_eq!(request.arguments.last().map(String::as_str), Some("-"));
        assert_eq!(
            request.standard_input.as_deref(),
            Some("Implement the task.")
        );
    }

    #[test]
    fn formats_codex_json_events_for_human_readable_logs() {
        let command = CodexProvider::execution_event(
            r#"{"type":"item.completed","item":{"type":"command_execution","command":"git status","aggregated_output":"On branch main","exit_code":0,"status":"completed"}}"#,
        );
        assert_eq!(command.kind, "command.completed");
        assert_eq!(command.command.as_deref(), Some("git status"));
        assert_eq!(command.message, "On branch main");
        assert_eq!(command.exit_code, Some(0));
        assert_eq!(
            CodexProvider::format_execution_output(
                r#"{"type":"item.completed","item":{"type":"agent_message","text":"Implemented the task."}}"#,
            ),
            "Implemented the task."
        );
    }
}
