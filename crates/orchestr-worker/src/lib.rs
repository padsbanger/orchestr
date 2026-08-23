//! Cross-platform local process execution for Orchestr workers.
//!
//! This crate accepts a program and argument array; it never invokes a shell.
//! The desktop host owns run identity and persistence, while this runtime owns
//! local capability detection, process lifecycle, and output streaming.

use std::{
    fmt,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

use serde::Serialize;

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerProfile {
    pub id: String,
    pub name: String,
    pub os: String,
    pub architecture: String,
    pub status: String,
    pub tools: Vec<ToolCapability>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCapability {
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProcessRequest {
    pub program: String,
    pub arguments: Vec<String>,
    pub working_directory: Option<PathBuf>,
    pub standard_input: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
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
                child: Arc::new(Mutex::new(child)),
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
    child: Arc<Mutex<Child>>,
}

impl WorkerHandle {
    pub fn wait(&self) -> Result<ExitStatus> {
        self.child
            .lock()
            .map_err(|_| WorkerError("The worker process lock is unavailable.".into()))?
            .wait()
            .map_err(|error| WorkerError(format!("Unable to wait for worker process: {error}")))
    }

    pub fn cancel(&self) -> Result<()> {
        let mut child = self
            .child
            .lock()
            .map_err(|_| WorkerError("The worker process lock is unavailable.".into()))?;
        match child.try_wait() {
            Ok(Some(_)) => Ok(()),
            Ok(None) => {
                #[cfg(windows)]
                {
                    // npm-installed CLIs such as Codex are launched through a .cmd wrapper.
                    // Killing only that wrapper leaves its Node child alive (and its output pipes
                    // open), so the run never reaches its terminal state. `taskkill /T` stops the
                    // complete process tree rooted at the worker process.
                    let process_id = child.id();
                    drop(child);
                    terminate_process_tree(process_id, &self.child)
                }

                #[cfg(not(windows))]
                {
                    child.kill().map_err(|error| {
                        WorkerError(format!("Unable to cancel worker process: {error}"))
                    })
                }
            }
            Err(error) => Err(WorkerError(format!(
                "Unable to inspect worker process status: {error}"
            ))),
        }
    }
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
    use super::{LocalWorker, ProcessRequest};

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

        assert!(exit_status.success());
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

        assert!(exit_status.success());
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

        assert!(!status.success());
    }
}
