# Architecture

## M0 boundaries

`apps/desktop` contains the React/Vite interface. It communicates with the
Tauri host only through typed command wrappers in `src/services`; React
components never access SQLite or execute operating-system commands.

`apps/desktop/src-tauri` is the local host. At M0 it initializes the app-data
directory, opens SQLite, runs migrations, and exposes the small settings API.

`crates/orchestr-db` owns SQLite connection setup, migration execution, and
persistence repositories for settings, projects, workspaces, and Kanban tasks.

`crates/orchestr-git` owns validated interactions with the installed `git`
executable. It accepts explicit workspace paths, passes command and argument
arrays without a shell, and resolves the actual repository root before a
workspace is persisted. Its repository-inspection API returns typed summaries,
recent commits, changed files, and bounded diffs; the Tauri host resolves a
project's local workspace before invoking it.

The board uses typed frontend service wrappers to request repository data. The
repository inspector is a non-blocking drawer, so Git activity can be viewed
without leaving or mutating Kanban workflow state.

## M4 worker boundary

`crates/orchestr-worker` owns cross-platform local capability detection and
process lifecycle. It accepts a program plus argument array and optional
standard input, never a shell string, and returns separate stdout/stderr events. The Tauri host owns active
run IDs and forwards those events to the UI; it also retains cancellation
handles outside React and the database.

The initial Workers screen exposes one implicit local worker and a predefined
`git --version` diagnostic. This demonstrates streamed execution without
opening an arbitrary command console before task/run authorization exists.

## M5 task specifications

Task specifications are persisted with the task in SQLite: acceptance criteria,
implementation notes, relevant paths, and dependency IDs. Dependency IDs are
validated and retained as structured references, but do not yet alter task
workflow or execution eligibility; that behavior belongs to M14. The board's
task inspector is read-focused, while the task dialog is the sole editing
surface for the specification.

## M6 provider boundary

`crates/orchestr-provider` defines provider status and structured provider
actions. The Codex provider detects the local CLI and checks its official login
status command through the worker runtime. Login, logout, and status actions
are executed by the Codex CLI on the worker; Orchestr stores no OAuth or API
credentials and never returns command output as credential data.

## M7 agent configuration

Agents are persisted configuration records: provider, role, optional model and
system instructions, skills, and a concurrency limit. They contain no provider
credentials. Tasks hold an optional agent ID; the application layer validates
that assignment against the registry, and deleting an agent clears its task
assignments safely.

## M8 task execution

Runs are persisted by `orchestr-db` with their task, agent, worker, lifecycle
timestamps, terminal status, exit code, and streamed stdout/stderr records.
Starting a run is an atomic application-layer transition: the assigned Todo
task moves to In Progress as its running record is created. A successfully
completed run moves that task to Review; failed and cancelled runs deliberately
remain In Progress for human follow-up.

The Tauri host builds a structured task prompt, asks the Codex provider for a
structured `codex exec` request, and delegates process execution to the local
worker. It persists and emits each output record, while the React task inspector
uses typed run services to display live and historical output and offer
cancellation. No React component constructs or runs a command directly.

## M9 execution timeline

Each run owns an ordered, persisted event stream. Provider JSONL records are
classified into lifecycle, command, validation, and agent-message events;
the host additionally snapshots Git state before and after execution to record
changed files and newly created commits. The event stream is migrated from the
earlier raw output records, survives restart, and is rendered as a chronological
timeline in the task inspector.

## M10 task isolation

Before a Codex run starts, the Tauri application layer asks `orchestr-git` to
create a generated `task/...` branch and a separate checkout at a sibling
`.orchestr-worktrees/<project>/<task>` path. The main workspace is never used
as the agent working directory. Branch and worktree ownership are persisted on
the task, and their creation is recorded as run events.

An isolated worktree requires an existing commit to share as its base. Orchestr
therefore rejects execution in an unborn repository branch and tells the user
to create the initial commit first, rather than copying uncommitted files into
an ambiguous agent workspace.

Projects created by Orchestr receive that empty baseline commit automatically.
When Git has no configured identity, only that generated commit uses the local
`Orchestr <orchestr@local>` fallback; no global or repository Git configuration
is changed. Registered repositories are never committed automatically.

Worktree removal is an explicit user action. It uses `git worktree remove`
without force, so uncommitted changes remain protected; the branch is retained
for the subsequent review workflow. Tasks that still own a worktree cannot be
deleted, preventing an untracked checkout from being orphaned.

## Planned extraction points

The Rust workspace introduces crates only when a milestone needs their
boundary:

- `orchestr-core`: project/task application behavior (M2+)
- `orchestr-worker`, `orchestr-protocol`, `orchestr-platform`: worker runtime
  and remote protocol work (M4+)
