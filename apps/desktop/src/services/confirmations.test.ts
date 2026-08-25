import { beforeEach, describe, expect, it, vi } from "vitest";

const { confirmMock } = vi.hoisted(() => ({
  confirmMock: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: confirmMock }));

import { errorMessage, runConfirmedDestructiveAction } from "./confirmations";

const options = {
  title: "Remove project",
  message: "Remove Demo from Orchestr?",
  confirmLabel: "Remove project",
};

describe("destructive action confirmation", () => {
  beforeEach(() => {
    confirmMock.mockReset();
  });

  it("does not run the action when the user cancels", async () => {
    const action = vi.fn();
    confirmMock.mockResolvedValue(false);

    await expect(runConfirmedDestructiveAction(options, action)).resolves.toBe(false);

    expect(confirmMock).toHaveBeenCalledOnce();
    expect(action).not.toHaveBeenCalled();
  });

  it("runs the action exactly once after confirmation", async () => {
    const action = vi.fn().mockResolvedValue(undefined);
    confirmMock.mockResolvedValue(true);

    await expect(runConfirmedDestructiveAction(options, action)).resolves.toBe(true);

    expect(confirmMock).toHaveBeenCalledOnce();
    expect(action).toHaveBeenCalledOnce();
  });

  it("propagates dialog failures without running the action", async () => {
    const action = vi.fn();
    confirmMock.mockRejectedValue(new Error("Dialog unavailable"));

    await expect(runConfirmedDestructiveAction(options, action)).rejects.toThrow("Dialog unavailable");
    expect(action).not.toHaveBeenCalled();
  });
});

describe("errorMessage", () => {
  it("preserves Tauri string errors", () => {
    expect(errorMessage("Remove task worktrees first.", "Unable to remove the project.")).toBe("Remove task worktrees first.");
  });
});
