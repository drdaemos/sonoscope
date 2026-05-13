import type { SampleRow, SampleTag, TagConflict } from "$lib/bindings/bindings";

let nextId = 1;

export function makeSample(overrides: Partial<SampleRow> = {}): SampleRow {
  const id = overrides.id ?? nextId++;
  return {
    id,
    filename: `sample_${id}.wav`,
    relative_path: `drums/sample_${id}.wav`,
    format: "wav",
    size_bytes: 102400,
    analysis_status: "done",
    tags: [],
    conflicts: [],
    ...overrides,
  };
}

export function makeTag(overrides: Partial<SampleTag> = {}): SampleTag {
  return {
    dimension: "Type",
    value: "loop",
    source: "heuristic",
    confidence: 0.9,
    is_primary: true,
    ...overrides,
  };
}

export function makeConflict(overrides: Partial<TagConflict> = {}): TagConflict {
  return {
    dimension: "Type",
    candidates: [
      makeTag({ value: "loop", source: "heuristic", confidence: 0.9 }),
      makeTag({ value: "one-shot", source: "model", confidence: 0.7 }),
    ],
    ...overrides,
  };
}

export function resetIdCounter() {
  nextId = 1;
}
