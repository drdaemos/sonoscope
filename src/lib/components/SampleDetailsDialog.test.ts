import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import SampleDetailsDialog from "./SampleDetailsDialog.svelte";
import { makeConflict, makeSample, makeTag } from "../../test/fixtures";

describe("SampleDetailsDialog", () => {
  it("shows ML detections, file metadata, and resolves conflict choices", async () => {
    const onClose = vi.fn();
    const onResolveConflict = vi.fn();
    const sample = makeSample({
      id: 42,
      filename: "kick_loop.wav",
      sample_rate: 48000,
      bit_depth: 24,
      channels: 1,
      tags: [
        makeTag({ dimension: "Instrument", value: "kick", source: "model", confidence: 0.82 }),
        makeTag({ dimension: "Type", value: "loop", source: "heuristic", confidence: 0.91 }),
        makeTag({ dimension: "Type", value: "one-shot", source: "model", confidence: 0.63 }),
      ],
      conflicts: [
        makeConflict({
          dimension: "Type",
          candidates: [
            makeTag({ dimension: "Type", value: "loop", source: "heuristic", confidence: 0.91 }),
            makeTag({ dimension: "Type", value: "one-shot", source: "model", confidence: 0.63 }),
          ],
        }),
      ],
    });

    render(SampleDetailsDialog, {
      props: {
        sample,
        onClose,
        onResolveConflict,
      },
    });

    expect(screen.getByRole("dialog", { name: "kick_loop.wav" })).toBeInTheDocument();
    expect(screen.getByText("ML Detections")).toBeInTheDocument();
    expect(screen.getByText("Instrument: kick")).toBeInTheDocument();
    expect(screen.getByText("48000 Hz")).toBeInTheDocument();
    expect(screen.getByText("Decisions Needed")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: /one-shot/i }));
    expect(onResolveConflict).toHaveBeenCalledWith(42, "Type", "one-shot");

    await fireEvent.click(screen.getByRole("button", { name: "Close sample details" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
