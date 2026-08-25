# Issues

## Destructive actions do not wait for confirmation in the Tauri app

- **Severity:** High
- **Observed:** 2026-08-24 on Windows, running `npm.cmd run tauri -- dev`
- **Affected flows:** Remove project, delete task, and delete agent

### Reproduction

1. Launch the desktop app.
2. Open a project and click a task's Delete button, or click Remove on a project or agent.
3. Observe the operation and the WebView console.

### Actual behavior

No usable confirmation dialog is shown. The console reports an unhandled rejection:

```text
dialog.confirm not allowed. Command not found
```

The handlers use `if (!window.confirm(...)) return`, but the dialog plugin replaces
`window.confirm` with an asynchronous function. Its returned `Promise` is truthy, so
the delete request starts immediately without waiting for the user's decision. If the
confirmation IPC rejects, that rejection is also left unhandled. During reproduction,
project removal surfaced only the generic `Unable to remove the project.` message;
task deletion gave no visible error.

Affected call sites:

- `apps/desktop/src/pages/DashboardPage/DashboardPage.tsx`
- `apps/desktop/src/pages/BoardPage/BoardPage.tsx`
- `apps/desktop/src/pages/AgentsPage/AgentsPage.tsx`

### Expected behavior

Each destructive action must await a supported confirmation dialog and invoke the
delete command only after the user explicitly confirms. Cancelling must leave data
unchanged, and a dialog failure must be caught and shown to the user.

### Suggested acceptance checks

- Use the async `confirm` API from `@tauri-apps/plugin-dialog` (or an in-app modal)
  and await its result in all three flows.
- Verify that cancelling never invokes the corresponding delete command.
- Verify that confirming invokes it exactly once.
- Verify that dialog failures produce a visible error without invoking deletion.
- Add regression tests for project, task, and agent deletion confirmations.
