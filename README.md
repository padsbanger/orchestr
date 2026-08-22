# Orchestr

Orchestr is a local-first engineering control room for Git-backed projects,
Kanban work, and eventually human-supervised AI workers.

## M0 development

Prerequisites:

- Node.js 22 or later
- Rust stable with Cargo and the platform prerequisites for [Tauri v2](https://v2.tauri.app/start/prerequisites/)

Install frontend dependencies and start the desktop app:

```bash
npm install
npm run tauri dev
```

Useful checks:

```bash
npm run check
npm run build
cargo test -p orchestr-db
```

The first launch creates Orchestr's local SQLite database in the operating
system application-data directory and applies its migrations automatically.

See [the M0 architecture notes](docs/architecture.md) for ownership boundaries
and planned crate extraction points.
