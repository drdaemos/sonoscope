import { derived, writable } from "svelte/store";
import { samples, type SampleRow, type SampleTag } from "$lib/stores/library";

export type SortKey = "filename" | "relative_path" | "type" | "instrument";
export type SortDirection = "asc" | "desc";
export type DimensionFilter = Record<string, string[]>;

export const filenameSearch = writable("");
export const dimensionFilters = writable<DimensionFilter>({});
export const conflictsOnly = writable(false);
export const unanalysedOnly = writable(false);
export const selectedSampleIds = writable<Set<number>>(new Set());
export const sortKey = writable<SortKey>("relative_path");
export const sortDirection = writable<SortDirection>("asc");

export const visibleSamples = derived(
  [
    samples,
    filenameSearch,
    dimensionFilters,
    conflictsOnly,
    unanalysedOnly,
    sortKey,
    sortDirection,
  ],
  ([
    $samples,
    $filenameSearch,
    $dimensionFilters,
    $conflictsOnly,
    $unanalysedOnly,
    $sortKey,
    $sortDirection,
  ]) => {
    const search = $filenameSearch.trim().toLowerCase();
    const filtered = $samples.filter((sample) => {
      if (search && !sample.filename.toLowerCase().includes(search)) return false;
      if ($conflictsOnly && !hasConflict(sample)) return false;
      if ($unanalysedOnly && sample.analysis_status !== "pending") return false;

      for (const [dimension, activeValues] of Object.entries($dimensionFilters)) {
        if (activeValues.length === 0) continue;
        const sampleValues = tagValues(sample, dimension);
        if (!activeValues.some((value) => sampleValues.includes(value))) return false;
      }

      return true;
    });

    return [...filtered].sort((a, b) => {
      const left = sortValue(a, $sortKey);
      const right = sortValue(b, $sortKey);
      const comparison = left.localeCompare(right, undefined, {
        numeric: true,
        sensitivity: "base",
      });
      return $sortDirection === "asc" ? comparison : -comparison;
    });
  },
);

export const filterOptions = derived(samples, ($samples) => {
  const options = new Map<string, Map<string, number>>();
  for (const sample of $samples) {
    for (const tag of sample.tags) {
      if (!["Type", "Instrument", "Key"].includes(tag.dimension)) continue;
      const dimensionOptions = options.get(tag.dimension) ?? new Map<string, number>();
      dimensionOptions.set(tag.value, (dimensionOptions.get(tag.value) ?? 0) + 1);
      options.set(tag.dimension, dimensionOptions);
    }
  }
  return options;
});

export function tagValues(sample: SampleRow, dimension: string): string[] {
  return sample.tags.filter((tag) => tag.dimension === dimension).map((tag) => tag.value);
}

export function displayTagValues(sample: SampleRow, dimension: string): string[] {
  return displayTags(sample, dimension).map((tag) => tag.value);
}

export function displayTags(sample: SampleRow, dimension: string): SampleTag[] {
  const seen = new Set<string>();
  return sample.tags
    .filter((tag) => tag.dimension === dimension)
    .sort((a, b) => {
      if (a.is_primary !== b.is_primary) return a.is_primary ? -1 : 1;
      if (a.source === "user" && b.source !== "user") return -1;
      if (a.source !== "user" && b.source === "user") return 1;
      return (b.confidence ?? 0) - (a.confidence ?? 0);
    })
    .filter((tag) => {
      if (seen.has(tag.value)) return false;
      seen.add(tag.value);
      return true;
    });
}

export function hasConflict(sample: SampleRow): boolean {
  return sample.conflicts.length > 0;
}

export function toggleFilterValue(dimension: string, value: string) {
  dimensionFilters.update((filters) => {
    const current = filters[dimension] ?? [];
    const nextValues = current.includes(value)
      ? current.filter((candidate) => candidate !== value)
      : [...current, value];
    return { ...filters, [dimension]: nextValues };
  });
}

export function clearFilters() {
  dimensionFilters.set({});
  filenameSearch.set("");
  conflictsOnly.set(false);
  unanalysedOnly.set(false);
}

export function setSort(nextKey: SortKey) {
  sortKey.update((currentKey) => {
    if (currentKey === nextKey) {
      sortDirection.update((direction) => (direction === "asc" ? "desc" : "asc"));
      return currentKey;
    }
    sortDirection.set("asc");
    return nextKey;
  });
}

export function toggleSelection(sampleId: number, additive: boolean) {
  selectedSampleIds.update((selected) => {
    const next = additive ? new Set(selected) : new Set<number>();
    if (next.has(sampleId)) {
      next.delete(sampleId);
    } else {
      next.add(sampleId);
    }
    return next;
  });
}

export function clearSelection() {
  selectedSampleIds.set(new Set());
}

function sortValue(sample: SampleRow, key: SortKey): string {
  switch (key) {
    case "filename":
      return sample.filename;
    case "type":
      return displayTagValues(sample, "Type").join(", ");
    case "instrument":
      return displayTagValues(sample, "Instrument").join(", ");
    case "relative_path":
    default:
      return sample.relative_path;
  }
}
