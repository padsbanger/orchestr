# AGENTS.md

## Project

**Orchestr** is a local-first AI-powered Kanban application for building software projects with human and AI workers.

Each Orchestr project represents a software project backed by a Git repository. Tasks are managed through a Kanban workflow and may be executed by AI agents in isolated Git branches/worktrees.

The product is evolving from a local Kanban + Git application into a distributed AI software-development control plane.

---

# Current project state

Completed milestones:

- M0 — Desktop Foundation
- M1 — Projects + Local Git Repositories
- M2 — Core Kanban
- M3 — Repository Awareness
- M4 — Local Worker Runtime
- M5 — Task Specification
- M6 — AI Provider Integration
- M7 — Agents
- M8 — First Agent Task Execution
- M9 — Execution Timeline + Observability
- M10 — Task Branches + Git Worktrees
- M11 — Review Workflow
- M12 — Integration Queue

Immediate next milestone:

**M13 — Quality Gates + Project Health**

Do not skip integration correctness in favor of more agents or more automation.

---

# Primary product objective

Orchestr must optimize for:

> **completed, integrated, healthy project progress**

It must not optimize for:

- number of running agents
- number of completed agent runs
- number of branches
- number of commits
- token usage
- worker utilization

Those are operational metrics, not project progress.

The fundamental project loop is:

```text
PROJECT GOAL
    |
MILESTONE / EPIC
    |
TASK
    |
dependencies satisfied?
  /   \
 no    yes
 |      |
BLOCKED READY
         |
         v
    IN_PROGRESS
         |
implementation validation
         |
         v
       REVIEW
         |
      APPROVED
         |
         v
  INTEGRATION QUEUE
         |
 update against latest integration branch
         |
 integration validation
         |
         v
       MERGE
         |
         v
   MAIN HEALTHY
         |
         v
        DONE
         |
         v
 unblock dependent tasks
```

---

# Product principles

## 1. Local-first

- The desktop application must remain useful without cloud infrastructure.
- Project source code remains in normal Git repositories owned by the user.
- Cloud services are optional extensions, not a requirement for basic use.

## 2. Project = Git repository

Each project is associated with a Git repository.

A project may originate from:

- a new local Git repository
- an existing local Git repository
- later, a cloned remote repository

A project may have multiple physical workspaces across workers.

## 3. Kanban represents workflow state

Kanban state and Git state are related but distinct.

Moving a card must not silently perform unrelated Git operations unless that transition explicitly represents such an operation.

## 4. Human and AI parity

Important actions must exist in the application/domain layer and be usable by both humans and agents.

Examples:

- create task
- edit task
- move task
- assign agent
- start run
- cancel run
- request review
- approve
- request changes
- retry
- integrate
- revert

Do not hide critical workflow transitions only inside prompts.

## 5. Workers own execution environments

Orchestr manages:

- projects
- milestones
- epics
- tasks
- dependencies
- agents
- scheduling
- reviews
- integration
- project progress

Workers manage:

- processes
- Git
- Codex
- build tools
- test tools
- Android tooling
- Xcode
- Docker
- project-specific commands

The control plane must not assume all projects are web applications.

## 6. Provider credentials do not belong to Orchestr

Orchestr must not store raw Codex/ChatGPT OAuth credentials in SQLite.

Provider authentication belongs to the worker environment where that provider executes.

## 7. Observability over magic

Persist and expose useful events such as:

- task became Ready
- task became Blocked
- run started
- process started
- process output
- process completed
- files changed
- validation started
- validation completed
- commit created
- review requested
- review approved
- integration queued
- integration started
- integration conflict
- integration failed
- merge completed
- main health changed
- cleanup completed
- dependent task unblocked

## 8. Safe incremental autonomy

Autonomy is introduced gradually.

- implementing agents do not approve their own work
- approval does not mean Done
- Done requires integration
- integration requires latest-main validation
- automatic scheduling must respect dependencies and downstream capacity

---

# Primary stack

## Desktop

- React
- TypeScript
- Vite
- Tauri 2
- SQLite
- dnd-kit
- Radix UI or Base UI
- Lucide icons
- Tailwind CSS or similarly lightweight styling

## Worker

Preferred:

- Rust
- one cross-platform implementation

Targets:

- Windows x64
- Linux x64
- Linux ARM64
- macOS ARM64
- macOS x64

Platform-specific behavior must remain behind a small adapter layer.

---

# Visual direction

Orchestr should feel like an engineering control room, not a generic SaaS Kanban product.

Keywords:

- industrial
- dense
- observable
- technical
- restrained

Guidelines:

- dark neutral base
- compact cards
- thin borders
- semantic status colors
- monospace for technical metadata
- subtle state-driven animation
- persistent project navigation
- detailed task/run inspector
- project health visible at all times

Suggested semantics:

- Backlog — neutral
- Ready — blue
- In Progress — amber
- Needs Input — yellow
- Review — violet
- Approved — indigo
- Integrating — cyan
- Done — green
- Blocked — orange
- Failed/Broken — red

---

# Core domain concepts

## Project

```ts
type Project = {
  id: string
  name: string
  description?: string
  defaultBranch: string
  integrationBranch: string
  createdAt: string
  updatedAt: string
}
```

Do not hard-code `main` throughout the application.

Use the project's configured integration branch.

## Workspace

```ts
type Workspace = {
  id: string
  projectId: string
  workerId: string
  path: string
}
```

A workspace is a physical checkout of a project on a worker.

## Milestone

A milestone represents a major project outcome.

```ts
type Milestone = {
  id: string
  projectId: string
  title: string
  description?: string
  status: "planned" | "active" | "completed" | "blocked"
  targetDate?: string
}
```

## Epic

An epic groups related tasks inside a milestone.

```ts
type Epic = {
  id: string
  projectId: string
  milestoneId?: string
  title: string
  description?: string
  status: "planned" | "active" | "completed" | "blocked"
}
```

## Task

Recommended workflow statuses:

```ts
type TaskStatus =
  | "backlog"
  | "ready"
  | "in_progress"
  | "needs_input"
  | "review"
  | "approved"
  | "integrating"
  | "blocked"
  | "done"
```

If the existing database currently uses `todo`, migrate incrementally. `ready` should eventually replace ambiguous `todo` semantics for schedulable work.

Suggested task model:

```ts
type Task = {
  id: string
  projectId: string
  milestoneId?: string
  epicId?: string

  title: string
  description?: string

  status: TaskStatus
  priority: "critical" | "high" | "normal" | "low"

  position: number

  createdAt: string
  updatedAt: string
}
```

## Task execution metadata

```ts
type TaskExecutionMetadata = {
  acceptanceCriteria: string[]
  implementationNotes?: string
  relevantPaths?: string[]

  dependencyIds: string[]

  assignedAgentId?: string
  assignedWorkerId?: string

  branch?: string
  worktreePath?: string
  lastRunId?: string
}
```

## Worker

```ts
type Worker = {
  id: string
  name: string
  os: "windows" | "linux" | "macos"
  architecture: "x64" | "arm64"
  status: "online" | "offline" | "busy" | "maintenance"
}
```

## Agent

Agents are configuration, not credentials.

```ts
type Agent = {
  id: string
  name: string
  provider: "codex" | "claude" | "gemini" | "custom"
  role: string
  model?: string
  systemPrompt?: string
  skills: string[]
  maxConcurrentTasks: number
}
```

## Run

```ts
type RunStatus =
  | "queued"
  | "running"
  | "paused"
  | "failed"
  | "completed"
  | "cancelled"
```

Run state and Task state are separate.

Example:

```text
Agent Run: COMPLETED
Task:      REVIEW
Branch:    task/TASK-42
Worktree:  exists
```

Only after successful integration:

```text
Agent Run: COMPLETED
Task:      DONE
Branch:    removed
Worktree:  removed
Main:      contains accepted changes
```

## Integration Attempt

```ts
type IntegrationStatus =
  | "queued"
  | "integrating"
  | "conflict"
  | "validation_failed"
  | "merged"
  | "failed"

type IntegrationAttempt = {
  id: string
  taskId: string
  sourceBranch: string
  targetBranch: string
  status: IntegrationStatus
  startedAt?: string
  completedAt?: string
  error?: string
}
```

## Project Health

Project health is a first-class concept.

```ts
type ProjectHealth =
  | "unknown"
  | "healthy"
  | "degraded"
  | "broken"
```

At minimum, track health of the integration branch.

---

# Task readiness

A task is schedulable only when it is `READY`.

`READY` means:

- acceptance criteria exist
- required dependencies are `DONE`
- required project context exists
- no unresolved project-level blocker prevents execution
- a suitable worker can execute it
- required provider authentication is available when AI execution is requested

The scheduler must not treat all non-started tasks as eligible.

Prefer:

```ts
getReadyTasks(projectId)
```

over:

```ts
getTodoTasks(projectId)
```

---

# Dependency rules

Dependencies determine what can run.

Priority determines what should run first.

A dependency is satisfied only when the dependency task is `DONE`.

Not when:

- its agent run completed
- it reached Review
- it was approved
- it entered Integration

Example:

```text
TASK-10 API
   |
   v
TASK-11 UI
```

TASK-11 remains blocked until TASK-10 is integrated and Done.

When TASK-10 becomes Done, Orchestr should re-evaluate and potentially move TASK-11:

```text
BLOCKED -> READY
```

Dependencies must be cycle-validated.

---

# Priority rules

Initial priority levels:

```text
critical
high
normal
low
```

Scheduling should consider:

1. eligibility
2. dependency satisfaction
3. project health
4. downstream WIP/backpressure
5. priority
6. worker/provider availability

Do not optimize solely for keeping every agent busy.

---

# Needs Input workflow

Agents must be able to ask for human input without guessing.

Example:

```text
IN_PROGRESS
  -> NEEDS_INPUT
  -> IN_PROGRESS
```

Persist:

- question
- requesting agent/run
- timestamp
- answer
- resolution timestamp

`NEEDS_INPUT` should pause active execution unless the implementation can safely continue elsewhere.

---

# Project blockers

Project-level blockers are first-class.

Examples:

- broken integration branch
- unavailable external service
- missing credentials
- unresolved architecture decision
- required SDK unavailable
- project configuration broken

A project blocker may affect multiple tasks.

Do not repeatedly fail many tasks for the same known project-level issue.

---

# Architecture decisions

Important technical decisions should be persisted as project knowledge.

Use an ADR-style concept.

Examples:

```text
ADR-001 Use Tauri
ADR-002 Worker implemented in Rust
ADR-003 Task execution uses Git worktrees
ADR-004 Integration is serialized per project
ADR-005 Done means integrated into integration branch
```

Agents should receive relevant accepted decisions automatically.

Do not allow later agents to casually contradict established architecture without an explicit decision change.

---

# Application architecture

Keep concerns separate:

```text
React UI
   |
   v
Application / Domain Layer
   |
   +--> SQLite
   |
   +--> Git Service
   |
   +--> Worker Client
   |
   +--> Readiness / Dependency Service
   |
   +--> Integration Service
   |
   +--> Project Health Service
             |
             v
           Worker
             |
      Git / Codex / build tools
```

React components must not own raw shell behavior.

Bad:

```ts
await exec("git rebase main")
```

Good:

```ts
await integrationService.integrateTask(taskId)
```

---

# Git rules

Use the installed `git` executable initially.

Wrap Git behavior behind a dedicated service.

Never:

- scatter Git shell commands across UI code
- assume the integration branch is always `main`
- assume one OS path format
- interpolate untrusted values into shell strings
- delete recoverable branches/worktrees before successful integration

Prefer command + argument arrays.

---

# Worktree rules

AI implementation tasks use isolated branches/worktrees.

Typical lifecycle:

```text
READY
  |
start task
  |
create branch
  |
create worktree
  |
run agent
  |
commit
  |
implementation validation
  |
REVIEW
```

Parallel implementation is allowed because worktrees isolate filesystem state.

Suggested branch naming:

```text
task/TASK-42-short-description
```

Do not make UI behavior depend on exact branch naming.

---

# Critical integration invariant

> **A task is DONE only when its accepted changes exist on the project's integration branch and that branch is healthy.**

A task must not be Done merely because:

- the agent finished
- commits exist
- review was approved
- merge command returned successfully but health validation failed

---

# Main / integration branch health invariant

The integration branch must remain healthy.

At minimum:

```text
merge
  |
integration validation
  |
healthy?
 /    \
yes    no
 |      |
DONE   BROKEN / recovery
```

If project health is `broken`:

- do not continue automatic integration
- surface the failure prominently
- preserve integration history
- create/recommend repair work
- avoid scheduling tasks that depend on the broken state when unsafe

Project header should eventually show:

```text
main
HEALTHY

Build      pass
Tests      pass
Typecheck  pass
Last merge TASK-42
```

---

# Integration Queue

Approved tasks enter a serialized integration queue.

Implementation may be parallel.

Integration is serialized per project.

```text
TASK-41 ----\
TASK-42 -----+--> Integration Queue --> integration branch
TASK-43 ----/
```

Only one integration attempt may mutate a project's integration branch at a time.

---

# Integration workflow

Expected lifecycle:

```text
REVIEW
  |
approved
  v
APPROVED
  |
queue
  v
INTEGRATING
  |
  +--> success + healthy --------> DONE
  |
  +--> conflict -----------------> BLOCKED
  |
  +--> validation failure -------> IN_PROGRESS / BLOCKED
  |
  +--> infrastructure failure ---> retryable failure
```

Recommended process:

```text
1. acquire project integration lock
2. verify task branch/worktree exists
3. refresh integration branch
4. update task branch against latest integration branch
5. detect conflicts
6. run integration validation
7. integrate using configured merge strategy
8. verify integration branch health
9. persist integration result
10. cleanup worktree
11. cleanup branch when safe
12. mark task DONE
13. re-evaluate dependent tasks
14. release integration lock
```

---

# Merge strategy

Use one explicit project merge strategy.

Initial recommended default:

**squash merge**

Reason:

AI task branches may contain noisy iterative commits.

Example:

```text
task branch:
- implement
- fix test
- retry
- lint
- final fix

squash ->

main:
TASK-42 Add OAuth callback
```

Orchestr should still retain the original run/commit history in metadata/logs.

Support other strategies later:

- merge commit
- rebase + fast-forward
- PR-based integration

---

# Two-stage validation

## Implementation validation

Runs in the task worktree after implementation.

Question:

> Does the task work on its own branch?

Typical:

```text
agent finishes
  -> lint
  -> typecheck
  -> tests
  -> build
  -> REVIEW
```

## Integration validation

Runs against the task updated onto the latest integration branch.

Question:

> Does it still work with everything that has already landed?

Typical:

```text
APPROVED
  -> update against latest integration branch
  -> validation
  -> merge
  -> health check
```

Passing implementation validation does not imply integration validation will pass.

---

# Conflict handling

Never silently discard changes.

On conflict:

- preserve branch
- preserve worktree
- persist conflicting files
- mark task Blocked
- expose manual resolution
- allow agent-assisted resolution later
- allow retry integration

Example:

```text
TASK-42
Integration blocked

Conflicts:
- src/auth/session.ts
```

---

# Cleanup rules

Cleanup occurs only after successful integration.

Before Done, preserve:

- task branch
- task worktree
- commits
- run history
- review history
- integration history

After successful integration:

- remove task worktree
- remove local task branch when safe
- retain metadata/history

If merge succeeds but cleanup fails, do not pretend integration failed.

Treat cleanup as recoverable maintenance work.

---

# Revert rules

Integrated changes may later prove incorrect.

Support a future explicit Revert action.

Revert must:

- create normal Git history
- not rewrite shared history
- preserve link to original task/integration
- update project health
- optionally create a follow-up repair task

Do not silently reset the integration branch.

---

# Quality gates

Project validation is project-defined.

Example:

```yaml
validation:
  - npm run lint
  - npm run typecheck
  - npm test
  - npm run build
```

Do not assume Node.

Other examples:

```text
./gradlew test
cargo test
pytest
dotnet test
xcodebuild ...
```

Validation output must:

- stream through worker events
- persist result
- attach to Run or Integration Attempt
- distinguish implementation validation from integration validation

---

# WIP limits and backpressure

Orchestr should optimize flow, not maximum concurrency.

Support WIP limits at least conceptually:

```text
In Progress   max 4
Review        max 3
Approved      max 3
Integrating   max 1
```

If downstream stages are congested, the scheduler should stop launching low-priority new work.

Example:

```text
Review queue overloaded
    |
stop starting extra implementation
```

This prevents large amounts of unfinished AI-generated work from accumulating.

---

# Project progress

Project progress must be based primarily on integrated outcomes.

Useful progress signals:

- milestone completion
- epic completion
- Done tasks
- Ready tasks
- Blocked tasks
- critical/high-priority unfinished work
- integration queue length
- project health
- dependency chain progress

Do not present:

- agents running
- commands executed
- tokens consumed

as the primary measure of progress.

Example:

```text
Remote Workers Milestone

17 / 27 tasks Done
4 Ready
3 In Progress
2 Review
1 Blocked

Integration queue: 1
Main: HEALTHY
```

---

# Worker architecture

The Worker is a general execution runtime, not a Codex-specific service.

It should support:

- process execution
- working directory
- environment variables
- stdout/stderr streaming
- cancellation
- process-tree termination
- capability detection
- filesystem operations required by Orchestr
- Git operations
- PTY where needed

Platform-specific differences belong behind an adapter.

---

# AI provider rules

Provider integration stays abstracted.

Conceptually:

```ts
interface AgentProvider {
  id: string
  detect(workerId: string): Promise<ProviderInstallation>
  authStatus(workerId: string): Promise<ProviderAuthStatus>
  login(workerId: string): Promise<LoginFlow>
  logout(workerId: string): Promise<void>
  execute(input: AgentRunInput): Promise<AgentRunHandle>
}
```

Codex is the first provider.

Do not put Codex-specific assumptions into core task/project types.

---

# Codex authentication

Codex authentication belongs to the worker.

Orchestr may:

- detect Codex
- detect version
- detect authentication
- launch official login
- launch device/headless login later
- disconnect
- test readiness

Do not ask for the user's ChatGPT password.

Do not store raw Codex OAuth tokens.

---

# Security

Workers are privileged executors.

Rules:

- validate workspace paths
- avoid shell interpolation
- use command argument arrays
- prevent path traversal
- log privileged operations
- redact secrets where practical
- require authenticated encrypted remote worker transport
- never expose unauthenticated arbitrary command execution
- treat integration and revert operations as privileged actions

---

# Testing priorities

High-priority service-level tests:

1. task state transitions
2. Ready eligibility
3. dependency satisfaction
4. dependency cycle prevention
5. project/workspace persistence
6. task priority ordering
7. worktree ownership
8. run state vs task state
9. Review -> Approved
10. integration queue ordering
11. per-project integration lock
12. stale branch update behavior
13. conflict handling
14. implementation validation
15. integration validation
16. merge strategy
17. project health changes
18. cleanup after integration
19. merge-success/cleanup-failure recovery
20. Done -> dependent task Ready
21. WIP/backpressure scheduling
22. process cancellation
23. worker capability detection

Git/integration tests should use temporary repositories.

---

# Scope discipline

Current next milestone:

**M13 — Quality Gates + Project Health**

Do not prioritize:

- architect agent
- autonomous planning
- remote workers
- cloud sync
- GitHub integration

before accepted work can reliably land on a healthy integration branch.

Immediately after integration correctness, prioritize:

1. quality gates + project health
2. dependencies + Ready/Blocked
3. milestones/epics + project progress
4. architect agent
5. parallel scheduling + WIP/backpressure
6. failure recovery

---

# Definition of a good change

A change is good when it:

- advances the active milestone
- improves integrated project progress
- keeps UI/domain/Git/worker boundaries clear
- preserves history
- remains recoverable under failure
- does not delete recoverable work prematurely
- has useful tests
- keeps provider-specific logic out of core domain types
- leaves the project buildable
- does not trade project health for agent throughput

The near-term system goal is:

```text
Task
  -> Ready
  -> Agent Run
  -> Worktree
  -> Implementation Validation
  -> Review
  -> Approval
  -> Integration Queue
  -> Latest Integration Branch
  -> Integration Validation
  -> Merge
  -> Main Healthy
  -> Cleanup
  -> Done
  -> Unblock Next Work
```
