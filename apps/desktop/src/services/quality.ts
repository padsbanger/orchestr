import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type ValidationStage = "implementation" | "integration";
export type ValidationStatus = "running" | "passed" | "failed" | "cancelled";
export type ProjectHealthStatus = "unknown" | "healthy" | "degraded" | "broken";

export type ValidationCommand = {
  id: string;
  projectId: string;
  stage: ValidationStage;
  name: string;
  program: string;
  arguments: string[];
  position: number;
  enabled: boolean;
  createdAt: string;
  updatedAt: string;
};

export type ValidationEvent = {
  id: number;
  commandId: string | null;
  kind: string;
  message: string;
  stream: "stdout" | "stderr" | null;
  exitCode: number | null;
  createdAt: string;
};

export type ValidationAttempt = {
  id: string;
  projectId: string;
  taskId: string | null;
  integrationAttemptId: string | null;
  stage: ValidationStage;
  status: ValidationStatus;
  error: string | null;
  startedAt: string;
  completedAt: string | null;
  events: ValidationEvent[];
};

export type ProjectHealth = {
  projectId: string;
  status: ProjectHealthStatus;
  lastValidationAttemptId: string | null;
  lastSuccessfulValidationAt: string | null;
  lastIntegrationAt: string | null;
  failingGate: string | null;
  updatedAt: string;
};

export type ValidationCommandInput = {
  projectId: string;
  stage: ValidationStage;
  name: string;
  program: string;
  arguments: string[];
};

export type ValidationRunEvent = {
  validationAttemptId: string;
  kind: string;
  commandId: string | null;
  stream: "stdout" | "stderr" | null;
  text: string;
  exitCode: number | null;
};

export function listValidationCommands(projectId: string, stage: ValidationStage): Promise<ValidationCommand[]> {
  return invoke<ValidationCommand[]>("list_validation_commands", { projectId, stage });
}

export function createValidationCommand(input: ValidationCommandInput): Promise<ValidationCommand> {
  return invoke<ValidationCommand>("create_validation_command", { input });
}

export function deleteValidationCommand(id: string): Promise<void> {
  return invoke<void>("delete_validation_command", { id });
}

export function listValidationAttempts(projectId: string): Promise<ValidationAttempt[]> {
  return invoke<ValidationAttempt[]>("list_validation_attempts", { projectId });
}

export function getProjectHealth(projectId: string): Promise<ProjectHealth> {
  return invoke<ProjectHealth>("get_project_health", { projectId });
}

export function rerunIntegrationValidation(projectId: string): Promise<ValidationAttempt> {
  return invoke<ValidationAttempt>("rerun_integration_validation", { projectId });
}

export function listenToValidationEvents(handler: (event: ValidationRunEvent) => void): Promise<UnlistenFn> {
  return listen<ValidationRunEvent>("validation://event", ({ payload }) => handler(payload));
}
