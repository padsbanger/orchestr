//! Provider integrations for Orchestr.
//!
//! Providers describe installed runtimes and create structured worker commands.
//! They never read, return, or persist authentication credentials.

use std::fmt;

use orchestr_worker::{LocalWorker, ProcessRequest};
use serde::Serialize;

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
}

pub struct CodexProvider;

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
        }
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
        classify_authentication, AgentProvider, AuthenticationStatus, CodexProvider, ProviderAction,
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
}
