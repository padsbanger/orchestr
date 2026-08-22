import { invoke } from "@tauri-apps/api/core";

export type ProviderStatus = {
  id: "codex";
  name: string;
  installed: boolean;
  version: string | null;
  authentication: "authenticated" | "unauthenticated" | "unavailable" | "unknown";
  readiness: "ready" | "needs_authentication" | "unavailable" | "unknown";
  detail: string;
};

export type ProviderRun = { runId: string };

export function getCodexProviderStatus(): Promise<ProviderStatus> {
  return invoke<ProviderStatus>("get_codex_provider_status");
}

export function startCodexLogin(): Promise<ProviderRun> {
  return invoke<ProviderRun>("start_codex_login");
}

export function logoutCodex(): Promise<ProviderRun> {
  return invoke<ProviderRun>("logout_codex");
}

export function testCodexConnection(): Promise<ProviderRun> {
  return invoke<ProviderRun>("test_codex_connection");
}
