import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export type ToolCapability = {
  name: string;
  installed: boolean;
  version: string | null;
};

export type WorkerProfile = {
  id: string;
  name: string;
  reportedName: string;
  os: string;
  architecture: string;
  status: "online" | "busy";
  tools: ToolCapability[];
  providers: ProviderStatus[];
  labels: string[];
  maintenance: boolean;
  maxConcurrentRuns: number;
};

export type ProviderStatus = {
  id: string;
  name: string;
  installed: boolean;
  version: string | null;
  authentication: "authenticated" | "unauthenticated" | "unavailable" | "unknown";
  readiness: "ready" | "needs_authentication" | "unavailable" | "unknown";
  detail: string;
};

export type RemoteWorkerWorkspace = {
  projectId: string;
  workspacePath: string;
  enabled: boolean;
};

export type RemoteWorker = {
  id: string;
  name: string;
  reportedName: string;
  endpoint: string;
  tokenEnvironmentVariable: string;
  hasCustomCa: boolean;
  os: string;
  architecture: string;
  status: "online" | "offline";
  protocolVersion: number;
  tools: ToolCapability[];
  providers: ProviderStatus[];
  labels: string[];
  maintenance: boolean;
  maxConcurrentRuns: number;
  workspaces: RemoteWorkerWorkspace[];
  lastSeenAt: string;
};

export type WorkerManagement = {
  workerId: string;
  displayName: string;
  labels: string[];
  maintenance: boolean;
  maxConcurrentRuns: number;
};

export type WorkerRunEvent = {
  runId: string;
  kind: "output" | "completed" | "failed" | "cancelled";
  stream: "stdout" | "stderr" | null;
  text: string | null;
  rawText?: string | null;
  command?: string | null;
  exitCode: number | null;
};

export type LocalWorkerRun = {
  runId: string;
};

export function getLocalWorkerProfile(): Promise<WorkerProfile> {
  return invoke<WorkerProfile>("get_local_worker_profile");
}

export function listRemoteWorkers(): Promise<RemoteWorker[]> {
  return invoke<RemoteWorker[]>("list_remote_workers");
}

export function updateWorkerManagement(input: {
  workerId: string;
  displayName: string;
  labels: string[];
  maintenance: boolean;
  maxConcurrentRuns: number;
}): Promise<WorkerManagement> {
  return invoke<WorkerManagement>("update_worker_management", { input });
}

export function registerRemoteWorker(input: {
  endpoint: string;
  tokenEnvironmentVariable: string;
  caCertificatePath?: string;
  projectId: string;
  workspacePath: string;
}): Promise<RemoteWorker> {
  return invoke<RemoteWorker>("register_remote_worker", { input });
}

export function refreshRemoteWorker(workerId: string): Promise<RemoteWorker> {
  return invoke<RemoteWorker>("refresh_remote_worker", { workerId });
}

export function deleteRemoteWorker(workerId: string): Promise<void> {
  return invoke<void>("delete_remote_worker", { workerId });
}

export function runLocalDiagnostic(): Promise<LocalWorkerRun> {
  return invoke<LocalWorkerRun>("run_local_diagnostic");
}

export function cancelLocalWorkerRun(runId: string): Promise<void> {
  return invoke<void>("cancel_local_worker_run", { runId });
}

export function listenToWorkerRunEvents(handler: (event: WorkerRunEvent) => void) {
  return listen<WorkerRunEvent>("worker://run-event", ({ payload }) => handler(payload));
}
