use std::{net::SocketAddr, path::PathBuf};

use orchestr_remote_worker::{serve, ServerConfig};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let config = remote_worker_config()?;
    println!(
        "Orchestr remote worker listening on https://{}",
        config.bind_address
    );
    serve(config).await
}

fn remote_worker_config() -> Result<ServerConfig, String> {
    required_environment("ORCHESTR_REMOTE_CERT").and_then(|certificate_path| {
        required_environment("ORCHESTR_REMOTE_KEY").and_then(|private_key_path| {
            required_environment("ORCHESTR_REMOTE_TOKEN").and_then(|authentication_token| {
                remote_bind_address().map(|bind_address| ServerConfig {
                    bind_address,
                    certificate_path: PathBuf::from(certificate_path),
                    private_key_path: PathBuf::from(private_key_path),
                    authentication_token,
                    allowed_workspace_roots: remote_workspace_roots(),
                    worker_id: environment("ORCHESTR_REMOTE_ID")
                        .unwrap_or_else(|| "remote-worker".into()),
                    worker_name: environment("ORCHESTR_REMOTE_NAME")
                        .unwrap_or_else(|| "Remote Worker".into()),
                })
            })
        })
    })
}

fn remote_bind_address() -> Result<SocketAddr, String> {
    environment("ORCHESTR_REMOTE_BIND")
        .unwrap_or_else(|| "0.0.0.0:9443".into())
        .parse::<SocketAddr>()
        .map_err(|error| format!("Invalid ORCHESTR_REMOTE_BIND: {error}"))
}

fn remote_workspace_roots() -> Vec<PathBuf> {
    environment("ORCHESTR_REMOTE_ROOTS")
        .unwrap_or_default()
        .split(';')
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .collect()
}

fn required_environment(name: &str) -> Result<String, String> {
    environment(name).ok_or_else(|| format!("Set {name} before starting the remote worker."))
}

fn environment(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
