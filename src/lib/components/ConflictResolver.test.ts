import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import ConflictResolver from "./ConflictResolver.svelte";
import { makeConflict, makeSample } from "../../test/fixtures";

describe("ConflictResolver", () => {
  it("renders conflicts in an overlay panel and resolves a selected candidate", async () => {
    const onResolve = vi.fn();
    const onClose = vi.fn();
    const sample = makeSample({
      id: 42,
      filename: "loop.wav",
      conflicts: [makeConflict()],
    });

    render(ConflictResolver, {
      props: {
        sample,
        onResolve,
        onClose,
      },
    });

    expect(screen.getByText("Resolve tag conflict")).toBeInTheDocument();
    expect(screen.getByText("loop.wav")).toBeInTheDocument();
    expect(screen.getByText("Current: loop")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: /one-shot/i }));
    expect(onResolve).toHaveBeenCalledWith(42, "Type", "one-shot");

    await fireEvent.click(screen.getByRole("button", { name: "Close conflict panel" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
