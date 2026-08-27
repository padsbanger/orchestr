import { beforeEach, describe, expect, it, vi } from "vitest";
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
import { createCollaborationEntry, listCollaborationEntries, resolveCollaborationEntry } from "./collaboration";

describe("collaboration service", () => {
  beforeEach(() => invokeMock.mockReset());

  it("lists project collaboration", async () => {
    invokeMock.mockResolvedValue([]);
    await expect(listCollaborationEntries("project-1")).resolves.toEqual([]);
    expect(invokeMock).toHaveBeenCalledWith("list_collaboration_entries", { projectId: "project-1" });
  });

  it("creates typed referenced entries and replies", async () => {
    invokeMock.mockResolvedValue({ id: "entry-1" });
    const input = { projectId: "project-1", taskId: "task-1", parentId: "parent-1", kind: "interface_change" as const, message: "Expose the endpoint.", referencedTaskIds: ["task-2"] };
    await createCollaborationEntry(input);
    expect(invokeMock).toHaveBeenCalledWith("create_collaboration_entry", { input });
  });

  it("resolves a collaboration thread", async () => {
    invokeMock.mockResolvedValue({ id: "entry-1", status: "resolved" });
    await resolveCollaborationEntry("entry-1");
    expect(invokeMock).toHaveBeenCalledWith("resolve_collaboration_entry", { entryId: "entry-1" });
  });
});
