# Architecture

## M0 boundaries

`apps/desktop` contains the React/Vite interface. It communicates with the
Tauri host only through typed command wrappers in `src/services`; React
components never access SQLite or execute operating-system commands.

`apps/desktop/src-tauri` is the local host. At M0 it initializes the app-data
directory, opens SQLite, runs migrations, and exposes the small settings API.

`crates/orchestr-db` owns SQLite connection setup, migration execution, and
persistence repositories. M1 will add project/workspace repositories here;
Git behavior will live behind a separate service rather than in the UI.

## Planned extraction points

The Rust workspace starts with only the database crate because it is required
by M0. Future crates are introduced only with their milestones:

- `orchestr-core`: project/task application behavior (M1/M2)
- `orchestr-git`: validated Git operations (M1/M3)
- `orchestr-worker`, `orchestr-protocol`, `orchestr-platform`: worker runtime
  and remote protocol work (M4+)

