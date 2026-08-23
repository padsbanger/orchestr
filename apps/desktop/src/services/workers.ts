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
  os: string;
  architecture: string;
  status: "online" | "busy";
  tools: ToolCapability[];
};

export type WorkerRunEvent = {
  runId: string;
  kind: "output" | "completed" | "failed" | "cancelled";
  stream: "stdout" | "stderr" | null;
  text: string | null;
  rawText?: string | null;
  exitCode: number | null;
};

export type LocalWorkerRun = {
  runId: string;
};

export function getLocalWorkerProfile(): Promise<WorkerProfile> {
  return invoke<WorkerProfile>("get_local_worker_profile");
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
