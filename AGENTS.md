# AGENTS.md

## Project

**Orchestr** is a local-first AI-powered Kanban application for building software projects with human and AI workers.

Each Orchestr project represents a software project backed by an isolated Git repository. Users manage work through a Kanban board, assign tasks to AI agents, and later run those tasks across one or more local or remote worker machines.

The initial product must work well as a normal Kanban + Git project manager before autonomous agent behavior is added.

---

## Product principles

1. **Local-first**
   - The first usable version runs entirely on the user's machine.
   - No cloud account is required for the MVP.
   - Project source code stays in normal Git repositories owned by the user.

2. **Project = Git repository**
   - Every Orchestr project is associated with a Git repository.
   - A project may be created as a new repository, registered from an existing local repository, or later cloned from a remote repository.

3. **Kanban is the source of workflow state**
   - Tasks move through explicit workflow states.
   - Moving a task does not implicitly mutate Git unless the action explicitly requires it.

4. **Human and AI parity**
   - Any important action an agent can perform should also be possible through the UI or core API.
   - Agents must use the same domain/application layer as human actions.
   - Do not hide critical state transitions inside prompts.

5. **Workers own execution environments**
   - Orchestr manages projects, tasks, scheduling, and orchestration.
   - Workers execute commands and provide machine capabilities.
   - The control plane must not assume a specific OS, build system, programming language, or project type.

6. **Provider credentials do not belong to Orchestr**
   - Orchestr must not store raw Codex/ChatGPT OAuth tokens in its database.
   - AI provider login should use the provider's official authentication flow.
   - Authentication is associated with the worker environment where the provider CLI/runtime executes.

7. **Observability over magic**
   - Agent execution should be inspectable.
   - Commands, logs, task state transitions, validation results, commits, and failures should become visible events.

8. **Safe incremental autonomy**
   - Early versions require human review.
   - Workers cannot silently merge or mark their own work complete.
   - Autonomous planning, scheduling, and merging are later milestones.

---

## Primary stack

### Desktop application

- React
- TypeScript
- Vite
- Tauri 2
- SQLite
- `dnd-kit` for Kanban drag and drop
- Radix UI or Base UI primitives
- Lucide icons
- Styling: Tailwind CSS or a similarly lightweight styling layer

### Worker

Preferred implementation:

- Rust
- One cross-platform codebase
- Build targets:
  - Windows x64
  - Linux x64
  - Linux ARM64
  - macOS ARM64
  - macOS x64

Do not create independent Windows/Linux/macOS worker applications. Platform-specific behavior must live behind a small platform abstraction.

---

## Visual direction

Orchestr should feel like an engineering control room rather than a generic SaaS Kanban product.

Design keywords:

- industrial
- dense
- observable
- technical
- restrained

Guidelines:

- dark neutral base
- thin borders
- compact information density
- semantic color for status
- monospace for technical metadata
- subtle animation only when it communicates state

Avoid:

- default unmodified shadcn appearance
- oversized cards
- excessive gradients
- decorative cyberpunk effects
- large empty whitespace typical of marketing SaaS dashboards

Suggested status semantics:

- backlog: neutral
- todo: blue
- in progress: amber
- review: violet
- done: green
- blocked: orange
- failed: red

---

## Core domain model

The exact schema may evolve, but keep the boundaries below.

### Project

```ts
type Project = {
  id: string
  name: string
  description?: string
  defaultBranch: string
  createdAt: string
  updatedAt: string
}
```

Do not permanently model a project as a single machine-specific `repoPath`.

A project may have one or more workspaces.

### Workspace

```ts
type Workspace = {
  id: string
  projectId: string
  workerId: string
  path: string
}
```

A workspace represents a physical checkout of a project on a specific worker.

For the MVP there will normally be one local workspace.

### Task

Initial workflow:

```ts
type TaskStatus =
  | "backlog"
  | "todo"
  | "in_progress"
  | "review"
  | "done"
```

Initial task model:

```ts
type Task = {
  id: string
  projectId: string
  title: string
  description?: string
  status: TaskStatus
  position: number
  createdAt: string
  updatedAt: string
}
```

Later fields may include:

```ts
type TaskExecutionMetadata = {
  acceptanceCriteria: string[]
  dependencies: string[]
  assignedAgentId?: string
  assignedWorkerId?: string
  branch?: string
  worktreePath?: string
  lastRunId?: string
}
```

Do not add all later fields before they are required by a milestone.

### Worker

```ts
type Worker = {
  id: string
  name: string
  os: "windows" | "linux" | "macos"
  architecture: "x64" | "arm64"
  status: "online" | "offline" | "busy"
}
```

Workers expose capabilities separately.

Example capabilities:

```ts
type WorkerCapabilities = {
  tools: Record<string, string | boolean>
  labels: string[]
}
```

Examples of labels:

- `windows`
- `linux`
- `macos`
- `android`
- `ios`
- `xcode`
- `docker`
- `node`
- `python`
- `gpu`
- `godot`

### Agent

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

### Run

Agent execution must eventually be represented explicitly.

```ts
type RunStatus =
  | "queued"
  | "running"
  | "paused"
  | "failed"
  | "completed"
  | "cancelled"
```

Each run should be traceable to:

- task
- agent
- worker
- workspace
- branch/worktree
- start/end timestamps
- logs/events
- validation results
- resulting commits

---

## Application architecture

Keep UI, domain logic, Git operations, worker execution, and provider integrations separate.

Target direction:

```text
React UI
   |
   v
Application / domain layer
   |
   +--> SQLite repositories
   |
   +--> Git service
   |
   +--> Worker client
             |
             v
          Worker
             |
      +------+------+------+
      |      |      |      |
     Git   Codex   npm   Gradle ...
```

Never call shell commands directly from React components.

Bad:

```ts
// UI component
await exec("git status")
```

Good:

```ts
await projectService.getRepositoryStatus(projectId)
```

The service may delegate to a local or remote worker.

---

## Suggested repository structure

This may evolve, but prefer clear boundaries.

```text
orchestr/
├── apps/
│   └── desktop/
│       ├── src/                  # React + TypeScript UI
│       └── src-tauri/            # Tauri host
│
├── crates/
│   ├── orchestr-core/            # domain/application logic
│   ├── orchestr-db/              # SQLite persistence
│   ├── orchestr-git/             # Git abstraction
│   ├── orchestr-worker/          # worker runtime
│   ├── orchestr-protocol/        # worker/control-plane protocol
│   └── orchestr-platform/        # OS-specific worker adapters
│
├── docs/
├── AGENTS.md
└── MILESTONES.md
```

If the first implementation keeps more logic inside `src-tauri`, preserve the same conceptual boundaries even before extracting crates.

---

## Git rules

For the early milestones:

- use the installed `git` executable
- wrap Git operations in a dedicated service
- never scatter Git shell commands throughout the codebase
- validate paths before invoking commands
- never assume the default branch is named `master`
- prefer explicit repository/workspace context

Initial supported operations:

- initialize repository
- validate repository
- current branch
- working-tree status
- latest commit
- basic commit history

Later:

- branches
- worktrees
- diffs
- commit creation
- merge/rebase
- remote operations
- pull requests

---

## Worktree rules

Worktrees are not part of the first MVP.

When introduced:

- one active AI implementation task should normally receive its own branch
- parallel agent tasks must use isolated worktrees
- branch/worktree ownership must be persisted
- failed runs must not automatically delete worktrees
- review must operate on the task branch/diff
- cleanup must be explicit and safe

Suggested naming:

```text
task/TASK-42-short-description
```

Do not hard-code naming assumptions into the UI.

---

## Kanban rules

Initial columns:

1. Backlog
2. Todo
3. In Progress
4. Review
5. Done

The initial board should support:

- create task
- edit task
- delete task
- reorder within a column
- drag between columns
- persistence across app restart

Do not implement configurable columns before the core board is stable.

Important invariant:

> An AI worker must not directly declare its own implementation fully complete.

Expected future workflow:

```text
TODO
  -> IN_PROGRESS
  -> REVIEW
  -> DONE
```

The transition from `REVIEW` to `DONE` is performed by a human or architect/reviewer role.

---

## Persistence

Use SQLite for Orchestr metadata.

The database stores:

- projects
- workspaces
- tasks
- task ordering
- workers
- agents
- runs
- events
- settings

Project source code remains in user-owned Git repositories.

Do not store Git repository contents inside SQLite.

Prefer migrations from the beginning.

---

## Worker architecture

The worker is a general execution runtime, not a Codex-specific service.

It should eventually support:

- execute command
- stream stdout/stderr
- cancellation
- process-tree termination
- working directory
- environment variables
- capability detection
- filesystem operations required by Orchestr
- Git operations
- PTY support when needed

Conceptually:

```rust
trait Platform {
    fn spawn(...);
    fn kill_process_tree(...);
    fn system_info(...);
    fn detect_tools(...);
}
```

OS-specific code should be limited to platform concerns such as:

- paths
- process signals
- shell behavior
- PTY/ConPTY
- service/daemon installation
- tool discovery differences

---

## Worker protocol

The local implementation may initially use Tauri commands directly.

Design the application layer so a remote worker can later use the same conceptual API.

Future protocol direction:

```text
Control Plane
   |
 HTTPS / WebSocket
   |
 Worker
```

Expected concepts:

- register worker
- heartbeat
- capabilities
- create job
- cancel job
- job status
- streamed job events

Possible event types:

```text
job.started
job.completed
job.failed
command.started
command.output
command.completed
file.modified
git.branch.created
git.commit.created
validation.started
validation.failed
validation.passed
```

Do not expose arbitrary unauthenticated remote command execution.

---

## AI provider integration

Provider integration must be behind an abstraction.

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

Start with Codex.

Future providers may include Claude, Gemini, or local runtimes.

Do not hard-code Codex assumptions into the task model.

---

## Codex authentication

Codex authentication belongs to the worker.

For local workers:

- detect whether Codex is installed
- detect authentication status
- allow the user to launch the official Codex login flow
- re-check status after login

For headless/remote workers:

- use the official device/browser authentication mechanism supported by Codex
- surface the login instructions/code in Orchestr
- never ask the user for their ChatGPT password

Do not persist raw OAuth access/refresh tokens in Orchestr's database.

---

## Security

Treat workers as privileged executors.

Rules:

- never execute user-controlled command strings without structured validation where possible
- avoid shell interpolation
- pass command + argument arrays rather than concatenated shell strings
- validate workspace paths
- prevent path traversal outside allowed workspaces for automated operations
- log privileged actions
- remote workers require authenticated/encrypted transport
- secrets must not be written to task logs
- redact known secret patterns where practical
- destructive Git/filesystem actions require explicit application-level intent

The MVP may trust the local user, but architecture should not make remote security impossible later.

---

## Testing

Prefer tests around domain behavior rather than implementation details.

Priority areas:

1. task state transitions
2. task ordering
3. project/workspace persistence
4. Git repository detection
5. command construction
6. worker capability detection
7. run state transitions
8. failure/retry behavior

For UI:

- test important board interactions
- test drag/drop state changes where practical
- avoid brittle screenshot-heavy test suites early

---

## Code quality

- TypeScript strict mode
- Rust warnings addressed
- small focused modules
- explicit error types where useful
- no silent failures
- errors shown to users with actionable context
- keep domain logic outside React components
- keep OS-specific logic outside generic worker code
- avoid premature abstractions that are not connected to a milestone

Prefer clarity over cleverness.

---

## Scope discipline

Do not jump ahead to autonomous multi-agent orchestration while earlier milestones are incomplete.

In particular, the following are deliberately **not MVP features**:

- autonomous task planning
- dynamic task dependencies
- architect agent
- automatic merging
- cloud sync
- teams
- GitHub integration
- remote workers
- multi-provider scheduling
- configurable Kanban workflows

Build the local project + Git + Kanban foundation first.

---

## Definition of a good change

A change is good when it:

- advances the active milestone
- preserves architecture boundaries
- has a clear user-visible purpose
- does not create unnecessary future lock-in
- includes appropriate tests
- does not introduce provider-specific logic into core domain objects
- leaves the project buildable

If a requested change conflicts with these principles, prefer the simplest implementation that preserves the long-term architecture.
