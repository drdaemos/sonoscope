import { writable } from 'svelte/store';
import type { AnalysisStatus, LibraryMeta, SampleRow } from '$lib/bindings/bindings';

export type { AnalysisStatus, LibraryMeta, SampleRow };

export const currentLibrary = writable<LibraryMeta | null>(null);
export const samples = writable<SampleRow[]>([]);
export const discoveryCount = writable<number>(0);
export const isDiscovering = writable<boolean>(false);
