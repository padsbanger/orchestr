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

## Planned extraction points

The Rust workspace introduces crates only when a milestone needs their
boundary:

- `orchestr-core`: project/task application behavior (M2+)
- `orchestr-worker`, `orchestr-protocol`, `orchestr-platform`: worker runtime
  and remote protocol work (M4+)
