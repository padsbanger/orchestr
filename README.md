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
npm run tauri -- dev
```

Useful checks:

```bash
npm run check
npm run build
cargo test -p orchestr-db
cargo test -p orchestr-git
```

The first launch creates Orchestr's local SQLite database in the operating
system application-data directory and applies its migrations automatically.

## Releases

The release workflow uses the numeric Git tag as the Tauri bundle version and
installer filename. For example:

```bash
git tag 0.2.1
git push origin 0.2.1
```

This publishes `Orchestr_0.2.1_x64-setup.exe`.

See [the M0 architecture notes](docs/architecture.md) for ownership boundaries
and planned crate extraction points.
