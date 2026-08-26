# MILESTONES.md

# Orchestr Roadmap

Orchestr grows from a local Kanban + Git application into a distributed AI software-development control plane.

The roadmap is optimized around one central objective:

> **consistent, integrated, healthy project progress**

Each milestone should leave Orchestr in a coherent and usable state.

---

# Completed Foundation

## M0 — Desktop Foundation — Complete

**Completed:** 2026-08-22

### Goal

Create the basic local desktop application and persistence layer.

### Scope

- React
- TypeScript
- Vite
- Tauri 2
- SQLite
- routing
- application layout
- migrations
- error handling
- basic settings storage

### Definition of done

```text
Launch app
  -> database opens
  -> UI renders
  -> app restart preserves persisted settings
```

---

## M1 — Projects + Local Git Repositories — Complete

### Goal

Treat every Orchestr project as a software project backed by Git.

### Scope

- create new local Git repository
- register existing local Git repository
- persist project
- persist local workspace
- detect default/current branch
- working tree status
- latest commit

### Definition of done

A user can create/register a project, restart Orchestr, and reopen the same Git repository.

---

## M2 — Core Kanban — Complete

**Completed:** 2026-08-22

### Goal

Make Orchestr useful as a local Kanban project manager.

### Scope

- create task
- edit task
- delete task
- drag between columns
- reorder within column
- persist ordering
- project-specific boards

### Definition of done

```text
Create project
  -> create tasks
  -> drag/reorder tasks
  -> restart
  -> exact board state restored
```

---

## M3 — Repository Awareness — Complete

**Completed:** 2026-08-22

### Goal

Expose useful Git state without becoming a full Git client.

### Scope

- current branch
- clean/dirty
- changed file count
- latest commit
- recent commits
- changed files
- basic diff inspection

### Definition of done

Repository state can be inspected while remaining in the project workflow.

---

## M4 — Local Worker Runtime — Complete

**Completed:** 2026-08-22

### Goal

Introduce the Worker abstraction.

### Scope

- OS/architecture detection
- tool detection
- process execution
- working directory
- stdout/stderr streaming
- cancellation
- exit codes

### Definition of done

Orchestr executes commands through the Worker abstraction and streams output into the UI.

---

## M5 — Task Specification — Complete

**Completed:** 2026-08-22

### Goal

Make tasks precise enough for human or AI implementation.

### Scope

- acceptance criteria
- implementation notes
- dependency model preparation
- relevant paths/context
- richer task detail panel

### Definition of done

A task contains enough information to determine whether implementation satisfies the requested outcome.

---

## M6 — AI Provider Integration — Complete

**Completed:** 2026-08-22

### Goal

Make AI providers first-class integrations.

### Scope

Start with Codex:

- installation detection
- version detection
- authentication status
- login
- logout
- readiness test

### Definition of done

Codex can be authenticated and reported as Ready without Orchestr storing raw provider OAuth credentials.

---

## M7 — Agents — Complete

**Completed:** 2026-08-22

### Goal

Introduce configurable AI agents.

### Scope

- name
- provider
- role
- model
- system instructions
- skills
- concurrency limit

### Definition of done

Users can configure agents and assign one to a task.

---

## M8 — First Agent Task Execution — Complete

**Completed:** 2026-08-22

### Goal

Run a Kanban task with Codex.

### Workflow

```text
TODO
  -> assign agent
  -> IN_PROGRESS
  -> execute
  -> REVIEW
```

### Scope

Persist Run data:

- task
- agent
- worker
- timestamps
- status
- output/events

### Definition of done

A user can assign Codex, watch execution, and receive the result in Review.

---

## M9 — Execution Timeline + Observability — Complete

**Completed:** 2026-08-22

### Goal

Make AI execution inspectable.

### Scope

Persist structured run events and make previous runs inspectable after restart.

### Definition of done

Completed and failed runs retain useful execution history.

---

## M10 — Task Branches + Git Worktrees — Complete

**Completed:** 2026-08-23

### Goal

Isolate concurrent AI implementation work.

### Scope

- task branches
- isolated worktrees
- persisted ownership
- execute agent in task worktree
- preserve failed work
- explicit cleanup

### Definition of done

Two tasks can work against the same project concurrently without sharing the same working directory.

---

## M11 — Review Workflow — Complete

**Completed:** 2026-08-23

### Goal

Make Review a real workflow stage.

### Scope

Reviewer can:

- inspect task
- inspect diff
- inspect commits
- inspect logs
- approve
- request changes

### Important correction

Approval no longer means Done.

Approved work must pass through Integration.

---

# Immediate Progress Pipeline

The near-term pipeline is:

```text
Task
  -> Ready
  -> Implementation
  -> Implementation Validation
  -> Review
  -> Approval
  -> Integration Queue
  -> Latest Integration Branch
  -> Integration Validation
  -> Merge
  -> Main Healthy
  -> Done
  -> Unblock Next Tasks
```

---

# M12 — Integration Queue — Complete

**Completed:** 2026-08-23

### Goal

Ensure accepted work safely reaches the project's canonical integration branch.

Initial default: `main`.

### Core invariant

> A task is DONE only when its accepted changes exist on the integration branch and that branch remains healthy.

### Workflow

```text
REVIEW
  -> APPROVED
  -> INTEGRATING
  -> DONE
```

Failure paths:

```text
INTEGRATING
  -> BLOCKED       conflict

INTEGRATING
  -> IN_PROGRESS   implementation issue discovered

INTEGRATING
  -> retryable failure
```

### Integration queue

Implementation may be parallel.

Integration is serialized per project.

```text
TASK-41 ----\
TASK-42 -----+--> Integration Queue --> main
TASK-43 ----/
```

### Scope

- `APPROVED` state
- `INTEGRATING` state
- `BLOCKED` support
- IntegrationAttempt persistence
- per-project integration queue
- per-project integration lock
- refresh integration branch
- update task branch against latest integration branch
- conflict detection
- merge/integrate
- persist result
- cleanup after successful integration
- retry integration
- Integration Queue UI

### Initial merge strategy

Recommended default:

**squash merge**

Keep original branch/run/commit history in Orchestr metadata.

### Cleanup

Before Done preserve:

- branch
- worktree
- commits
- run history
- review history

After successful integration:

- delete worktree
- delete local task branch when safe
- preserve historical metadata

### Definition of done

Multiple implementation tasks may finish independently, but accepted work lands one-by-one on the latest integration branch.

No task becomes Done while its code exists only on an isolated branch.

---

# M13 — Quality Gates + Project Health — Complete

**Completed:** 2026-08-23

### Goal

Make the integration branch reliably green and expose project health.

### Main invariant

> Automatic integration must stop when the integration branch is broken.

### Two-stage validation

#### Implementation validation

```text
agent finishes
  -> validation
  -> REVIEW
```

Question:

> Does this task work on its own branch?

#### Integration validation

```text
APPROVED
  -> update against latest integration branch
  -> validation
  -> merge
```

Question:

> Does this task still work with everything already integrated?

### Project health

Track:

```text
unknown
healthy
degraded
broken
```

Project header should expose:

- integration branch
- health
- last successful validation
- last integration
- failing gate when broken

### Validation configuration

Example:

```yaml
validation:
  - npm run lint
  - npm run typecheck
  - npm test
  - npm run build
```

Must support non-Node projects.

### Scope

- configurable validation commands
- streamed logs
- persisted results
- implementation validation
- integration validation
- integration-branch health state
- stop automatic integration on Broken
- recovery/re-run validation action

### Definition of done

Tasks cannot enter successful Review without required implementation checks, cannot integrate without latest-branch checks, and Orchestr clearly knows whether the canonical branch is healthy.

---

# M14 — Dependencies + Ready / Blocked Workflow — Complete

**Completed:** 2026-08-23

### Goal

Ensure agents only start work that is actually executable.

### New workflow concepts

```text
BACKLOG
  -> READY
  -> IN_PROGRESS
```

Blocked work:

```text
BLOCKED
  -> READY
```

### READY definition

A task is Ready when:

- acceptance criteria exist
- dependencies are Done
- required context exists
- no project blocker prevents execution
- required worker/provider is available when applicable

### Dependencies

Example:

```text
TASK-10 Database
   |
   v
TASK-11 API
   |
   v
TASK-14 UI
```

Dependencies are satisfied only by `DONE`.

Not by:

- run completed
- Review
- Approved
- Integrating

### Scope

- dependency persistence
- dependency cycle validation
- Ready eligibility calculation
- Blocked reason
- automatic re-evaluation
- Done -> unblock dependent tasks
- priority field

Priority:

```text
critical
high
normal
low
```

### Definition of done

The scheduler/UI can distinguish:

- work that exists
- work that is Ready
- work that is Blocked

Completing and integrating one task can automatically make the next task Ready.

---

# M15 — Milestones, Epics + Project Progress — Complete

**Completed:** 2026-08-23

### Goal

Measure whether the project is moving toward meaningful outcomes.

### Hierarchy

```text
Project
  |
Milestone
  |
Epic
  |
Task
```

### Scope

Milestones:

- title
- description
- status
- optional target date

Epics:

- title
- description
- milestone link
- status

Tasks may belong to:

- milestone
- epic

### Progress dashboard

Prefer outcome metrics:

```text
Remote Workers Milestone

17 / 27 Done
4 Ready
3 In Progress
2 Review
1 Blocked

Integration Queue: 1
Main: HEALTHY
```

Avoid using agent activity as primary progress:

```text
12 agents running
4.5M tokens
183 commands
```

Those remain secondary operational metrics.

### Definition of done

The user can answer:

- what milestone is active?
- what outcome is being built?
- what is Done?
- what is Ready next?
- what is Blocked?
- is main healthy?

---

# M16 — Architect / Reviewer Agent — Complete

**Completed:** 2026-08-23

### Goal

Automate technical review without self-approval.

### Inputs

Architect receives:

- task
- acceptance criteria
- relevant project decisions
- diff
- commits
- implementation validation
- run summary

### Outputs

- approve
- request changes
- review notes

### Workflow

```text
Worker Agent
  -> REVIEW
  -> Architect
  -> APPROVED
  -> Integration Queue
```

or:

```text
Architect
  -> request changes
  -> IN_PROGRESS
```

### Definition of done

A logically separate reviewer agent can approve or reject work, but approved work still goes through normal integration.

---

# M17 — Parallel Local Agents + WIP Limits — Complete

**Completed:** 2026-08-25

### Goal

Increase throughput without creating excessive unfinished work.

### Scope

- worker concurrency limits
- agent concurrency limits
- execution queue
- multiple worktrees
- active runs
- WIP limits
- downstream backpressure

Example:

```text
In Progress   max 4
Review        max 3
Approved      max 2
Integrating   max 1
```

### Scheduler principle

Do not start more implementation simply because an agent is idle.

If Review or Integration is congested, reduce new starts.

### Definition of done

Multiple agents can work concurrently while Orchestr prevents downstream queues from growing without control.

---

# M18 — Failure Recovery + Revert — Complete

**Completed:** 2026-08-25

### Goal

Recover safely from agent, process, Git, integration, cleanup, or regression failures.

### Run recovery

- retry
- resume
- restart clean
- retry with another agent
- inspect worktree
- cancel
- abandon
- escalate

### Integration recovery

- retry integration
- recover stale integration lock
- preserve conflict state
- resume after Orchestr restart
- handle merge-success/cleanup-failure

### Revert

For already-integrated regressions:

- create normal Git revert
- link revert to original task/integration
- update project health
- optionally create repair task

### Definition of done

Crashes and bad changes are recoverable without losing traceability or rewriting shared history.

---

# M19 — Project Blockers + Needs Input — Complete

**Completed:** 2026-08-25

### Goal

Prevent agents from repeatedly guessing or failing on unresolved human/project issues.

### Needs Input

```text
IN_PROGRESS
  -> NEEDS_INPUT
  -> IN_PROGRESS
```

Persist:

- question
- answer
- requesting run/agent
- timestamps

### Project blockers

Examples:

- missing credentials
- SDK unavailable
- broken main
- external API unavailable
- unresolved product decision

### Scope

- task-level Needs Input
- project-level blocker records
- blocker affected tasks
- pause/suppress unsafe automatic scheduling
- resume affected tasks when blocker resolved

### Definition of done

Orchestr can stop and ask rather than allowing many agents to repeatedly fail or invent decisions.

---

# M20 — Architecture Decisions / Project Knowledge — Complete

**Completed:** 2026-08-26

### Goal

Give agents durable project memory and prevent architectural drift.

### Sources

- `AGENTS.md`
- ADRs
- architecture docs
- coding standards
- repository instructions
- reusable skills
- task context

### ADR examples

```text
ADR-001 Use Tauri
ADR-002 Worker is Rust
ADR-003 Tasks use worktrees
ADR-004 Integration is serialized
```

### Scope

- accepted architecture decisions
- superseded decisions
- relevant context selection
- context preview in UI
- inject relevant knowledge into agent runs

### Definition of done

Agents receive consistent, inspectable project knowledge and do not casually contradict accepted architectural decisions.

---

# M21 — Remote Worker Protocol — Complete

**Completed:** 2026-08-26

### Goal

Dispatch work to another machine.

### Architecture

```text
Orchestr Desktop
      |
 authenticated
 HTTPS/WebSocket
      |
Remote Worker
```

### Scope

- registration
- authentication
- heartbeat
- capabilities
- create job
- stream events
- cancel
- reconnect

### Definition of done

A task can execute on another computer while state remains visible in Orchestr.

---

# M22 — Worker Management — Complete

**Completed:** 2026-08-26

### Goal

Support heterogeneous worker machines.

Example:

```text
DEV-PC
Windows
android, docker, gpu

VPS-01
Linux
docker, node, python

MAC-MINI
macOS
xcode, ios
```

### Scope

- names
- labels
- capabilities
- online/offline
- provider authentication state
- concurrency
- maintenance state

### Definition of done

Users can understand which machines are available and what each can execute.

---

# M23 — Capability-Aware Scheduler — Complete

### Goal

Automatically choose valid work + valid workers.

### Scheduling inputs

- task Ready state
- priority
- dependencies
- worker capability
- agent/provider readiness
- project health
- WIP limits
- downstream congestion

Example:

```text
Build Android APK

requires:
  android
  java
  gradle
```

### Definition of done

Orchestr selects only Ready work and dispatches it only to capable workers without overwhelming downstream stages.

### Delivered

- task specifications persist explicit required worker capabilities
- capabilities match installed tools, normalized worker labels, OS, and architecture
- manual and project-level scheduling share provider-ready worker selection
- Ready work is ordered by priority and excludes tasks with active runs
- agent, worker, project WIP, health, blocker, and downstream limits gate dispatch
- persisted scheduler decisions explain every scheduled, skipped, or blocked task
- Flow exposes a Schedule Ready work control and recent decision history

---

# M24 — Planning Agent — Complete

**Completed:** 2026-08-26

### Goal

Turn project goals/features into structured work.

Example:

```text
Add GitHub OAuth authentication
```

Planner proposes:

```text
Epic: GitHub Authentication

TASK-101 Configure OAuth
TASK-102 Create callback
TASK-103 Add session handling
TASK-104 Add login UI
TASK-105 Add logout
TASK-106 Add tests
```

### Scope

- milestone/epic proposal
- task proposal
- acceptance criteria
- dependency proposal
- priority proposal
- human approval

### Definition of done

A user can approve an AI-generated implementation plan that becomes structured, dependency-aware project work.

### Delivered

- read-only Codex planning against repository context, existing work, and accepted ADRs
- durable proposal lifecycle with raw transcript, structured plan, failure, cancellation, and human decision history
- validated milestone, epic, task, acceptance-criteria, capability, priority, and dependency proposals
- rejection of missing, self-referential, unknown, or cyclic task dependencies
- atomic human approval that materializes the full hierarchy or none of it
- right-hand planning workspace with live state, transcript, proposal inspection, approve, reject, and cancel actions

---

# M25 — Agent Collaboration

### Goal

Allow agents to coordinate through auditable Orchestr workflows.

### Scope

Agents may:

- comment
- create requests
- report blockers
- reference tasks
- request interface changes
- escalate questions

All communication is persisted.

### Definition of done

Agents can coordinate without hidden peer-to-peer conversation.

---

# M26 — Remote Git Hosting

### Goal

Integrate with hosted Git providers.

Start with GitHub.

### Scope

- clone
- remotes
- push task branch
- issues
- issue -> task
- PR creation
- checks/comments
- merge workflow

### Integration modes

Later support:

- local integration
- PR-based integration
- hybrid integration

### Definition of done

An Orchestr task can participate in a normal hosted Git workflow without abandoning local-first behavior.

---

# M27 — CI/CD Integration

### Goal

Combine local validation with external CI/deployment state.

### Scope

- read CI status
- associate CI with task/PR
- required checks
- deployment state
- deployment task types

### Definition of done

Project progress can include both local validation and remote CI outcomes.

---

# M28 — Metrics + Cost Control

### Goal

Measure reliability, cost, and bottlenecks.

### Operational metrics

- run count
- success/failure
- retries
- duration
- token usage
- provider/model usage
- cost
- worker utilization

### Flow metrics

- Ready lead time
- In Progress duration
- Review queue time
- Integration queue time
- conflict rate
- blocked time
- validation failure rate
- milestone throughput

### Important rule

Operational activity is not project progress.

### Definition of done

Users can identify bottlenecks, unreliable agents, excessive cost, or slow project flow.

---

# M29 — Autonomous Project Mode

### Goal

Allow a project goal to move through planning, implementation, integration, and completion with limited supervision.

### Target workflow

```text
Project Goal
   |
Planner
   |
Milestone / Epic / Tasks
   |
Human approval
   |
Ready Queue
   |
Scheduler
   |
Workers + Agents
   |
Implementation Validation
   |
Review / Architect
   |
Integration Queue
   |
Integration Validation
   |
Merge
   |
Main Healthy
   |
Done
   |
Unblock Next Tasks
```

### Safety requirements

- configurable autonomy
- audit trail
- stop/cancel controls
- no hidden merges
- dependencies
- WIP limits
- project-health gates
- retry limits
- conflict handling
- human escalation

### Definition of done

A user can approve a plan and allow Orchestr to make sustained project progress while preserving control, health, and traceability.

---

# Updated Release Grouping

## v0.1 — Local Kanban + Git — Complete

- M0 Desktop Foundation
- M1 Projects + Git
- M2 Core Kanban
- M3 Repository Awareness

---

## v0.2 — Local Execution — Complete

- M4 Local Worker
- M5 Task Specification
- M6 AI Provider Integration
- M7 Agents

---

## v0.3 — First AI Developer

- M8 Agent Task Execution — Complete
- M9 Execution Timeline — Complete
- M10 Worktrees — Complete
- M11 Review Workflow — Complete
- M12 Integration Queue — Complete
- M13 Quality Gates + Project Health — Complete

v0.3 is complete when accepted AI work reliably lands on a healthy integration branch.

---

## v0.4 — Consistent Project Progress

- M14 Dependencies + Ready/Blocked
- M15 Milestones/Epics + Progress
- M16 Architect Agent
- M17 Parallel Agents + WIP Limits
- M18 Failure Recovery + Revert
- M19 Project Blockers + Needs Input
- M20 Architecture Decisions / Project Knowledge

This release turns Orchestr from an agent runner into a system that manages project flow.

---

## v0.5 — Distributed Orchestr

- M21 Remote Worker Protocol
- M22 Worker Management
- M23 Capability-Aware Scheduler

---

## v0.6 — Project Intelligence

- M24 Planning Agent
- M25 Agent Collaboration

---

## v0.7 — Hosted Development Workflow

- M26 Remote Git Hosting
- M27 CI/CD Integration
- M28 Metrics + Cost Control

---

## v1.0 — Autonomous Orchestration

- M29 Autonomous Project Mode

---

# Immediate Implementation Sequence

Continue from the completed M24 state:

```text
1. Implement persisted, auditable agent comments, requests, blockers, references, and escalations
```

Do not let later agents casually contradict accepted technical decisions.

The immediate objective is:

```text
accepted task
  -> safely integrated
  -> main remains healthy
  -> blockers, input needs, and accepted decisions remain explicit
  -> next valid task becomes Ready
```

That loop is the foundation for consistent autonomous project progress.
