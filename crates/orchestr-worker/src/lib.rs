//! Cross-platform local process execution for Orchestr workers.
//!
//! This crate accepts a program and argument array; it never invokes a shell.
//! The desktop host owns run identity and persistence, while this runtime owns
//! local capability detection, process lifecycle, and output streaming.

use std::{
    fmt,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

use serde::{Deserialize, Serialize};

const KNOWN_TOOLS: &[(&str, &[&str])] = &[
    ("git", &["--version"]),
    ("node", &["--version"]),
    ("npm", &["--version"]),
    ("pnpm", &["--version"]),
    ("bun", &["--version"]),
    ("docker", &["--version"]),
    ("python", &["--version"]),
    ("cargo", &["--version"]),
    ("java", &["--version"]),
    ("gradle", &["--version"]),
    ("codex", &["--version"]),
];

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerProfile {
    pub id: String,
    pub name: String,
    pub os: String,
    pub architecture: String,
    pub status: String,
    pub tools: Vec<ToolCapability>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCapability {
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessRequest {
    pub program: String,
    pub arguments: Vec<String>,
    pub working_directory: Option<PathBuf>,
    pub standard_input: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessOutput {
    pub stream: OutputStream,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessExit {
    pub success: bool,
    pub code: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteJobRequest {
    pub id: String,
    pub process: ProcessRequest,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteJobEvent {
    pub sequence: u64,
    pub stream: OutputStream,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteJobSnapshot {
    pub id: String,
    pub status: String,
    pub events: Vec<RemoteJobEvent>,
    pub next_cursor: u64,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWorkerHandshake {
    pub protocol_version: u32,
    pub profile: WorkerProfile,
    #[serde(default)]
    pub providers: Vec<ProviderCapability>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapability {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub authentication: String,
    pub readiness: String,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct RemoteWorkerConfig {
    pub endpoint: String,
    pub token: String,
    pub ca_certificate_pem: Option<String>,
}

#[derive(Debug)]
pub struct WorkerError(String);

impl fmt::Display for WorkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for WorkerError {}

pub type Result<T> = std::result::Result<T, WorkerError>;

pub struct LocalWorker;

impl LocalWorker {
    pub fn profile() -> WorkerProfile {
        WorkerProfile {
            id: "local".into(),
            name: "Local Machine".into(),
            os: platform_os().into(),
            architecture: platform_architecture().into(),
            status: "online".into(),
            tools: KNOWN_TOOLS
                .iter()
                .map(|(name, arguments)| detect_tool(name, arguments))
                .collect(),
        }
    }

    pub fn start(request: ProcessRequest) -> Result<WorkerRun> {
        validate_request(&request)?;
        let executable = resolve_program(&request.program);
        let mut command = Command::new(&executable);
        command
            .args(&request.arguments)
            .stdin(if request.standard_input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(working_directory) = &request.working_directory {
            command.current_dir(working_directory);
        }

        let mut child = command.spawn().map_err(|error| {
            WorkerError(format!("Unable to start {}: {error}", executable.display()))
        })?;
        if let Some(input) = request.standard_input.clone() {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| WorkerError("Worker standard input was not captured.".into()))?;
            thread::spawn(move || {
                let _ = stdin.write_all(input.as_bytes());
            });
        }
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| WorkerError("Worker stdout was not captured.".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| WorkerError("Worker stderr was not captured.".into()))?;
        let (sender, receiver) = mpsc::channel();
        spawn_output_reader(stdout, OutputStream::Stdout, sender.clone());
        spawn_output_reader(stderr, OutputStream::Stderr, sender);

        Ok(WorkerRun {
            handle: WorkerHandle {
                inner: WorkerHandleKind::Local(Arc::new(Mutex::new(child))),
            },
            output: receiver,
        })
    }

    pub fn inspect_tool(name: &str, arguments: &[&str]) -> ToolCapability {
        detect_tool(name, arguments)
    }
}

pub struct WorkerRun {
    pub handle: WorkerHandle,
    pub output: Receiver<ProcessOutput>,
}

#[derive(Clone)]
pub struct WorkerHandle {
    inner: WorkerHandleKind,
}

#[derive(Clone)]
enum WorkerHandleKind {
    Local(Arc<Mutex<Child>>),
    Remote(RemoteWorkerHandle),
}

#[derive(Clone)]
struct RemoteWorkerHandle {
    client: reqwest::blocking::Client,
    endpoint: String,
    token: String,
    job_id: String,
    completion: Arc<Mutex<Receiver<Result<ProcessExit>>>>,
}

impl WorkerHandle {
    pub fn wait(&self) -> Result<ProcessExit> {
        self.inner.wait()
    }

    pub fn cancel(&self) -> Result<()> {
        self.inner.cancel()
    }
}

impl WorkerHandleKind {
    fn wait(&self) -> Result<ProcessExit> {
        match self {
            Self::Local(child) => wait_for_local_process(child),
            Self::Remote(remote) => remote
                .completion
                .lock()
                .map_err(|_| WorkerError("The remote job state is unavailable.".into()))?
                .recv()
                .map_err(|_| WorkerError("The remote job monitor stopped unexpectedly.".into()))?,
        }
    }

    fn cancel(&self) -> Result<()> {
        match self {
            Self::Local(child) => cancel_local_process(child),
            Self::Remote(remote) => cancel_remote_job(remote),
        }
    }
}

fn wait_for_local_process(child: &Arc<Mutex<Child>>) -> Result<ProcessExit> {
    child
        .lock()
        .map_err(|_| WorkerError("The worker process lock is unavailable.".into()))?
        .wait()
        .map(|status| ProcessExit {
            success: status.success(),
            code: status.code(),
        })
        .map_err(|error| WorkerError(format!("Unable to wait for worker process: {error}")))
}

fn cancel_local_process(child: &Arc<Mutex<Child>>) -> Result<()> {
    let mut child_guard = child
        .lock()
        .map_err(|_| WorkerError("The worker process lock is unavailable.".into()))?;
    match child_guard.try_wait() {
        Ok(Some(_)) => Ok(()),
        Ok(None) => {
            #[cfg(windows)]
            {
                // npm-installed CLIs such as Codex are launched through a .cmd wrapper.
                // Killing only that wrapper leaves its Node child alive (and its output pipes
                // open), so the run never reaches its terminal state. `taskkill /T` stops the
                // complete process tree rooted at the worker process.
                let process_id = child_guard.id();
                drop(child_guard);
                terminate_process_tree(process_id, child)
            }

            #[cfg(not(windows))]
            {
                child_guard.kill().map_err(|error| {
                    WorkerError(format!("Unable to cancel worker process: {error}"))
                })
            }
        }
        Err(error) => Err(WorkerError(format!(
            "Unable to inspect worker process status: {error}"
        ))),
    }
}

pub struct RemoteWorkerClient {
    client: reqwest::blocking::Client,
    endpoint: String,
    token: String,
}

impl RemoteWorkerClient {
    pub fn connect(config: RemoteWorkerConfig) -> Result<Self> {
        let endpoint = validate_remote_endpoint(&config.endpoint)?;
        let mut builder = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(8))
            .timeout(std::time::Duration::from_secs(35));
        if let Some(pem) = config.ca_certificate_pem {
            let certificate = reqwest::Certificate::from_pem(pem.as_bytes())
                .map_err(|error| WorkerError(format!("Invalid worker CA certificate: {error}")))?;
            builder = builder.add_root_certificate(certificate);
        }
        let client = builder.build().map_err(|error| {
            WorkerError(format!("Unable to configure remote worker TLS: {error}"))
        })?;
        Ok(Self {
            client,
            endpoint,
            token: config.token,
        })
    }

    pub fn handshake(&self) -> Result<RemoteWorkerHandshake> {
        self.client
            .get(format!("{}/v1/worker", self.endpoint))
            .bearer_auth(&self.token)
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .map_err(remote_request_error)?
            .json()
            .map_err(|error| WorkerError(format!("Invalid worker handshake: {error}")))
    }

    pub fn start(&self, request: RemoteJobRequest) -> Result<WorkerRun> {
        let job_id = request.id.clone();
        self.client
            .post(format!("{}/v1/jobs", self.endpoint))
            .bearer_auth(&self.token)
            .json(&request)
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .map_err(remote_request_error)?;
        Ok(self.monitor_job(job_id, 0))
    }

    pub fn reconnect(&self, job_id: &str, after: u64) -> WorkerRun {
        self.monitor_job(job_id.to_owned(), after)
    }

    fn monitor_job(&self, job_id: String, after: u64) -> WorkerRun {
        let (output_sender, output) = mpsc::channel();
        let (completion_sender, completion) = mpsc::channel();
        let client = self.client.clone();
        let endpoint = self.endpoint.clone();
        let token = self.token.clone();
        let monitored_job_id = job_id.clone();
        thread::spawn(move || {
            let result = poll_remote_job(
                &client,
                &endpoint,
                &token,
                &monitored_job_id,
                after,
                output_sender,
            );
            let _ = completion_sender.send(result);
        });
        WorkerRun {
            handle: WorkerHandle {
                inner: WorkerHandleKind::Remote(RemoteWorkerHandle {
                    client: self.client.clone(),
                    endpoint: self.endpoint.clone(),
                    token: self.token.clone(),
                    job_id,
                    completion: Arc::new(Mutex::new(completion)),
                }),
            },
            output,
        }
    }
}

fn validate_remote_endpoint(endpoint: &str) -> Result<String> {
    let endpoint = endpoint.trim().trim_end_matches('/');
    if endpoint.starts_with("https://") && endpoint.len() > "https://".len() {
        Ok(endpoint.to_owned())
    } else {
        Err(WorkerError(
            "Remote worker endpoints must use HTTPS.".into(),
        ))
    }
}

fn poll_remote_job(
    client: &reqwest::blocking::Client,
    endpoint: &str,
    token: &str,
    job_id: &str,
    after: u64,
    output: Sender<ProcessOutput>,
) -> Result<ProcessExit> {
    let mut cursor = after;
    loop {
        let snapshot = fetch_remote_job_with_retry(client, endpoint, token, job_id, cursor)?;
        cursor = snapshot.next_cursor;
        forward_remote_events(&output, &snapshot.events);
        if snapshot.status != "running" {
            return remote_job_exit(snapshot);
        }
        thread::sleep(std::time::Duration::from_millis(250));
    }
}

fn fetch_remote_job_with_retry(
    client: &reqwest::blocking::Client,
    endpoint: &str,
    token: &str,
    job_id: &str,
    cursor: u64,
) -> Result<RemoteJobSnapshot> {
    retry_remote_job(
        client,
        endpoint,
        token,
        job_id,
        cursor,
        std::time::Instant::now(),
    )
}

fn retry_remote_job(
    client: &reqwest::blocking::Client,
    endpoint: &str,
    token: &str,
    job_id: &str,
    cursor: u64,
    disconnected_at: std::time::Instant,
) -> Result<RemoteJobSnapshot> {
    fetch_remote_job(client, endpoint, token, job_id, cursor).or_else(|error| {
        ensure_remote_retry_window(disconnected_at, error)?;
        thread::sleep(std::time::Duration::from_millis(500));
        retry_remote_job(client, endpoint, token, job_id, cursor, disconnected_at)
    })
}

fn ensure_remote_retry_window(
    disconnected_at: std::time::Instant,
    error: WorkerError,
) -> Result<()> {
    if disconnected_at.elapsed() >= std::time::Duration::from_secs(60) {
        Err(error)
    } else {
        Ok(())
    }
}

fn forward_remote_events(output: &Sender<ProcessOutput>, events: &[RemoteJobEvent]) {
    for event in events {
        let _ = output.send(ProcessOutput {
            stream: event.stream.clone(),
            text: event.text.clone(),
        });
    }
}

fn fetch_remote_job(
    client: &reqwest::blocking::Client,
    endpoint: &str,
    token: &str,
    job_id: &str,
    cursor: u64,
) -> Result<RemoteJobSnapshot> {
    client
        .get(format!("{endpoint}/v1/jobs/{job_id}"))
        .bearer_auth(token)
        .query(&[("after", cursor)])
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(remote_request_error)?
        .json()
        .map_err(|error| WorkerError(format!("Invalid remote worker event response: {error}")))
}

fn remote_job_exit(snapshot: RemoteJobSnapshot) -> Result<ProcessExit> {
    match snapshot.status.as_str() {
        "completed" | "cancelled" => Ok(ProcessExit {
            success: snapshot.status == "completed" && snapshot.exit_code == Some(0),
            code: snapshot.exit_code,
        }),
        _ => Err(WorkerError(
            snapshot
                .error
                .unwrap_or_else(|| "The remote worker job failed.".into()),
        )),
    }
}

fn cancel_remote_job(remote: &RemoteWorkerHandle) -> Result<()> {
    remote
        .client
        .delete(format!("{}/v1/jobs/{}", remote.endpoint, remote.job_id))
        .bearer_auth(&remote.token)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map(|_| ())
        .map_err(remote_request_error)
}

fn remote_request_error(error: reqwest::Error) -> WorkerError {
    WorkerError(format!("Remote worker request failed: {error}"))
}

#[cfg(windows)]
fn terminate_process_tree(process_id: u32, child: &Arc<Mutex<Child>>) -> Result<()> {
    let output = Command::new("taskkill")
        .args(["/PID", &process_id.to_string(), "/T", "/F"])
        .output()
        .map_err(|error| WorkerError(format!("Unable to cancel worker process tree: {error}")))?;

    if output.status.success() {
        return Ok(());
    }

    // A process can finish in the small window between try_wait and taskkill.
    // Treat that race as a successful cancellation rather than showing a false error.
    let mut child = child
        .lock()
        .map_err(|_| WorkerError("The worker process lock is unavailable.".into()))?;
    if child
        .try_wait()
        .map_err(|error| WorkerError(format!("Unable to inspect worker process status: {error}")))?
        .is_some()
    {
        return Ok(());
    }

    let details = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(WorkerError(if details.is_empty() {
        "Unable to cancel worker process tree.".into()
    } else {
        format!("Unable to cancel worker process tree: {details}")
    }))
}

fn validate_request(request: &ProcessRequest) -> Result<()> {
    if request.program.trim().is_empty() {
        return Err(WorkerError(
            "A program is required to start a worker process.".into(),
        ));
    }
    if request.program.contains('\0')
        || request
            .arguments
            .iter()
            .any(|argument| argument.contains('\0'))
    {
        return Err(WorkerError(
            "Programs and arguments cannot contain null bytes.".into(),
        ));
    }
    if let Some(working_directory) = &request.working_directory {
        if !working_directory.is_dir() {
            return Err(WorkerError(
                "The requested working directory does not exist.".into(),
            ));
        }
    }
    Ok(())
}

fn spawn_output_reader<R>(reader: R, stream: OutputStream, sender: Sender<ProcessOutput>)
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        for result in BufReader::new(reader).lines() {
            let (text, failed) = match result {
                Ok(line) => (
                    truncate_output_line(strip_terminal_control_sequences(&line)),
                    false,
                ),
                Err(error) => (format!("Unable to read worker output: {error}"), true),
            };
            if sender
                .send(ProcessOutput {
                    stream: stream.clone(),
                    text,
                })
                .is_err()
            {
                return;
            }
            if failed {
                return;
            }
        }
    });
}

fn detect_tool(name: &str, arguments: &[&str]) -> ToolCapability {
    let output = Command::new(resolve_program(name)).args(arguments).output();
    let version = output
        .ok()
        .and_then(|output| {
            output.status.success().then(|| {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let text = if stdout.trim().is_empty() {
                    stderr
                } else {
                    stdout
                };
                text.lines()
                    .next()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(str::to_owned)
            })
        })
        .flatten();

    ToolCapability {
        name: name.to_owned(),
        installed: version.is_some(),
        version,
    }
}

fn resolve_program(program: &str) -> PathBuf {
    #[cfg(windows)]
    {
        let path = Path::new(program);
        if path.components().count() == 1 && path.extension().is_none() {
            for extension in ["cmd", "exe", "bat"] {
                let candidate = format!("{program}.{extension}");
                if let Some(path) = find_program_on_path(&candidate) {
                    return path;
                }
            }
        }
    }
    PathBuf::from(program)
}

#[cfg(windows)]
fn find_program_on_path(program: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|directory| directory.join(program))
            .find(|candidate| candidate.is_file())
    })
}

fn truncate_output_line(mut line: String) -> String {
    const MAX_OUTPUT_LINE_BYTES: usize = 16_000;
    if line.len() <= MAX_OUTPUT_LINE_BYTES {
        return line;
    }
    let boundary = line
        .char_indices()
        .take_while(|(index, _)| *index <= MAX_OUTPUT_LINE_BYTES)
        .map(|(index, _)| index)
        .last()
        .unwrap_or_default();
    line.truncate(boundary);
    line.push_str(" [output line truncated]");
    line
}

/// Removes ANSI color, cursor, and hyperlink control sequences before output
/// reaches the desktop UI. Terminal decoration is not meaningful in a GUI and
/// can otherwise leak raw escape characters into logs.
fn strip_terminal_control_sequences(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut characters = value.chars().peekable();

    while let Some(character) = characters.next() {
        if character != '\u{1b}' {
            output.push(character);
            continue;
        }

        match characters.next() {
            Some('[') => {
                while let Some(next) = characters.next() {
                    if ('\u{40}'..='\u{7e}').contains(&next) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(next) = characters.next() {
                    if next == '\u{7}' {
                        break;
                    }
                    if next == '\u{1b}' && characters.peek() == Some(&'\\') {
                        characters.next();
                        break;
                    }
                }
            }
            Some(_) | None => {}
        }
    }
    output
}

fn platform_os() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "macos",
        "linux" => "linux",
        other => other,
    }
}

fn platform_architecture() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        remote_job_exit, validate_remote_endpoint, LocalWorker, ProcessRequest, RemoteJobSnapshot,
        RemoteWorkerClient, RemoteWorkerConfig,
    };

    #[test]
    fn streams_output_from_a_structured_git_command() {
        let run = LocalWorker::start(ProcessRequest {
            program: "git".into(),
            arguments: vec!["--version".into()],
            working_directory: None,
            standard_input: None,
        })
        .expect("git starts");
        let output = run
            .output
            .into_iter()
            .map(|event| event.text)
            .collect::<Vec<_>>();
        let exit_status = run.handle.wait().expect("git completes");

        assert!(exit_status.success);
        assert!(output.iter().any(|line| line.contains("git version")));
    }

    #[test]
    fn writes_optional_standard_input_without_using_a_shell() {
        let run = LocalWorker::start(ProcessRequest {
            program: "git".into(),
            arguments: vec!["hash-object".into(), "--stdin".into()],
            working_directory: None,
            standard_input: Some("orchestr\n".into()),
        })
        .expect("git starts");
        let output = run
            .output
            .into_iter()
            .map(|event| event.text)
            .collect::<Vec<_>>();
        let exit_status = run.handle.wait().expect("git completes");

        assert!(exit_status.success);
        assert!(output.iter().any(|line| !line.is_empty()));
    }

    #[test]
    fn rejects_an_empty_program() {
        let result = LocalWorker::start(ProcessRequest {
            program: " ".into(),
            arguments: Vec::new(),
            working_directory: None,
            standard_input: None,
        });

        assert!(result.is_err());
    }

    #[test]
    fn remote_endpoints_require_https_and_are_normalized() {
        assert!(validate_remote_endpoint("http://worker.example").is_err());
        assert_eq!(
            validate_remote_endpoint(" https://worker.example:9443/ ").expect("HTTPS is valid"),
            "https://worker.example:9443"
        );
    }

    #[test]
    fn remote_client_accepts_system_or_custom_tls_roots() {
        RemoteWorkerClient::connect(RemoteWorkerConfig {
            endpoint: "https://worker.example:9443".into(),
            token: "test-token".into(),
            ca_certificate_pem: None,
        })
        .expect("system TLS roots configure");
        assert!(RemoteWorkerClient::connect(RemoteWorkerConfig {
            endpoint: "https://worker.example:9443".into(),
            token: "test-token".into(),
            ca_certificate_pem: Some(
                "-----BEGIN CERTIFICATE-----\ninvalid\n-----END CERTIFICATE-----".into(),
            ),
        })
        .is_err());
    }

    #[test]
    fn remote_terminal_snapshots_preserve_exit_results() {
        let success = remote_job_exit(snapshot("completed", Some(0), None))
            .expect("completed job returns an exit result");
        assert!(success.success);
        assert_eq!(success.code, Some(0));
        let cancelled = remote_job_exit(snapshot("cancelled", None, None))
            .expect("cancelled job returns an exit result");
        assert!(!cancelled.success);
        let failed = remote_job_exit(snapshot("failed", Some(2), Some("build failed".into())))
            .expect_err("failed job returns its error");
        assert_eq!(failed.to_string(), "build failed");
    }

    #[test]
    fn strips_terminal_color_and_hyperlink_control_sequences() {
        assert_eq!(
            super::strip_terminal_control_sequences("\u{1b}[94mDevice code\u{1b}[0m"),
            "Device code"
        );
        assert_eq!(
            super::strip_terminal_control_sequences(
                "\u{1b}]8;;https://auth.openai.com\u{7}Open link\u{1b}]8;;\u{7}"
            ),
            "Open link"
        );
    }

    fn snapshot(status: &str, exit_code: Option<i32>, error: Option<String>) -> RemoteJobSnapshot {
        RemoteJobSnapshot {
            id: "job-1".into(),
            status: status.into(),
            events: Vec::new(),
            next_cursor: 0,
            exit_code,
            error,
        }
    }

    #[cfg(windows)]
    #[test]
    fn detects_npm_through_its_windows_command_wrapper() {
        let profile = LocalWorker::profile();
        let npm = profile
            .tools
            .iter()
            .find(|tool| tool.name == "npm")
            .expect("npm capability is present");

        assert!(npm.installed);
        assert!(npm.version.is_some());
    }

    #[cfg(windows)]
    #[test]
    fn cancelling_a_command_wrapper_stops_its_process_tree() {
        let run = LocalWorker::start(ProcessRequest {
            program: "cmd".into(),
            arguments: vec![
                "/C".into(),
                "ping".into(),
                "127.0.0.1".into(),
                "-n".into(),
                "30".into(),
            ],
            working_directory: None,
            standard_input: None,
        })
        .expect("command wrapper starts");

        std::thread::sleep(std::time::Duration::from_millis(100));
        run.handle.cancel().expect("process tree cancels");
        let status = run.handle.wait().expect("process exits after cancellation");
        drop(run.output);

        assert!(!status.success);
    }
}
