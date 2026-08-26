# Remote worker setup

M21 adds a small TLS worker daemon that runs Orchestr's structured process
requests on another machine. The desktop authenticates every request with a
bearer token; provider credentials remain on the worker and are never copied
into Orchestr's database.

## Start a worker

Install the repository and Rust toolchain on the worker, then set:

- `ORCHESTR_REMOTE_CERT`: PEM TLS certificate path
- `ORCHESTR_REMOTE_KEY`: PEM private-key path
- `ORCHESTR_REMOTE_TOKEN`: a random bearer token containing at least 32 characters
- `ORCHESTR_REMOTE_ROOTS`: allowed workspace roots separated by `;`
- `ORCHESTR_REMOTE_BIND`: optional address, default `0.0.0.0:9443`
- `ORCHESTR_REMOTE_ID`: optional stable worker ID
- `ORCHESTR_REMOTE_NAME`: optional display name

Start the daemon from the repository:

```bash
cargo run -p orchestr-remote-worker
```

Expose only the configured TLS port through the host firewall. Use a
certificate issued by a trusted CA where practical. For a private CA, retain
the public CA PEM for desktop registration. Never copy the worker's private
key to the desktop.

## Register it in the desktop

Set an environment variable in the desktop process to the same bearer token.
Open **Workers**, then enter:

- the worker's `https://` endpoint;
- the token environment-variable name, not the token value;
- the public CA certificate path when the certificate is privately issued;
- the project and its workspace path on the worker.

Orchestr stores only the environment-variable name and public CA certificate.
It authenticates a heartbeat, verifies protocol version 1, and records the
reported OS, architecture, and installed tools. A project with an enabled
remote registration sends new Codex task runs to that worker.

## Workspace requirement

The M21 transport executes on another machine but keeps Git preparation,
review, and integration under the desktop control plane. The registered
workspace and its managed worktrees must therefore be reachable from both
machines, such as through a mounted network share. Workspace replication and
independent checkout synchronization are intentionally outside M21.

## Protocol behavior

The JSON HTTPS protocol supports:

- authenticated worker handshake and capability heartbeat;
- idempotently identified job creation;
- ordered stdout/stderr events with resumable cursors;
- job status and exit-code polling;
- cancellation;
- desktop restart reconnection to a still-running worker job.

The worker rejects plain HTTP, invalid bearer tokens, duplicate job IDs, and
working directories outside its configured roots. Jobs and event buffers are
currently retained in worker memory, so reconnection survives a desktop or
network interruption, not a worker-daemon restart.
