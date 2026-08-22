# MILESTONES.md

# Orchestr Roadmap

This roadmap intentionally grows Orchestr from a useful local Kanban + Git application into a distributed AI software-development control plane.

Each milestone should leave the application in a coherent, usable state.

---

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

### Deliverables

- app launches as a desktop application
- SQLite database is initialized automatically
- persistent sidebar/application shell
- basic project dashboard route exists
- development and production builds work

### Definition of done

```text
Launch app
  -> database opens
  -> UI renders
  -> app restart preserves persisted settings
```

---

## M1 — Projects + Local Git Repositories

**Status:** Complete

### Goal

Treat every Orchestr project as a software project backed by Git.

### Scope

Create project from:

- new empty repository
- existing local Git repository

Initial project fields:

- id
- name
- description
- default branch
- timestamps

Initial workspace fields:

- project
- local worker
- repository path

### Git features

- `git init`
- repository validation
- current branch
- working tree status
- latest commit

### UI

Project dashboard:

```text
Projects

Trading Bot
VPS Monitor
Android App

+ New Project
```

Project creation flow:

```text
New project
  -> choose/create directory
  -> initialize Git
  -> persist project/workspace
  -> open project
```

### Definition of done

A user can create a project, close Orchestr, reopen it, and open the same valid Git repository again.

---

## M2 — Core Kanban

**Status:** Complete

**Completed:** 2026-08-22

### Goal

Make Orchestr useful as a normal local Kanban project manager.

### Columns

- Backlog
- Todo
- In Progress
- Review
- Done

### Scope

- create task
- edit task
- delete task
- drag between columns
- reorder within column
- persist ordering
- project-specific boards

### Task fields

- id
- project id
- title
- description
- status
- position
- timestamps

### Definition of done

```text
Create project
  -> create tasks
  -> drag tasks
  -> reorder tasks
  -> restart Orchestr
  -> exact board state is restored
```

---

## M3 — Repository Awareness

**Status:** Complete

**Completed:** 2026-08-22

### Goal

Make Git state visible without turning the product into a full Git client.

### Scope

Project header:

- current branch
- clean/dirty status
- changed file count
- latest commit

Repository view:

- recent commits
- changed files
- basic diff inspection

### Architecture

Introduce/solidify a Git service abstraction.

React components must not execute Git commands directly.

### Definition of done

The board can remain open while users inspect current repository state and recent Git activity.

---

## M4 — Local Worker Runtime

**Status:** Complete

**Completed:** 2026-08-22

### Goal

Introduce the Worker abstraction before AI execution.

Initially the user has one implicit local worker.

### Scope

Worker can:

- report OS and architecture
- report installed tools
- execute a process
- specify working directory
- stream stdout
- stream stderr
- cancel execution
- report exit code

### Capability detection

Initially detect useful tools such as:

- git
- node
- npm
- pnpm
- bun
- docker
- python
- cargo
- java
- gradle
- codex

### UI

```text
Workers

Local Machine    Ready

Windows x64
Git      2.x
Node     24.x
Codex    installed
Docker   installed
```

### Definition of done

Orchestr can run a harmless command through the Worker abstraction and stream its output into the desktop UI.

---

## M5 — Task Specification

**Status:** Complete

**Completed:** 2026-08-22

### Goal

Make tasks precise enough to be handed to AI workers.

### Scope

Add:

- acceptance criteria
- optional implementation notes
- task dependencies field/model preparation
- optional relevant paths/context
- richer task detail panel

Example:

```text
TASK-42
Add GitHub OAuth

Acceptance criteria
- successful callback creates session
- invalid callback shows an error
- authentication tests exist
- build passes
```

### Definition of done

A task contains enough structured information that a developer or AI agent can determine when it is complete.

---

## M6 — AI Provider Integration

**Status:** Complete

**Completed:** 2026-08-22

### Goal

Make AI providers first-class integrations without yet implementing complex scheduling.

Start with Codex.

### Scope

Provider abstraction:

- installation detection
- version detection
- authentication status
- login
- logout
- connection test

### Codex authentication

- use official Codex authentication
- support local browser-based login
- later support device/headless authentication
- Orchestr does not store raw OAuth credentials

### UI

```text
Settings
  AI Providers

Codex
Installed       yes
Authenticated   yes
Status          Ready
```

### Definition of done

A user can install/detect Codex, authenticate through the supported official flow, and Orchestr reports the provider as ready.

---

## M7 — Agents

### Goal

Introduce configurable AI workers as application entities.

### Scope

Agent fields:

- name
- provider
- role
- model
- system instructions
- skills
- concurrency limit

Examples:

```text
Codex Terra
Frontend Engineer

Codex Sol
Architect
```

### Important rule

Agents contain configuration, not authentication credentials.

Authentication belongs to a provider installation on a worker.

### Definition of done

Users can create/edit/delete agent configurations and select an agent for a task.

---

## M8 — First Agent Task Execution

### Goal

Run a task with Codex on the local worker.

### Scope

Workflow:

```text
TODO
  -> assign agent
  -> IN_PROGRESS
  -> execute Codex
  -> REVIEW
```

Persist a Run entity containing:

- task
- agent
- worker
- start time
- end time
- status
- output/events

### UI

Task inspector shows:

- assigned agent
- run status
- runtime
- logs
- cancel action

### Important rule

The worker/agent does not mark its own work `DONE`.

Successful execution moves the task to `REVIEW`.

### Definition of done

A user can assign Codex to a task, watch its output, and receive the resulting task in Review.

---

## M9 — Execution Timeline + Observability

### Goal

Make AI work inspectable.

### Scope

Persist structured events:

- run started
- command started
- command completed
- file modified
- validation started
- validation completed
- commit created
- run failed
- run completed

Example:

```text
15:20:01  Task assigned
15:20:03  Agent started
15:20:17  Read package.json
15:21:42  Modified auth.ts
15:22:13  Added auth.test.ts
15:22:40  npm test
15:22:54  Tests passed
```

### Definition of done

A run can be inspected after it finishes or fails, including after restarting Orchestr.

---

## M10 — Task Branches + Git Worktrees

### Goal

Isolate AI tasks so multiple tasks can safely modify the same project.

### Scope

For eligible tasks:

- create task branch
- create isolated worktree
- persist branch/worktree ownership
- execute agent in the worktree
- preserve failed worktrees for inspection
- cleanup explicitly

Example:

```text
main repo
|
+-- worktrees/
    +-- TASK-42/
    +-- TASK-43/
```

### Definition of done

Two tasks can modify the same project concurrently without sharing a working directory.

---

## M11 — Review Workflow

### Goal

Turn `REVIEW` into a real workflow state.

### Scope

Reviewer can:

- inspect task specification
- inspect diff
- inspect commits
- inspect logs
- approve
- request changes
- return task to In Progress

Workflow:

```text
IN_PROGRESS
  -> REVIEW
      -> DONE

or

IN_PROGRESS
  -> REVIEW
      -> IN_PROGRESS
```

### Definition of done

An AI-produced change can be reviewed and either accepted or returned with feedback without losing execution history.

---

## M12 — Quality Gates

### Goal

Validate work automatically before review.

### Project configuration

Example:

```yaml
validation:
  - npm run lint
  - npm run typecheck
  - npm test
  - npm run build
```

### Scope

- configurable validation commands
- streamed validation output
- pass/fail state
- attach results to run
- failed validation prevents automatic transition to Review
- optionally send failure back to the same agent for repair

### Definition of done

A task cannot enter successful review without satisfying configured project checks.

---

## M13 — Architect / Reviewer Agent

### Goal

Automate technical review without allowing the implementing agent to approve itself.

### Inputs

Architect receives:

- task
- acceptance criteria
- diff
- commits
- validation results
- worker summary

### Outputs

- approve
- request changes
- review notes

### Workflow

```text
Worker Agent
  -> REVIEW
  -> Architect

Architect
  -> approve -> DONE

or

Architect
  -> request changes -> IN_PROGRESS
```

### Definition of done

A separate architect agent can review a task and produce a structured decision with auditable reasoning/notes.

---

## M14 — Dependencies + Blocking

### Goal

Represent task ordering explicitly.

### Scope

- task-to-task dependencies
- blocked status/indicator
- prevent execution of blocked tasks
- automatically unblock when dependencies complete
- dependency visualization

Example:

```text
TASK-10 Database
   |
   +--> TASK-11 API
            |
            +--> TASK-14 UI
```

### Definition of done

The scheduler/UI cannot accidentally start a task whose dependencies are incomplete.

---

## M15 — Parallel Local Agents

### Goal

Run multiple isolated tasks concurrently on one capable machine.

### Scope

- worker concurrency limits
- agent concurrency limits
- queue
- multiple active worktrees
- resource-safe cancellation
- active run overview

### Definition of done

Multiple independent tasks can execute concurrently without corrupting each other's files or run state.

---

## M16 — Failure Recovery

### Goal

Make Orchestr resilient to real agent failures.

### Run states

- queued
- running
- paused
- failed
- retrying
- completed
- cancelled

### Actions

- retry
- resume
- restart clean
- retry with another agent
- inspect worktree
- cancel
- rollback/abandon result
- escalate to human

### Definition of done

Killing Codex or restarting Orchestr does not lose task/run ownership or leave the system unable to recover.

---

## M17 — Remote Worker Protocol

### Goal

Allow Orchestr Desktop to dispatch work to another machine.

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

- worker registration
- authenticated connection
- heartbeat
- capability report
- create job
- stream events
- cancel job
- reconnect behavior

### Security

Remote execution must never be exposed as an unauthenticated command API.

### Definition of done

A task created on Orchestr Desktop can execute on a second computer and stream its run state back to the board.

---

## M18 — Worker Management

### Goal

Support a pool of heterogeneous machines.

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

- worker names
- labels
- capabilities
- online/offline state
- provider authentication state
- concurrency
- maintenance/disabled state

### Definition of done

Users can understand which machines are available and what each machine can execute.

---

## M19 — Capability-Aware Scheduler

### Goal

Select the correct worker automatically.

Example task:

```text
Build Android APK

requires:
  android
  java
  gradle
```

Possible workers:

```text
Windows Desktop    eligible
Linux VPS          not eligible
Mac Mini           eligible
```

### Scope

- task requirements
- worker labels/capabilities
- eligibility filtering
- worker availability
- agent/provider availability
- basic scheduling policy

### Definition of done

A queued task can automatically be assigned only to a worker capable of executing it.

---

## M20 — Project Knowledge

### Goal

Give agents consistent project context.

### Sources

- `AGENTS.md`
- architecture docs
- coding standards
- repository-specific instructions
- reusable skills
- task context

### UI

Expose the context that will be provided to the agent.

### Definition of done

An agent run receives predictable, inspectable project instructions rather than ad hoc prompt construction.

---

## M21 — Planning Agent

### Goal

Turn feature requests into structured tasks.

Example input:

```text
Add GitHub OAuth authentication.
```

Example output:

```text
TASK-101 Configure OAuth
TASK-102 Create callback
TASK-103 Add session handling
TASK-104 Add login UI
TASK-105 Add logout
TASK-106 Add tests
```

### Scope

- feature/epic input
- proposed tasks
- acceptance criteria
- dependency proposal
- human approval before task creation

### Definition of done

The user can describe a feature and approve an AI-generated implementation plan that becomes Kanban tasks.

---

## M22 — Agent Collaboration

### Goal

Allow agents to coordinate through Orchestr rather than unrestricted peer-to-peer chat.

### Scope

Agents may:

- comment on tasks
- create structured requests
- report blockers
- reference another task
- request API/interface changes
- escalate questions

All communication must be persisted.

### Definition of done

One agent can request work/information from another agent through an auditable Orchestr workflow.

---

## M23 — Remote Git Hosting

### Goal

Integrate project repositories with hosted Git platforms.

Start with GitHub.

### Scope

- clone remote repository
- configure remote
- push task branch
- read issues
- issue -> Orchestr task
- create pull request
- inspect pull-request checks/comments
- merge after approval

### Definition of done

An Orchestr task can produce a normal remote Git branch/PR workflow without abandoning the local-first architecture.

---

## M24 — CI/CD Integration

### Goal

Combine local agent validation with external CI.

### Scope

- read CI state
- associate CI runs with tasks/PRs
- prevent completion when required checks fail
- deployment status
- optional deployment task types

### Definition of done

Review can include both local quality gates and remote CI results.

---

## M25 — Metrics + Cost Control

### Goal

Understand how effective the agent system is.

### Metrics

- task count
- success/failure
- retries
- run duration
- token usage where available
- model/provider usage
- cost where available
- worker utilization
- validation failure rate

### Definition of done

Users can compare agents/workers and identify expensive, slow, or unreliable workflows.

---

## M26 — Autonomous Project Mode

### Goal

Allow a high-level goal to flow through planning, implementation, validation, and review.

Target workflow:

```text
User goal
   |
Planner
   |
Architecture / task proposal
   |
Human approval
   |
Scheduler
   |
Workers + agents
   |
Quality gates
   |
Architect review
   |
Merge / completion
```

### Safety requirements

- configurable autonomy level
- task/run audit trail
- clear stop/cancel controls
- no hidden merges
- dependency-aware scheduling
- failure/retry limits
- human escalation

### Definition of done

A user can approve a plan and allow Orchestr to execute multiple dependent tasks with limited supervision while retaining full visibility and control.

---

# Suggested Release Grouping

## v0.1 — Local Kanban + Git

- M0 Desktop Foundation
- M1 Projects + Git
- M2 Core Kanban
- M3 Repository Awareness

Useful even with zero AI.

---

## v0.2 — Local Execution

- M4 Local Worker
- M5 Task Specification
- M6 AI Provider Integration
- M7 Agents

The execution architecture exists before autonomous behavior.

---

## v0.3 — First AI Developer

- M8 First Agent Task Execution
- M9 Execution Timeline
- M10 Task Branches + Worktrees
- M11 Review Workflow

A human can safely delegate one task to Codex.

---

## v0.4 — Reliable AI Workflow

- M12 Quality Gates
- M13 Architect Agent
- M14 Dependencies
- M15 Parallel Local Agents
- M16 Failure Recovery

Orchestr becomes useful for serious multi-task development on one machine.

---

## v0.5 — Distributed Orchestr

- M17 Remote Worker Protocol
- M18 Worker Management
- M19 Capability-Aware Scheduler

Orchestr can use a Windows workstation, Linux VPS, Mac, GPU box, or other suitable development machine.

---

## v0.6 — Project Intelligence

- M20 Project Knowledge
- M21 Planning Agent
- M22 Agent Collaboration

Feature requests can become coordinated implementation plans.

---

## v0.7 — Hosted Development Workflow

- M23 Remote Git Hosting
- M24 CI/CD Integration
- M25 Metrics + Cost Control

Orchestr becomes compatible with normal professional Git/CI workflows.

---

## v1.0 — Autonomous Orchestration

- M26 Autonomous Project Mode

Goal -> plan -> tasks -> workers -> review -> completion.

---

# Immediate implementation target

Start with **v0.1 only**.

The first coding sequence should be:

```text
1. Scaffold React + TypeScript + Vite + Tauri
2. Add SQLite + migrations
3. Create application shell/sidebar
4. Implement Project persistence
5. Implement New Project flow
6. Initialize/register Git repository
7. Implement project dashboard
8. Implement Kanban task persistence
9. Implement task CRUD
10. Implement drag/drop + ordering
11. Add repository status to project header
```

Do not implement Codex, remote workers, worktrees, or autonomous agents until this flow is solid.
