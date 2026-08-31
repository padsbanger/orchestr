# Orchestr Engineering Case Study

## Context

Orchestr began as a local Kanban application and grew into a desktop control
plane for software work performed by humans and AI agents. The difficult part
was not invoking an AI CLI. It was preserving delivery correctness while work
moved through several independent systems:

- a durable task state machine;
- provider processes that can exit, hang, or lose connectivity;
- Git branches and worktrees that may contain valuable partial work;
- review and integration decisions made later by another actor;
- project-defined validation commands; and
- a React interface receiving both snapshots and live events.

The implementation is local-first: source stays in user-owned Git repositories,
orchestration metadata lives in SQLite, and privileged work is performed by the
Tauri/Rust host or an authenticated worker. This document focuses on the
hardest engineering problems visible in the repository, how they were solved,
and what the solutions cost.

The most important product constraint was:

> Agent output is not project progress. Accepted, integrated work on a healthy
> target branch is project progress.

## 1. Keeping task, process, review, and integration state consistent

### Why this was hard

A single “task complete” flag is unsafe once execution is asynchronous. A
provider process can complete while its files are uncommitted, validation can
fail after a successful process exit, a reviewer can request changes, or an
approved branch can conflict with newer local work. Treating any of those events as
Done would make downstream dependencies eligible too early.

There are also multiple failure windows. The desktop might stop after a run is
queued but before a process starts, after a persisted merge but before cleanup,
or while an integration lock is held. Recovery has to preserve evidence without
repeating a side effect blindly. Git and SQLite cannot share a transaction, so
a crash after Git creates a merge commit but before SQLite records it remains a
known reconciliation gap.

### Solution

Orchestr separates the lifecycles:

- `TaskStatus` represents delivery state: Backlog, Ready, In Progress, Needs
  Input, Review, Approved, Integrating, Blocked, and Done.
- `RunStatus` represents one execution attempt: Queued, Running, Failed,
  Completed, or Cancelled.
- `IntegrationAttempt` has its own queue and terminal states.
- Validation, review, recovery, input, and revert records are separate durable
  entities.

The database owns the critical transitions. Claiming queued work changes the
run to Running and the task to In Progress in one transaction. A successful run
can move the task only to Review. Approval creates an integration attempt and
moves the task to Approved; it does not merge or mark the task Done. Only the
integration completion path can make the final transition.

That separation is implemented in
[`crates/orchestr-db/src/lib.rs`](crates/orchestr-db/src/lib.rs), while
[`apps/desktop/src-tauri/src/main.rs`](apps/desktop/src-tauri/src/main.rs)
coordinates processes and Git around those transactions.

Recovery is modeled instead of inferred. Failed and cancelled runs can link to
a replacement run or an explicit abandon/escalate action. Needs Input records
retain the question, requesting run and agent, answer, and timestamps. Startup
also recovers interrupted integration and autonomy records into inspectable
states rather than assuming that in-memory work continued.

### Failure semantics

| Failure point | Persisted result | Recoverable material |
| --- | --- | --- |
| Provider exits unsuccessfully | Run Failed; task remains In Progress | Run output/events, branch, worktree, partial files |
| Agent requests a decision | Task Needs Input; input request remains open | Question, run/agent identity, worktree |
| Implementation validation fails | Run Failed; task does not reach Review | Command output, validation attempt, worktree |
| Review requests changes | Task returns to In Progress | Run history and branch/worktree; the rerun path is currently incomplete |
| Update/rebase conflicts | Integration attempt Conflict; task Blocked | Conflicting branch and worktree |
| Integration validation fails | No merge; project marked Broken | Validation events and integration attempt |
| Merge succeeds but cleanup fails | Task remains Done; cleanup error is recorded | Landed commit plus any cleanup artifacts not yet removed |

### Trade-off

This produces a larger state machine and more recovery code than a simple job
queue. The benefit is that completion has one defensible meaning, failures are
not erased, and dependent tasks do not advance because a process happened to
exit with code zero.

One guard is still weaker than the intended contract: successful execution
checks that the task worktree is clean, but it does not require a new commit or
non-empty diff. A clean no-op run can reach Review and must be caught there.

Database tests exercise readiness, transitions, queues, locks, integration,
recovery, metrics, and workflow projections in
[`crates/orchestr-db/src/lib.rs`](crates/orchestr-db/src/lib.rs). The current
large file is also maintainability debt; the transaction boundaries are clear,
but the implementation should be split into domain-focused modules.

## 2. Allowing parallel implementation without unsafe integration

### Why this was hard

Parallel agents should not share a mutable checkout. At the same time, passing
tests on an old branch is not enough: two individually correct tasks can become
incompatible after one lands. Git conflicts and cleanup errors also need to be
reported without discarding recoverable work.

### Solution

Each implementation task receives a generated branch and a sibling Git
worktree. The primary project checkout is not used as the provider's working
directory. Worktree ownership is stored on the task, and execution refuses an
unborn repository because Git has no commit from which to create an isolated
checkout.

The integration path is serialized against other Orchestr attempts by a
per-project database lock:

1. claim the next queued integration attempt;
2. verify the primary and task worktrees are in an acceptable state;
3. update the task branch onto the current local configured target branch;
4. return conflicts as structured results;
5. run integration validation on that updated candidate;
6. squash-merge the candidate;
7. persist the merge, healthy project state, and Done transition;
8. re-evaluate dependent tasks as part of that database transition; and
9. perform branch/worktree cleanup as a best-effort maintenance step.

The Git boundary is centralized in
[`crates/orchestr-git/src/lib.rs`](crates/orchestr-git/src/lib.rs). It invokes
the installed `git` executable with a program plus argument array rather than a
shell string. That keeps React and general application code away from raw shell
behavior and avoids interpolating branch names or paths into a shell command.

Validation is intentionally split:

- implementation validation asks whether the task works in its isolated
  worktree;
- integration validation asks whether the candidate still works after being
  updated onto the current local target.

Validation commands are stored as executable plus arguments, not shell
snippets, so a project can configure Node, Rust, Python, JVM, or other tooling
without making the core model package-manager-specific.

### A subtle decision: merge success and cleanup success are different

An early design temptation is to wrap merge and cleanup into one success flag.
That creates an incorrect outcome if Git has already landed the commit but a
worktree cannot be removed. Retrying “integration” could then duplicate work or
misreport branch health.

On the normal path, Orchestr persists successful integration before cleanup. A
cleanup failure is attached to the successful attempt and can be retried
independently. This is a small ordering decision with a large effect on
recoverability, but it does not make Git and SQLite atomic: a crash between the
Git commit and database persistence still needs manual reconciliation.

### Trade-off

Git worktrees use extra disk space and leave more artifacts to manage. Squash
merge also means the target receives one task-level commit rather than the
branch's iterative history. Run events, the integration attempt, the final
observed branch commit, and the merge commit remain available in Orchestr, but
the full deleted branch history is not copied into SQLite and the current merge
strategy is not configurable. The project lock also cannot prevent an external
process from changing the primary checkout during validation; integration
therefore assumes exclusive use of that checkout for the operation.

Temporary-repository tests in
[`crates/orchestr-git/src/lib.rs`](crates/orchestr-git/src/lib.rs) cover
inspection, worktree behavior, update/conflict cases, squash integration,
cleanup, and revert without depending on a developer's repository.

## 3. Building a responsive cockpit without creating a second state machine

### Why this was hard

The interface needs a compact answer to “what needs attention now?” The source
data is distributed across tasks, runs, agent reviews, integration attempts,
input requests, blockers, health, scheduler decisions, agents, and workers.
Loading every log and diff for every card would be wasteful, while deriving the
workflow independently in React would risk disagreement with the database.

Live events introduce another race: an older snapshot request can resolve after
a newer one and overwrite fresh state. Bursts of stdout or several domain
mutations can also trigger redundant refreshes.

### Solution

The Rust persistence layer builds a project-scoped `ProjectWorkflowSnapshot`.
It projects the canonical nine task states into four display stages—Queue,
Build, Verify & Land, and Done—and derives:

- the current actor;
- the next action or waiting reason;
- readiness details;
- attention items;
- agent activity; and
- integration-branch health.

Blocked tasks retain their last meaningful stage through task-status history,
so “Blocked” is an exception state rather than a misleading fifth delivery
phase. The snapshot deliberately excludes large logs, diffs, and validation
output; inspectors load those details only when the relevant tab opens.

The frontend boundary in
[`apps/desktop/src/services/workflow.ts`](apps/desktop/src/services/workflow.ts)
normalizes the wire shape and contains a conservative compatibility projection
for rolling schema changes. The board in
[`apps/desktop/src/pages/BoardPage/BoardPage.tsx`](apps/desktop/src/pages/BoardPage/BoardPage.tsx)
uses request sequencing to reject stale responses and coalesces workflow-change
events before refreshing. This keeps SQLite authoritative while still giving
the renderer responsive updates.

Drag-and-drop is deliberately narrow. It can reorder or move planning work
between Backlog and Ready, with optimistic UI and rollback on failure. Starting
execution, resolving Needs Input, approving review, and integrating use explicit
commands, so a visual gesture cannot bypass domain policy.

### Trade-off

The backend read model reduces duplicated business logic, but it creates a
cross-language contract whose Rust and TypeScript types are currently maintained
by hand. The compatibility normalization helps at runtime but is not equivalent
to generated end-to-end types. `BoardPage.tsx` has also accumulated substantial
coordination logic and is a candidate for extraction into focused hooks or a
state coordinator.

Frontend tests cover the snapshot service, board model, cockpit behavior, task
detail behavior, and service wrappers under
[`apps/desktop/src`](apps/desktop/src). They do not yet drive the complete
packaged Tauri application.

## 4. Scheduling for flow, not maximum agent utilization

### Why this was hard

“Run the highest-priority task whenever an agent is idle” is not a safe
scheduler. A task may lack acceptance criteria, depend on unfinished work, be
affected by a project blocker, require a capability no worker has, or be unsafe
to start while Review and Approved queues are already congested.

Those decisions also need to be explainable. A card that simply stays Ready
without a reason is difficult to operate.

### Solution

Eligibility is evaluated before priority. The scheduler checks, in broad order:

- task specification and completed dependencies;
- project blockers and integration-branch health;
- assigned provider readiness;
- one worker satisfying every required capability;
- worker and agent concurrency;
- project In Progress WIP; and
- downstream Review and Approved/Integrating capacity.

Only eligible work is ordered by critical, high, normal, and low priority.
Queue claim is transactional, and partial unique indexes prevent multiple active
runs for one task. Integration has a separate one-at-a-time project lock.

Every scheduling result is stored with an outcome and human-readable reason in
the `scheduler_decisions` table introduced by
[`crates/orchestr-db/migrations/0021_capability_scheduler.sql`](crates/orchestr-db/migrations/0021_capability_scheduler.sql).
The same reasons feed the workflow snapshot, so the UI can distinguish “waiting
for a dependency” from “no authenticated worker” or “review capacity is full.”

### Trade-off

This scheduler intentionally leaves workers idle when starting more work would
increase unfinished inventory. It optimizes for integrated flow rather than
utilization. The policy is more complex, and because capability/provider state
can change outside SQLite, the Tauri layer augments persisted state with live
local-worker capability and provider readiness before returning the cockpit
snapshot. Remote readiness comes from the most recently persisted handshake.

## 5. Extending execution to remote machines without turning the desktop into an open shell

### Why this was hard

A remote worker is a privileged command executor. A protocol that accepts an
arbitrary shell string, trusts caller-provided paths, or exposes provider tokens
would be an unacceptable boundary. Network interruptions also must not scramble
the ordered run timeline.

### Solution

[`crates/orchestr-remote-worker`](crates/orchestr-remote-worker) exposes a small
HTTPS protocol. It requires an exact bearer token, performs a constant-time
byte comparison after checking token length, canonicalizes requested working
directories, and rejects jobs whose working directory is outside
administrator-configured roots. Requests contain a program and argument array,
not a shell command. Provider credentials remain on the worker; the desktop
stores only the name of the environment variable containing the bearer token.

Worker events have monotonically increasing cursors. The desktop can poll after
its last persisted cursor, tolerate a bounded connection interruption, and feed
remote stdout/stderr through the same run-event pipeline used for local
execution. Cancellation uses the shared worker-handle abstraction in
[`crates/orchestr-worker`](crates/orchestr-worker).

### Trade-off and current boundary

The protocol does not synchronize repositories. Git preparation, review, and
integration remain desktop-owned, so both machines need access to the same
registered workspace through a shared or mounted path. Jobs and buffered events
are held in daemon memory: a desktop or network interruption is recoverable
while the daemon lives, but a daemon restart loses that process state.

Configured validation also currently runs on the desktop's local worker after a
remote implementation. This keeps one validation path but means a toolchain
available only on the remote machine cannot satisfy a gate yet.

## Security and trust-boundary decisions

Several smaller decisions support the larger workflow:

- React never opens SQLite or launches operating-system commands directly.
- Git and process APIs use executable/argument structures rather than shell
  interpolation.
- Workspace paths are resolved and checked at privileged boundaries.
- Codex authentication is detected and invoked in the worker environment; raw
  OAuth credentials are not persisted in Orchestr.
- Structured agent directives such as review decisions and Needs Input are
  accepted only from provider-classified agent-message events. Repository or
  command output cannot impersonate those control messages.
- Integration and revert preserve ordinary Git history rather than rewriting
  the shared target branch.

These checks do not make arbitrary AI-generated code safe. Workers remain
privileged environments and should be configured with the least workspace and
credential access needed for their projects. The working-directory allowlist is
not an operating-system sandbox; a launched program still has the filesystem
permissions of the daemon account.

## Verification strategy

The repository tests the boundaries closest to irreversible workflow changes:

- SQLite state transitions, dependency cycles, readiness recalculation, queue
  claims, locks, health, recovery, collaboration, metrics, and workflow read
  models;
- Git behavior in temporary repositories, including worktrees, conflicts,
  integration, cleanup, and revert;
- local worker streaming and cancellation plus remote protocol behavior;
- provider event parsing and structured control-message handling; and
- TypeScript service adapters, board models, workflow projections, inspectors,
  and confirmation behavior.

The CI workflow runs the TypeScript check, targeted coverage-backed TypeScript
CRAP analysis, Rust coverage-backed CRAP comparison, and a Windows native bundle
build. The Rust quality gate rejects any CRAP regression and new functions
scoring 30 or higher against the checked-in baseline.

This is not complete verification. There is no packaged-desktop end-to-end
suite, frontend coverage is targeted, and CI does not yet enforce formatter,
linter, or Clippy checks. Those gaps are documented rather than hidden behind a
single “tests pass” statement.

## What I would improve next

The next engineering work should reduce risk in the existing control plane
before adding broader distribution:

1. split the Tauri application layer, database crate, and board coordinator into
   smaller domain modules without weakening transaction boundaries;
2. generate or validate TypeScript IPC types from the Rust contract;
3. add packaged-desktop smoke tests for the highest-value lifecycle paths;
4. make local process restart recovery explicit and durable;
5. decide where validation should run for remote work and persist remote job
   state across daemon restarts;
6. separate default and integration branches and make merge strategy explicit;
   and
7. expand release automation beyond unsigned Windows NSIS artifacts when the
   product is ready for wider distribution.

These improvements are deliberately framed as future work. No benchmark,
availability, scale, or production-readiness result is claimed by this case
study.
