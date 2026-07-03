import { describe, it, expect, beforeEach } from "vitest";
import { get } from "svelte/store";
import { samples, tagDimensions } from "$lib/stores/library";
import {
  clearFilters,
  clearSelection,
  conflictsOnly,
  dimensionFilters,
  dimensionSortKey,
  displayTags,
  displayTagValues,
  filenameSearch,
  filterOptions,
  hasConflict,
  reviewViewportKey,
  selectedSampleIds,
  setSelectionState,
  setSort,
  sortDirection,
  sortKey,
  tagValues,
  toggleFilterValue,
  toggleSelection,
  unanalysedOnly,
  visibleSamples,
} from "$lib/stores/review";
import { makeConflict, makeSample, makeTag, resetIdCounter } from "../../test/fixtures";

beforeEach(() => {
  resetIdCounter();
  samples.set([]);
  tagDimensions.set([
    { name: "Type", value_type: "enum", values: ["loop", "one-shot"] },
    { name: "Instrument", value_type: "multi_enum", values: ["kick", "snare"] },
    { name: "Key", value_type: "enum", values: ["A", "C"] },
    { name: "Mode", value_type: "enum", values: ["major", "minor"] },
  ]);
  clearFilters();
  clearSelection();
  sortKey.set("relative_path");
  sortDirection.set("asc");
});

describe("visibleSamples filtering", () => {
  it("returns all samples when no filters are active", () => {
    const data = [makeSample(), makeSample(), makeSample()];
    samples.set(data);
    expect(get(visibleSamples)).toHaveLength(3);
  });

  it("filters by filename search", () => {
    samples.set([
      makeSample({ filename: "kick_01.wav" }),
      makeSample({ filename: "snare_01.wav" }),
      makeSample({ filename: "kick_02.wav" }),
    ]);
    filenameSearch.set("kick");
    expect(get(visibleSamples)).toHaveLength(2);
    expect(get(visibleSamples).every((s) => s.filename.includes("kick"))).toBe(true);
  });

  it("filters by filename search case-insensitively", () => {
    samples.set([makeSample({ filename: "KICK_loud.wav" })]);
    filenameSearch.set("kick");
    expect(get(visibleSamples)).toHaveLength(1);
  });

  it("filters by dimension", () => {
    samples.set([
      makeSample({ tags: [makeTag({ dimension: "Type", value: "loop" })] }),
      makeSample({ tags: [makeTag({ dimension: "Type", value: "one-shot" })] }),
      makeSample({ tags: [makeTag({ dimension: "Type", value: "loop" })] }),
    ]);
    toggleFilterValue("Type", "loop");
    expect(get(visibleSamples)).toHaveLength(2);
  });

  it("filters conflicts only", () => {
    samples.set([
      makeSample({ conflicts: [makeConflict()] }),
      makeSample({ conflicts: [] }),
    ]);
    conflictsOnly.set(true);
    expect(get(visibleSamples)).toHaveLength(1);
    expect(get(visibleSamples)[0]!.conflicts.length).toBeGreaterThan(0);
  });

  it("filters unanalysed only", () => {
    samples.set([
      makeSample({ analysis_status: "pending" }),
      makeSample({ analysis_status: "done" }),
      makeSample({ analysis_status: "pending" }),
    ]);
    unanalysedOnly.set(true);
    expect(get(visibleSamples)).toHaveLength(2);
  });

  it("combines multiple filters", () => {
    samples.set([
      makeSample({
        filename: "kick_loop.wav",
        tags: [makeTag({ dimension: "Type", value: "loop" })],
      }),
      makeSample({
        filename: "kick_shot.wav",
        tags: [makeTag({ dimension: "Type", value: "one-shot" })],
      }),
      makeSample({
        filename: "snare_loop.wav",
        tags: [makeTag({ dimension: "Type", value: "loop" })],
      }),
    ]);
    filenameSearch.set("kick");
    toggleFilterValue("Type", "loop");
    expect(get(visibleSamples)).toHaveLength(1);
    expect(get(visibleSamples)[0]!.filename).toBe("kick_loop.wav");
  });
});

describe("visibleSamples sorting", () => {
  it("sorts by filename ascending", () => {
    samples.set([
      makeSample({ filename: "charlie.wav", relative_path: "c" }),
      makeSample({ filename: "alpha.wav", relative_path: "a" }),
      makeSample({ filename: "bravo.wav", relative_path: "b" }),
    ]);
    setSort("filename");
    const result = get(visibleSamples).map((s) => s.filename);
    expect(result).toEqual(["alpha.wav", "bravo.wav", "charlie.wav"]);
  });

  it("toggles sort direction on same key", () => {
    samples.set([
      makeSample({ filename: "alpha.wav", relative_path: "a" }),
      makeSample({ filename: "bravo.wav", relative_path: "b" }),
    ]);
    setSort("filename");
    expect(get(sortDirection)).toBe("asc");

    setSort("filename");
    expect(get(sortDirection)).toBe("desc");
    const result = get(visibleSamples).map((s) => s.filename);
    expect(result).toEqual(["bravo.wav", "alpha.wav"]);
  });

  it("resets direction when switching sort key", () => {
    setSort("filename");
    setSort("filename"); // now desc
    expect(get(sortDirection)).toBe("desc");

    const typeSortKey = dimensionSortKey("Type");
    setSort(typeSortKey); // new key resets to asc
    expect(get(sortDirection)).toBe("asc");
    expect(get(sortKey)).toBe(typeSortKey);
  });

  it("sorts by type using tag values", () => {
    samples.set([
      makeSample({
        filename: "a.wav",
        relative_path: "a",
        tags: [makeTag({ dimension: "Type", value: "one-shot" })],
      }),
      makeSample({
        filename: "b.wav",
        relative_path: "b",
        tags: [makeTag({ dimension: "Type", value: "loop" })],
      }),
    ]);
    setSort(dimensionSortKey("Type"));
    const result = get(visibleSamples).map((s) => s.filename);
    expect(result).toEqual(["b.wav", "a.wav"]);
  });
});

describe("filterOptions", () => {
  it("builds dimension options from sample tags", () => {
    samples.set([
      makeSample({ tags: [makeTag({ dimension: "Type", value: "loop" })] }),
      makeSample({ tags: [makeTag({ dimension: "Type", value: "loop" })] }),
      makeSample({ tags: [makeTag({ dimension: "Type", value: "one-shot" })] }),
      makeSample({ tags: [makeTag({ dimension: "Instrument", value: "kick" })] }),
    ]);
    const options = get(filterOptions);
    expect(options.get("Type")?.get("loop")).toBe(2);
    expect(options.get("Type")?.get("one-shot")).toBe(1);
    expect(options.get("Instrument")?.get("kick")).toBe(1);
  });

  it("ignores dimensions outside Type/Instrument/Key", () => {
    samples.set([
      makeSample({ tags: [makeTag({ dimension: "Genre", value: "techno" })] }),
    ]);
    const options = get(filterOptions);
    expect(options.has("Genre")).toBe(false);
  });
});

describe("toggleFilterValue", () => {
  it("adds a filter value", () => {
    toggleFilterValue("Type", "loop");
    expect(get(dimensionFilters)["Type"]).toEqual(["loop"]);
  });

  it("removes a filter value on second toggle", () => {
    toggleFilterValue("Type", "loop");
    toggleFilterValue("Type", "loop");
    expect(get(dimensionFilters)["Type"]).toEqual([]);
  });

  it("supports multiple values per dimension", () => {
    toggleFilterValue("Type", "loop");
    toggleFilterValue("Type", "one-shot");
    expect(get(dimensionFilters)["Type"]).toEqual(["loop", "one-shot"]);
  });
});

describe("clearFilters", () => {
  it("resets all filter state", () => {
    filenameSearch.set("kick");
    toggleFilterValue("Type", "loop");
    conflictsOnly.set(true);
    unanalysedOnly.set(true);

    clearFilters();

    expect(get(filenameSearch)).toBe("");
    expect(get(dimensionFilters)).toEqual({});
    expect(get(conflictsOnly)).toBe(false);
    expect(get(unanalysedOnly)).toBe(false);
  });
});

describe("reviewViewportKey", () => {
  it("changes for review view controls", () => {
    const initialKey = get(reviewViewportKey);
    filenameSearch.set("kick");
    expect(get(reviewViewportKey)).not.toBe(initialKey);
  });

  it("does not change when sample data refreshes without changing review controls", () => {
    const initialKey = get(reviewViewportKey);
    samples.set([makeSample({ filename: "updated.wav" })]);
    expect(get(reviewViewportKey)).toBe(initialKey);
  });
});

describe("selection", () => {
  it("non-additive toggleSelection always results in single item", () => {
    toggleSelection(1, false);
    expect(get(selectedSampleIds).has(1)).toBe(true);
    expect(get(selectedSampleIds).size).toBe(1);
  });

  it("additive toggleSelection removes on second toggle", () => {
    toggleSelection(1, true);
    expect(get(selectedSampleIds).has(1)).toBe(true);

    toggleSelection(1, true);
    expect(get(selectedSampleIds).has(1)).toBe(false);
  });

  it("non-additive toggleSelection replaces selection", () => {
    toggleSelection(1, false);
    toggleSelection(2, false);
    expect(get(selectedSampleIds).has(1)).toBe(false);
    expect(get(selectedSampleIds).has(2)).toBe(true);
  });

  it("additive toggleSelection accumulates", () => {
    toggleSelection(1, true);
    toggleSelection(2, true);
    expect(get(selectedSampleIds).has(1)).toBe(true);
    expect(get(selectedSampleIds).has(2)).toBe(true);
  });

  it("clearSelection empties set", () => {
    toggleSelection(1, true);
    toggleSelection(2, true);
    clearSelection();
    expect(get(selectedSampleIds).size).toBe(0);
  });

  it("setSelectionState selects and deselects without toggling unrelated rows", () => {
    setSelectionState(1, true);
    setSelectionState(2, true);
    setSelectionState(1, true);

    expect([...get(selectedSampleIds)].sort()).toEqual([1, 2]);

    setSelectionState(1, false);
    setSelectionState(3, false);

    expect([...get(selectedSampleIds)]).toEqual([2]);
  });
});

describe("tag helpers", () => {
  it("tagValues extracts values for a dimension", () => {
    const sample = makeSample({
      tags: [
        makeTag({ dimension: "Type", value: "loop" }),
        makeTag({ dimension: "Type", value: "one-shot" }),
        makeTag({ dimension: "Instrument", value: "kick" }),
      ],
    });
    expect(tagValues(sample, "Type")).toEqual(["loop", "one-shot"]);
    expect(tagValues(sample, "Instrument")).toEqual(["kick"]);
  });

  it("displayTags deduplicates and prioritises user > primary > confidence", () => {
    const sample = makeSample({
      tags: [
        makeTag({ dimension: "Type", value: "loop", source: "heuristic", confidence: 0.9, is_primary: false }),
        makeTag({ dimension: "Type", value: "loop", source: "user", confidence: null, is_primary: true }),
        makeTag({ dimension: "Type", value: "one-shot", source: "model", confidence: 0.7, is_primary: false }),
      ],
    });
    const tags = displayTags(sample, "Type");
    expect(tags).toHaveLength(2);
    expect(tags[0]!.source).toBe("user");
    expect(tags[1]!.value).toBe("one-shot");
  });

  it("displayTagValues returns just values", () => {
    const sample = makeSample({
      tags: [makeTag({ dimension: "Type", value: "loop" })],
    });
    expect(displayTagValues(sample, "Type")).toEqual(["loop"]);
  });

  it("hasConflict returns true when conflicts exist", () => {
    expect(hasConflict(makeSample({ conflicts: [makeConflict()] }))).toBe(true);
    expect(hasConflict(makeSample({ conflicts: [] }))).toBe(false);
  });
});
