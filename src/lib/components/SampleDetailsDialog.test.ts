import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import SampleDetailsDialog from "./SampleDetailsDialog.svelte";
import type { TagDimension } from "$lib/stores/library";
import { makeConflict, makeSample, makeTag } from "../../test/fixtures";

const EDITABLE_DIMENSIONS: TagDimension[] = [
  { name: "Type", value_type: "enum", values: ["loop", "one-shot"] },
  { name: "Instrument", value_type: "multi_enum", values: ["kick", "snare", "drums"] },
  { name: "Tempo", value_type: "numeric", values: [] },
];

function defaultProps() {
  return {
    editableDimensions: EDITABLE_DIMENSIONS,
    onClose: vi.fn(),
    onResolveConflict: vi.fn(),
    onSetTag: vi.fn(),
    onClearTag: vi.fn(),
  };
}

describe("SampleDetailsDialog", () => {
  it("shows ML detections, file metadata, and resolves conflict choices", async () => {
    const props = defaultProps();
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

    render(SampleDetailsDialog, { props: { sample, ...props } });

    expect(screen.getByRole("dialog", { name: "kick_loop.wav" })).toBeInTheDocument();
    expect(screen.getByText("ML Detections")).toBeInTheDocument();
    expect(screen.getByText("Instrument: kick")).toBeInTheDocument();
    expect(screen.getByText("48000 Hz")).toBeInTheDocument();
    expect(screen.getByText("Decisions Needed")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: /one-shot/i }));
    expect(props.onResolveConflict).toHaveBeenCalledWith(42, "Type", "one-shot");

    await fireEvent.click(screen.getByRole("button", { name: "Close sample details" }));
    expect(props.onClose).toHaveBeenCalledTimes(1);
  });

  it("saves a tag for the selected dimension from the edit section", async () => {
    const props = defaultProps();
    const sample = makeSample({
      id: 7,
      filename: "mystery.wav",
      tags: [],
      conflicts: [],
    });

    render(SampleDetailsDialog, { props: { sample, ...props } });

    expect(screen.getByText("Edit Tags")).toBeInTheDocument();

    // The first editable dimension (Type) is preselected with its first value.
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(props.onSetTag).toHaveBeenCalledWith(7, "Type", "loop");

    await fireEvent.click(screen.getByRole("button", { name: "Clear user tag" }));
    expect(props.onClearTag).toHaveBeenCalledWith(7, "Type");
  });

  it("defaults the edit value to the sample's current primary tag", async () => {
    const props = defaultProps();
    const sample = makeSample({
      id: 9,
      tags: [
        makeTag({ dimension: "Type", value: "one-shot", source: "user", is_primary: true }),
      ],
      conflicts: [],
    });

    render(SampleDetailsDialog, { props: { sample, ...props } });

    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(props.onSetTag).toHaveBeenCalledWith(9, "Type", "one-shot");
  });
});
