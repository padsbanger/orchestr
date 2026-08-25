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
validated and retained as structured references. The board's
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
Starting a run is an atomic application-layer transition: the assigned Ready
task moves to In Progress as its running record is created. A successfully
completed run moves that task to Review; failed and cancelled runs deliberately
remain In Progress for human follow-up.

The Tauri host builds a structured task prompt, asks the Codex provider for a
structured `codex exec` request, and delegates process execution to the local
worker. It persists and emits each output record, while the React task inspector
uses typed run services to display live and historical output and offer
cancellation. No React component constructs or runs a command directly.

The task prompt includes a completion contract: after validation, Codex must
commit task-related changes on its isolated branch and leave that worktree
clean. The host verifies this after a successful provider process; uncommitted
changes cause the run to fail and leave the task In Progress instead of moving
it to Review.

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

## M11 review workflow

Review is a human-controlled task stage. When a task enters Review, the task
inspector loads a typed `TaskReview` through the frontend review service and a
Tauri command. `orchestr-git` reads the task worktree to provide its branch,
commits relative to the project's default branch, a bounded committed and
uncommitted diff, and changed-file metadata. Existing persisted run events
remain visible in the same inspector, so review does not depend on the
primary workspace's repository inspector.

The task inspector provides explicit human actions to request changes or
approve the review. Requesting changes returns the task to In Progress while
preserving its branch and worktree. The worktree can also be opened in the
native file manager; the Tauri host verifies that it is the managed worktree
owned by the requested task before launching the platform file manager.

Approval is deliberately separate from Git integration. The legacy M11
approval transition currently records `Review -> Done`; M12 must migrate that
temporary transition to `Review -> Approved` and introduce a serialized,
observable integration operation. A task may be considered Done only after
its accepted branch has been integrated into the project's configured default
branch. Until that succeeds, its branch, worktree, commits, and run history
must remain available for inspection and recovery.

## M12 integration queue

Approval now creates a persisted, queued `IntegrationAttempt` and moves the
task from Review to Approved. The database owns attempt history, queue order,
and a per-project integration lock; only the queue processor may move a task
through Integrating to Done. Failed attempts return the task to Approved for a
retry, while Git conflicts move it to Blocked and preserve the branch and
worktree for manual or later agent-assisted resolution.

The Tauri integration command claims the next attempt under that lock, then
asks `orchestr-git` to verify clean primary and task worktrees, rebase the task
branch onto the current configured integration branch, and squash merge it.
Git conflicts are returned as structured results rather than silently resolved.
After a merge, the result and commit are persisted before best-effort cleanup
removes the task worktree and branch. A cleanup failure remains visible on the
successful attempt and never undoes the integrated commit.

The board exposes the queue in a dedicated inspector with its status, errors,
and retry action. The primary repository inspector reflects a successful merge
because the integration happens in the primary workspace.

## Quality gates and project health (M13)

The system's primary measure of progress is a healthy integration branch, not
agent activity. The intended flow is:

```text
Milestone / Epic -> Task -> READY -> IN_PROGRESS -> REVIEW -> APPROVED
    -> integration queue -> integration + validation -> healthy branch -> DONE
    -> dependent tasks re-evaluated as READY
```

The application/domain layer will own this state machine and expose typed
services for readiness, dependencies, integration, health, blockers, and
project knowledge. React will render and invoke those services; it will not
derive eligibility, manipulate Git, or decide integration policy itself.

### Readiness, dependencies, and flow control

`READY` and `BLOCKED` will be first-class task states. A task is Ready only
when it has sufficient specification and context, no unresolved project
blocker, a suitable execution environment, and every dependency is truly
Done. Reaching Review, approval, or a completed agent run never satisfies a
dependency. Cycles must be rejected, and a successful integration must trigger
re-evaluation of affected dependent tasks.

Priority (`critical`, `high`, `normal`, `low`) determines ordering among
eligible work; it does not bypass readiness. WIP limits and downstream
backpressure will prevent new implementation work from overwhelming Review or
the single-task integration stage.

### Integration queue and branch health

Validation commands are project-scoped, ordered, and split between
`implementation` and `integration` stages. A command has a display name, an
executable, and an argument array. It is deliberately not a shell snippet:
the worker runs `program + arguments` in the relevant workspace. This supports
Node, Rust, Python, JVM, and other projects without binding the quality model
to a package manager or a shell.

Each check run is a durable `ValidationAttempt`, with independently persisted
events for command start, stdout, stderr, exit code, and completion. The
desktop emits those events while they arrive so the quality-gate inspector can
show live output as well as history after restart.

Implementation validation runs in the isolated task worktree after Codex exits
cleanly. A configured failing check makes the run fail, so the task cannot
reach Review successfully. Integration validation runs only after the task
branch has been rebased onto the latest target branch and before its squash
merge. Thus its working directory is the exact candidate being integrated.

M12's queue remains serialized per project. Before claiming an attempt, the
integration service reads project health. A `broken` integration branch refuses
new integration until the operator runs the explicit recovery validation. A
failed integration check returns the task to Approved, records the failure, and
marks the project broken; it does not merge the candidate. A passing check
permits the squash merge and then records the branch as healthy with the latest
successful validation and integration timestamps.

The initial merge strategy is squash merge: task branches may contain
iterative commits, while the integration branch receives one clear task-level
commit. Original branch and run history remain available as Orchestr metadata.
The project-health record has four states: `unknown`, `healthy`, `degraded`,
and `broken`, plus the last validation, last known-good timestamp, last
integration timestamp, and failing gate. The board header exposes the current
state; the Quality Gates panel owns configuration, output history, and the
manual recovery action. Only a successful validation and merge can mark a task
Done.

## M14 readiness, dependencies, and priority

Migration `0011_task_readiness` replaces the ambiguous persisted `todo` state
with `ready`, adds a priority (`critical`, `high`, `normal`, or `low`), and
adds an operator-facing blocked reason. Existing Todo tasks migrate to Ready
and are immediately re-evaluated when the database opens.

`orchestr-db` owns the readiness policy. It validates that dependencies are
unique, refer to tasks in the same project, and cannot create a cycle. A task
is eligible for Ready when it has at least one acceptance criterion and all
its dependencies are truly Done. Moving a task to Ready evaluates that policy;
an ineligible task is placed in Blocked with the specific unmet requirement.
Agent execution independently re-checks the policy, so a direct API call
cannot bypass it.

Blocked and Ready tasks are recalculated after task edits, on database open,
and when a successful integration makes a task Done. The last case moves newly
eligible dependents from Blocked to Ready. Tasks remain in Backlog until an
operator intentionally promotes them for evaluation; priority guides the
operator and later scheduler but never bypasses eligibility. The React board
only renders the resulting status and reason through typed task services.

## M15 outcomes and project progress

`milestones` and `epics` are project-owned records in `orchestr-db`.
Milestones carry a title, optional description and target date, and a compact
outcome status (`planned`, `active`, `completed`, or `blocked`). Epics may
belong to one milestone and use the same outcome status. Task records retain
optional foreign-key links to both levels, so a task can be grouped directly
under a milestone or under a milestone's epic without moving repository data
into SQLite.

The database validates that all selected outcome records belong to the task's
project and that an epic's milestone agrees with the task's selected milestone.
That keeps hierarchy rules in the domain layer rather than trusting form state.
The `ProjectProgress` read model derives task counts from persisted workflow
status: Done, Ready, In Progress, Review, Blocked, Backlog, and total. It also
produces the same counts per milestone and its contained epics. An integrated
task is therefore reflected as Done only through the existing integration
workflow; milestones never infer completion from agent activity.

The React outcome service is a typed Tauri boundary for milestones, epics, and
the derived project-progress read model. The Progress route combines those
outcome metrics with the existing integration-branch health and queue state.
It supports creating milestones and epics, while task editing assigns work to
the applicable outcome hierarchy. React does not calculate progress locally or
make integration-health decisions.

## M16 architect review

An architect review is a durable `agent_reviews` record, not an implementation
run and not a hidden prompt-side state transition. It stores the separate
reviewer agent, lifecycle status, machine-readable decision, concise notes,
bounded raw provider output, failure detail, and timestamps. The database
accepts a review only while its task is in Review and rejects the task's
assigned implementation agent as reviewer.

The desktop prepares the reviewer with task context, acceptance criteria,
relevant paths, the branch diff and commits, the latest implementation-run
summary, and implementation-validation history. Project decision records are
not introduced until M20, so the prompt explicitly identifies that absence
rather than fabricating policy. Codex runs in its `read-only` sandbox with no
additional writable Git directories. Only an `agent_message` containing the
strict `ORCHESTR_REVIEW_DECISION` and `ORCHESTR_REVIEW_NOTES` protocol is
accepted; command output or repository content cannot impersonate a decision.

On a valid recommendation, the application layer performs the existing Review
transition: `approve` creates the ordinary serialized integration attempt, and
`request_changes` returns the task to In Progress. Neither path merges code or
marks a task Done. The Review inspector shows status, reviewer, notes, bounded
raw output, and cancellation while refreshing task state through its typed
agent-review service.

## M17 parallel local agents and flow control

Migration `0015_flow_control` turns the existing `queued` run status into a
durable execution queue. Queue order is priority-first (`critical`, `high`,
`normal`, `low`) and FIFO within a priority. Claiming work is an atomic
database transition: the run becomes Running and its Ready task becomes In
Progress in the same transaction. A partial unique index prevents more than
one queued or running implementation run for a task, while database triggers
prevent queued work from being silently edited, reassigned, or moved.

The claim policy checks capacity in this order: local-worker concurrency,
project health and WIP pressure, then the candidate agent's
`max_concurrent_tasks`. Project flow limits default to four In Progress, three
Review, and two Approved plus Integrating tasks; the local worker defaults to
four concurrent implementation runs. Review or Approved congestion pauses new
starts even when a worker or agent is idle. The existing per-project
integration claim remains serialized to one mutating attempt.

Tauri owns queue dispatch and process lifecycle. Each claimed task creates its
existing isolated branch/worktree and launches independently. A terminal run,
review decision, successful integration, limit update, new enqueue, or app
startup asks the scheduler to fill newly available slots. Launch preparation,
output forwarding, completion classification, validation, and persistence are
separate operations so failures remain observable and one run cannot hold the
database lock while another executes.

The board's Flow inspector exposes the worker and downstream occupancy, the
specific pressure reason, editable limits, and the ordered queue. Queued runs
remain Ready, may be cancelled without creating a worktree, survive restart,
and begin automatically when capacity returns. React only invokes the typed
flow-control service and renders the persisted read model; it does not decide
which task is eligible to start.

### Planning, blockers, and durable project context

Project-level blockers and task-level `NEEDS_INPUT` records will stop unsafe
or speculative execution. An input request retains its question, requesting
agent/run, answer, and resolution timestamps. Architecture decisions and other
durable project memory will be stored as ADR-style records so relevant,
accepted context can be inspected and supplied to future agent runs.

### Recovery and cleanup

Before successful integration, branches, worktrees, review history, run logs,
and integration attempts remain recoverable. Conflicts move work to an
actionable blocked state; infrastructure failures are retryable. Cleanup is
best-effort maintenance after an integrated, healthy result—its failure must
not rewrite history or claim that an already successful merge failed. Future
reverts will create normal Git history, retain the link to the original
integration, and update project health rather than resetting shared history.

## Planned extraction points

The Rust workspace introduces crates only when a milestone needs their
boundary:

- `orchestr-core`: project/task application behavior (M2+)
- `orchestr-worker`, `orchestr-protocol`, `orchestr-platform`: worker runtime
  and remote protocol work (M4+)
