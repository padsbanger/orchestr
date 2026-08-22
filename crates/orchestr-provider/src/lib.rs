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
}

pub struct CodexProvider;

impl CodexProvider {
    /// Converts Codex's `exec --json` protocol records into concise terminal
    /// entries. The raw JSON protocol remains a provider concern, not UI data.
    pub fn format_execution_output(line: &str) -> String {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            return line.to_owned();
        };
        match event.get("type").and_then(Value::as_str) {
            Some("thread.started") => "Codex session started.".into(),
            Some("turn.started") => "Codex started working.".into(),
            Some("turn.completed") => "Codex finished working.".into(),
            Some("error") => event
                .get("message")
                .and_then(Value::as_str)
                .map(|message| format!("Error: {message}"))
                .unwrap_or_else(|| "Codex reported an error.".into()),
            Some("item.completed") | Some("item.started") => event
                .get("item")
                .map(format_codex_item)
                .unwrap_or_else(|| "Codex updated its execution state.".into()),
            Some(event_type) => format!("Codex event: {event_type}"),
            None => line.to_owned(),
        }
    }
}

fn format_codex_item(item: &Value) -> String {
    match item.get("type").and_then(Value::as_str) {
        Some("command_execution") => {
            let command = item
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or("command");
            let mut output = format!("$ {command}");
            if let Some(status) = item.get("status").and_then(Value::as_str) {
                output.push_str(&format!("\n[{status}]"));
            }
            if let Some(text) = item.get("aggregated_output").and_then(Value::as_str) {
                let text = text.trim();
                if !text.is_empty() {
                    output.push_str(&format!("\n{text}"));
                }
            }
            if let Some(exit_code) = item.get("exit_code").and_then(Value::as_i64) {
                output.push_str(&format!("\nexit {exit_code}"));
            }
            output
        }
        Some("agent_message") => item
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| "Codex sent a message.".into()),
        Some("reasoning") => "Codex is reasoning...".into(),
        Some("file_change") => "Codex updated files.".into(),
        Some(item_type) => format!("Codex event: {item_type}"),
        None => "Codex updated its execution state.".into(),
    }
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

        let mut arguments = vec![
            "exec".into(),
            "--json".into(),
            "--color".into(),
            "never".into(),
            "--sandbox".into(),
            "workspace-write".into(),
        ];
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
        let request = CodexProvider
            .execution_request(AgentRunInput {
                model: Some("gpt-5.6-terra".into()),
                prompt: "Implement the task.".into(),
                working_directory: std::env::current_dir().expect("current directory"),
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
        assert_eq!(request.arguments.last().map(String::as_str), Some("-"));
        assert_eq!(
            request.standard_input.as_deref(),
            Some("Implement the task.")
        );
    }

    #[test]
    fn formats_codex_json_events_for_human_readable_logs() {
        let command = CodexProvider::format_execution_output(
            r#"{"type":"item.completed","item":{"type":"command_execution","command":"git status","aggregated_output":"On branch main","exit_code":0,"status":"completed"}}"#,
        );
        assert_eq!(command, "$ git status\n[completed]\nOn branch main\nexit 0");
        assert_eq!(
            CodexProvider::format_execution_output(
                r#"{"type":"item.completed","item":{"type":"agent_message","text":"Implemented the task."}}"#,
            ),
            "Implemented the task."
        );
    }
}
