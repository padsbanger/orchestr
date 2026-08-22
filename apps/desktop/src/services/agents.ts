import { invoke } from "@tauri-apps/api/core";

export type Agent = {
  id: string;
  name: string;
  provider: "codex" | "claude" | "gemini" | "custom";
  role: string;
  model: string | null;
  systemPrompt: string | null;
  skills: string[];
  maxConcurrentTasks: number;
  createdAt: string;
  updatedAt: string;
};

export type AgentInput = {
  name: string;
  provider: Agent["provider"];
  role: string;
  model?: string;
  systemPrompt?: string;
  skills: string[];
  maxConcurrentTasks: number;
};

export function listAgents(): Promise<Agent[]> {
  return invoke<Agent[]>("list_agents");
}

export function createAgent(input: AgentInput): Promise<Agent> {
  return invoke<Agent>("create_agent", { input });
}

export function updateAgent(id: string, input: AgentInput): Promise<Agent> {
  return invoke<Agent>("update_agent", { input: { id, ...input } });
}

export function deleteAgent(id: string): Promise<void> {
  return invoke<void>("delete_agent", { id });
}
