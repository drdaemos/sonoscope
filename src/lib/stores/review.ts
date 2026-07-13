import { derived, writable } from "svelte/store";
import { samples, tagDimensions, type SampleRow, type SampleTag } from "$lib/stores/library";

export type SortKey = "filename" | "relative_path" | `dimension:${string}`;
export type SortDirection = "asc" | "desc";
export type DimensionFilter = Record<string, string[]>;

/// Sentinel filter value that matches samples with no tag on a dimension.
/// Lets the user isolate files the organise pattern would send to _untagged.
export const UNTAGGED_FILTER_VALUE = "__untagged__";

const sortCollator = new Intl.Collator(undefined, {
  numeric: true,
  sensitivity: "base",
});

export const filenameSearch = writable("");
export const dimensionFilters = writable<DimensionFilter>({});
export const conflictsOnly = writable(false);
export const unanalysedOnly = writable(false);
export const selectedSampleIds = writable<Set<number>>(new Set());
export const sortKey = writable<SortKey>("relative_path");
export const sortDirection = writable<SortDirection>("asc");

export const reviewViewportKey = derived(
  [
    filenameSearch,
    dimensionFilters,
    conflictsOnly,
    unanalysedOnly,
    sortKey,
    sortDirection,
  ],
  ([
    $filenameSearch,
    $dimensionFilters,
    $conflictsOnly,
    $unanalysedOnly,
    $sortKey,
    $sortDirection,
  ]) =>
    JSON.stringify({
      filenameSearch: $filenameSearch,
      dimensionFilters: Object.entries($dimensionFilters)
        .map(([dimension, values]) => [dimension, [...values].sort()] as const)
        .sort(([left], [right]) => left.localeCompare(right)),
      conflictsOnly: $conflictsOnly,
      unanalysedOnly: $unanalysedOnly,
      sortKey: $sortKey,
      sortDirection: $sortDirection,
    }),
);

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
        const matchesUntagged =
          activeValues.includes(UNTAGGED_FILTER_VALUE) && sampleValues.length === 0;
        if (!matchesUntagged && !activeValues.some((value) => sampleValues.includes(value)))
          return false;
      }

      return true;
    });

    // Precompute sort keys once per sample; comparing inside the sort
    // callback would recompute them O(n log n) times.
    const keyed = filtered.map((sample) => ({
      sample,
      key: sortValue(sample, $sortKey),
    }));
    keyed.sort((a, b) => {
      const comparison = sortCollator.compare(a.key, b.key);
      return $sortDirection === "asc" ? comparison : -comparison;
    });
    return keyed.map((entry) => entry.sample);
  },
);

export const filterOptions = derived([samples, tagDimensions], ([$samples, $tagDimensions]) => {
  const filterableDimensions = new Set(
    $tagDimensions
      .filter((dimension) => ["enum", "multi_enum"].includes(dimension.value_type))
      .map((dimension) => dimension.name),
  );
  const options = new Map<string, Map<string, number>>();
  for (const sample of $samples) {
    for (const tag of sample.tags) {
      if (filterableDimensions.size > 0 && !filterableDimensions.has(tag.dimension)) continue;
      const dimensionOptions = options.get(tag.dimension) ?? new Map<string, number>();
      dimensionOptions.set(tag.value, (dimensionOptions.get(tag.value) ?? 0) + 1);
      options.set(tag.dimension, dimensionOptions);
    }
  }
  return options;
});

/// Number of samples with no tag at all on each filterable dimension.
/// Dimensions where every sample is tagged are omitted.
export const untaggedCounts = derived([samples, tagDimensions], ([$samples, $tagDimensions]) => {
  const counts = new Map<string, number>();
  const filterableDimensions = $tagDimensions
    .filter((dimension) => ["enum", "multi_enum"].includes(dimension.value_type))
    .map((dimension) => dimension.name);
  for (const dimension of filterableDimensions) {
    const count = $samples.filter((sample) => tagValues(sample, dimension).length === 0).length;
    if (count > 0) counts.set(dimension, count);
  }
  return counts;
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

export function dimensionSortKey(dimension: string): SortKey {
  return `dimension:${dimension}`;
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

export function setSelectionState(sampleId: number, selected: boolean) {
  selectedSampleIds.update((current) => {
    if (current.has(sampleId) === selected) return current;

    const next = new Set(current);
    if (selected) {
      next.add(sampleId);
    } else {
      next.delete(sampleId);
    }
    return next;
  });
}

export function clearSelection() {
  selectedSampleIds.set(new Set());
}

function sortValue(sample: SampleRow, key: SortKey): string {
  if (key.startsWith("dimension:")) {
    return displayTagValues(sample, key.slice("dimension:".length)).join(", ");
  }

  switch (key) {
    case "filename":
      return sample.filename;
    case "relative_path":
    default:
      return sample.relative_path;
  }
}
