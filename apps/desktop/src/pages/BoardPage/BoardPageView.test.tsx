// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BoardViewSwitch } from "./BoardPageView";

afterEach(cleanup);

describe("BoardViewSwitch", () => {
  it("marks Flow as the default and requests the advanced lifecycle view", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<BoardViewSwitch boardView="flow" onChange={onChange} />);

    expect(screen.getByRole("button", { name: "Flow" }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByRole("button", { name: "Full lifecycle" }).getAttribute("aria-pressed")).toBe("false");

    await user.click(screen.getByRole("button", { name: "Full lifecycle" }));

    expect(onChange).toHaveBeenCalledWith("lifecycle");
  });
});
