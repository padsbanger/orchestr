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
process lifecycle. It accepts a program plus argument array, never a shell
string, and returns separate stdout/stderr events. The Tauri host owns active
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

## Planned extraction points

The Rust workspace introduces crates only when a milestone needs their
boundary:

- `orchestr-core`: project/task application behavior (M2+)
- `orchestr-worker`, `orchestr-protocol`, `orchestr-platform`: worker runtime
  and remote protocol work (M4+)
