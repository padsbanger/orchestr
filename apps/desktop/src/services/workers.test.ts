import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

import { deleteRemoteWorker, listRemoteWorkers, refreshRemoteWorker, registerRemoteWorker } from "./workers";

describe("remote worker service", () => {
  beforeEach(() => invokeMock.mockReset());

  it("registers an HTTPS worker without passing a raw token", async () => {
    invokeMock.mockResolvedValue({ id: "worker-1" });
    const input = {
      endpoint: "https://worker.example:9443",
      tokenEnvironmentVariable: "ORCHESTR_WORKER_TOKEN",
      projectId: "project-1",
      workspacePath: "/srv/project",
    };
    await registerRemoteWorker(input);
    expect(invokeMock).toHaveBeenLastCalledWith("register_remote_worker", { input });
    expect(JSON.stringify(invokeMock.mock.calls)).not.toContain("secret-token");
  });

  it("lists, refreshes, and removes registrations", async () => {
    invokeMock.mockResolvedValue([]);
    await listRemoteWorkers();
    expect(invokeMock).toHaveBeenLastCalledWith("list_remote_workers");
    await refreshRemoteWorker("worker-1");
    expect(invokeMock).toHaveBeenLastCalledWith("refresh_remote_worker", { workerId: "worker-1" });
    await deleteRemoteWorker("worker-1");
    expect(invokeMock).toHaveBeenLastCalledWith("delete_remote_worker", { workerId: "worker-1" });
  });
});
