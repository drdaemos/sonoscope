import { writable } from "svelte/store";
import {
  commands,
  type AnalysisStatus,
  type LibraryMeta,
  type SampleRow,
  type SampleTag,
  type TagDimension,
} from "$lib/bindings/bindings";

export type { AnalysisStatus, LibraryMeta, SampleRow, SampleTag, TagDimension };

export type RecentLibrary = {
  path: string;
  name: string;
  lastOpenedAt: number;
};

const RECENT_LIBRARIES_KEY = "sonoscope.recentLibraries";

export const currentLibrary = writable<LibraryMeta | null>(null);
export const samples = writable<SampleRow[]>([]);
export const tagDimensions = writable<TagDimension[]>([]);
export const discoveryCount = writable<number>(0);
export const isDiscovering = writable<boolean>(false);
export const analysisProcessed = writable<number>(0);
export const analysisTotal = writable<number>(0);
export const isAnalyzing = writable<boolean>(false);
export const recentLibraries = writable<RecentLibrary[]>(loadRecentLibraries());

/// Replace one sample in the store without reloading the whole library.
export function updateSample(updated: SampleRow): void {
  samples.update((items) =>
    items.map((sample) => (sample.id === updated.id ? updated : sample)),
  );
}

/// Reload a single sample after a tag edit. Returns the error string on
/// failure so callers can surface it.
export async function refreshSample(sampleId: number): Promise<string | null> {
  const result = await commands.getSample(sampleId);
  if (result.status === "error") {
    return typeof result.error === "string" ? result.error : JSON.stringify(result.error);
  }
  updateSample(result.data);
  return null;
}

export function libraryDisplayName(rootPath: string): string {
  const normalized = rootPath.replaceAll("\\", "/").replace(/\/$/, "");
  return normalized.split("/").at(-1) ?? rootPath;
}

export function rememberLibrary(path: string): void {
  recentLibraries.update((items) => {
    const next = [
      { path, name: libraryDisplayName(path), lastOpenedAt: Date.now() },
      ...items.filter((item) => item.path !== path),
    ].slice(0, 5);
    saveRecentLibraries(next);
    return next;
  });
}

function loadRecentLibraries(): RecentLibrary[] {
  if (typeof localStorage === "undefined") return [];
  const raw = localStorage.getItem(RECENT_LIBRARIES_KEY);
  if (!raw) return [];

  try {
    const parsed = JSON.parse(raw) as RecentLibrary[];
    return parsed
      .filter((item) => typeof item.path === "string" && typeof item.name === "string")
      .slice(0, 5);
  } catch {
    return [];
  }
}

function saveRecentLibraries(items: RecentLibrary[]): void {
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(RECENT_LIBRARIES_KEY, JSON.stringify(items));
}
