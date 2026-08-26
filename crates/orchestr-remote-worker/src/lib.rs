use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use orchestr_worker::{
    LocalWorker, RemoteJobEvent, RemoteJobRequest, RemoteJobSnapshot, RemoteWorkerHandshake,
    WorkerHandle,
};
use serde::Deserialize;
use subtle::ConstantTimeEq;

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind_address: SocketAddr,
    pub certificate_path: PathBuf,
    pub private_key_path: PathBuf,
    pub authentication_token: String,
    pub allowed_workspace_roots: Vec<PathBuf>,
    pub worker_id: String,
    pub worker_name: String,
}

#[derive(Clone)]
struct ServerState {
    token: Arc<String>,
    roots: Arc<Vec<PathBuf>>,
    profile: RemoteWorkerHandshake,
    jobs: Arc<Mutex<HashMap<String, JobRecord>>>,
}

struct JobRecord {
    status: String,
    events: Vec<RemoteJobEvent>,
    exit_code: Option<i32>,
    error: Option<String>,
    cancel_requested: bool,
    handle: WorkerHandle,
}

#[derive(Debug, Deserialize)]
struct EventQuery {
    #[serde(default)]
    after: u64,
}

pub async fn serve(config: ServerConfig) -> Result<(), String> {
    let state = build_state(&config)?;
    let tls = axum_server::tls_rustls::RustlsConfig::from_pem_file(
        &config.certificate_path,
        &config.private_key_path,
    )
    .await
    .map_err(|error| format!("Unable to load worker TLS identity: {error}"))?;
    let router = Router::new()
        .route("/v1/worker", get(worker_handshake))
        .route("/v1/jobs", post(create_job))
        .route("/v1/jobs/{job_id}", get(job_snapshot).delete(cancel_job))
        .with_state(state);
    axum_server::bind_rustls(config.bind_address, tls)
        .serve(router.into_make_service())
        .await
        .map_err(|error| format!("Remote worker server stopped: {error}"))
}

fn build_state(config: &ServerConfig) -> Result<ServerState, String> {
    if config.authentication_token.trim().len() < 32 {
        return Err("The remote worker token must contain at least 32 characters.".into());
    }
    let roots = canonical_workspace_roots(&config.allowed_workspace_roots)?;
    let mut profile = LocalWorker::profile();
    profile.id.clone_from(&config.worker_id);
    profile.name.clone_from(&config.worker_name);
    Ok(ServerState {
        token: Arc::new(config.authentication_token.clone()),
        roots: Arc::new(roots),
        profile: RemoteWorkerHandshake {
            protocol_version: PROTOCOL_VERSION,
            profile,
        },
        jobs: Arc::new(Mutex::new(HashMap::new())),
    })
}

fn canonical_workspace_roots(roots: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    if roots.is_empty() {
        return Err("Configure at least one allowed workspace root.".into());
    }
    roots
        .iter()
        .map(|root| {
            std::fs::canonicalize(root).map_err(|error| {
                format!(
                    "Unable to access workspace root {}: {error}",
                    root.display()
                )
            })
        })
        .collect()
}

async fn worker_handshake(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    match authorize(&headers, &state.token) {
        Ok(()) => (StatusCode::OK, Json(state.profile)).into_response(),
        Err(status) => status.into_response(),
    }
}

async fn create_job(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<RemoteJobRequest>,
) -> impl IntoResponse {
    if let Err(status) = authorize(&headers, &state.token) {
        return status.into_response();
    }
    match start_job(&state, request) {
        Ok(snapshot) => (StatusCode::CREATED, Json(snapshot)).into_response(),
        Err((status, message)) => (status, message).into_response(),
    }
}

async fn job_snapshot(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxumPath(job_id): AxumPath<String>,
    Query(query): Query<EventQuery>,
) -> impl IntoResponse {
    if let Err(status) = authorize(&headers, &state.token) {
        return status.into_response();
    }
    match snapshot_job(&state.jobs, &job_id, query.after) {
        Some(snapshot) => (StatusCode::OK, Json(snapshot)).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn cancel_job(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxumPath(job_id): AxumPath<String>,
) -> impl IntoResponse {
    if let Err(status) = authorize(&headers, &state.token) {
        return status;
    }
    match request_job_cancellation(&state.jobs, &job_id) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(status) => status,
    }
}

fn authorize(headers: &HeaderMap, expected_token: &str) -> Result<(), StatusCode> {
    let supplied = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    let matches = supplied.len() == expected_token.len()
        && supplied.as_bytes().ct_eq(expected_token.as_bytes()).into();
    if matches {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn start_job(
    state: &ServerState,
    mut request: RemoteJobRequest,
) -> Result<RemoteJobSnapshot, (StatusCode, String)> {
    validate_job_workspace(&mut request, &state.roots)
        .map_err(|message| (StatusCode::BAD_REQUEST, message))?;
    if state
        .jobs
        .lock()
        .map_err(|_| internal_state_error())?
        .contains_key(&request.id)
    {
        return Err((
            StatusCode::CONFLICT,
            "The remote job already exists.".into(),
        ));
    }
    let run = LocalWorker::start(request.process)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    let handle = run.handle.clone();
    {
        let mut jobs = state.jobs.lock().map_err(|_| internal_state_error())?;
        if jobs.contains_key(&request.id) {
            let _ = run.handle.cancel();
            return Err((
                StatusCode::CONFLICT,
                "The remote job already exists.".into(),
            ));
        }
        jobs.insert(
            request.id.clone(),
            JobRecord {
                status: "running".into(),
                events: Vec::new(),
                exit_code: None,
                error: None,
                cancel_requested: false,
                handle,
            },
        );
    }
    monitor_job(request.id.clone(), run, Arc::clone(&state.jobs));
    snapshot_job(&state.jobs, &request.id, 0).ok_or_else(internal_state_error)
}

fn validate_job_workspace(request: &mut RemoteJobRequest, roots: &[PathBuf]) -> Result<(), String> {
    let requested = request
        .process
        .working_directory
        .as_deref()
        .ok_or_else(|| "Remote jobs require a working directory.".to_owned())?;
    let canonical = std::fs::canonicalize(requested)
        .map_err(|error| format!("Unable to access the remote working directory: {error}"))?;
    if !roots.iter().any(|root| canonical.starts_with(root)) {
        return Err("The working directory is outside the worker's allowed roots.".into());
    }
    request.process.working_directory = Some(canonical);
    Ok(())
}

fn monitor_job(
    job_id: String,
    run: orchestr_worker::WorkerRun,
    jobs: Arc<Mutex<HashMap<String, JobRecord>>>,
) {
    thread::spawn(move || {
        for output in run.output {
            if let Ok(mut jobs) = jobs.lock() {
                if let Some(job) = jobs.get_mut(&job_id) {
                    let sequence = job.events.last().map_or(1, |event| event.sequence + 1);
                    job.events.push(RemoteJobEvent {
                        sequence,
                        stream: output.stream,
                        text: output.text,
                    });
                }
            }
        }
        let result = run.handle.wait();
        if let Ok(mut jobs) = jobs.lock() {
            if let Some(job) = jobs.get_mut(&job_id) {
                finish_job(job, result);
            }
        }
    });
}

fn finish_job(job: &mut JobRecord, result: orchestr_worker::Result<orchestr_worker::ProcessExit>) {
    match result {
        Ok(exit) => {
            job.exit_code = exit.code;
            job.status = if job.cancel_requested {
                "cancelled"
            } else if exit.success {
                "completed"
            } else {
                "failed"
            }
            .into();
            if job.status == "failed" {
                job.error = Some("The remote process exited with an error.".into());
            }
        }
        Err(error) => {
            job.status = "failed".into();
            job.error = Some(error.to_string());
        }
    }
}

fn snapshot_job(
    jobs: &Arc<Mutex<HashMap<String, JobRecord>>>,
    job_id: &str,
    after: u64,
) -> Option<RemoteJobSnapshot> {
    let jobs = jobs.lock().ok()?;
    let job = jobs.get(job_id)?;
    Some(RemoteJobSnapshot {
        id: job_id.to_owned(),
        status: job.status.clone(),
        events: job
            .events
            .iter()
            .filter(|event| event.sequence > after)
            .cloned()
            .collect(),
        next_cursor: job.events.last().map_or(after, |event| event.sequence),
        exit_code: job.exit_code,
        error: job.error.clone(),
    })
}

fn request_job_cancellation(
    jobs: &Arc<Mutex<HashMap<String, JobRecord>>>,
    job_id: &str,
) -> Result<(), StatusCode> {
    let handle = {
        let mut jobs = jobs.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let job = jobs.get_mut(job_id).ok_or(StatusCode::NOT_FOUND)?;
        if job.status != "running" {
            return Ok(());
        }
        job.cancel_requested = true;
        job.handle.clone()
    };
    handle
        .cancel()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn internal_state_error() -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "The remote worker state is unavailable.".into(),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        authorize, build_state, canonical_workspace_roots, snapshot_job, start_job,
        validate_job_workspace, ServerConfig,
    };
    use axum::http::{header::AUTHORIZATION, HeaderMap, HeaderValue, StatusCode};
    use orchestr_worker::{ProcessRequest, RemoteJobRequest};

    #[test]
    fn authentication_requires_an_exact_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer wrong"));
        assert_eq!(
            authorize(&headers, "correct"),
            Err(StatusCode::UNAUTHORIZED)
        );
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer correct"));
        assert_eq!(authorize(&headers, "correct"), Ok(()));
    }

    #[test]
    fn remote_jobs_are_confined_to_configured_workspace_roots() {
        let root = temporary_path("root");
        let outside = temporary_path("outside");
        std::fs::create_dir_all(root.join("project")).expect("root creates");
        std::fs::create_dir_all(&outside).expect("outside creates");
        let roots = canonical_workspace_roots(std::slice::from_ref(&root)).expect("root validates");
        let mut allowed = job(root.join("project"));
        validate_job_workspace(&mut allowed, &roots).expect("nested workspace is allowed");
        let mut rejected = job(outside.clone());
        assert!(validate_job_workspace(&mut rejected, &roots).is_err());
        std::fs::remove_dir_all(root).expect("root removes");
        std::fs::remove_dir_all(outside).expect("outside removes");
    }

    #[test]
    fn remote_job_events_resume_from_a_cursor() {
        let root = temporary_path("execution");
        std::fs::create_dir_all(&root).expect("root creates");
        let state = build_state(&ServerConfig {
            bind_address: "127.0.0.1:0".parse().expect("address parses"),
            certificate_path: PathBuf::from("unused-cert.pem"),
            private_key_path: PathBuf::from("unused-key.pem"),
            authentication_token: "01234567890123456789012345678901".into(),
            allowed_workspace_roots: vec![root.clone()],
            worker_id: "remote-test".into(),
            worker_name: "Remote test".into(),
        })
        .expect("state builds");
        start_job(&state, job(root.clone())).expect("job starts");
        let completed = (0..100)
            .find_map(|_| {
                let snapshot = snapshot_job(&state.jobs, "job-1", 0).expect("job exists");
                if snapshot.status == "running" {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    None
                } else {
                    Some(snapshot)
                }
            })
            .expect("job completes");
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.exit_code, Some(0));
        assert!(!completed.events.is_empty());
        let resumed = snapshot_job(&state.jobs, "job-1", completed.next_cursor)
            .expect("job remains reconnectable");
        assert!(resumed.events.is_empty());
        std::fs::remove_dir_all(root).expect("root removes");
    }

    fn job(path: PathBuf) -> RemoteJobRequest {
        RemoteJobRequest {
            id: "job-1".into(),
            process: ProcessRequest {
                program: "git".into(),
                arguments: vec!["--version".into()],
                working_directory: Some(path),
                standard_input: None,
            },
        }
    }

    fn temporary_path(label: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock advances")
            .as_nanos();
        std::env::temp_dir().join(format!("orchestr-remote-{label}-{unique}"))
    }

    use std::path::PathBuf;
}
