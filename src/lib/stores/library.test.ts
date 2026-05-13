import { describe, it, expect, beforeEach, vi } from "vitest";
import { get } from "svelte/store";
import {
  currentLibrary,
  libraryDisplayName,
  recentLibraries,
  rememberLibrary,
  samples,
} from "$lib/stores/library";
import { makeSample, resetIdCounter } from "../../test/fixtures";

beforeEach(() => {
  resetIdCounter();
  currentLibrary.set(null);
  samples.set([]);
  recentLibraries.set([]);
  localStorage.clear();
});

describe("libraryDisplayName", () => {
  it("extracts last path segment from unix path", () => {
    expect(libraryDisplayName("/home/user/Samples/My Library")).toBe("My Library");
  });

  it("extracts last path segment from windows path", () => {
    expect(libraryDisplayName("C:\\Users\\user\\Samples\\My Library")).toBe("My Library");
  });

  it("handles trailing slash", () => {
    expect(libraryDisplayName("/home/user/Samples/")).toBe("Samples");
  });

  it("returns full path if no separator", () => {
    expect(libraryDisplayName("Samples")).toBe("Samples");
  });
});

describe("rememberLibrary", () => {
  it("adds a library to recents", () => {
    rememberLibrary("/path/to/lib");
    const recents = get(recentLibraries);
    expect(recents).toHaveLength(1);
    expect(recents[0]!.path).toBe("/path/to/lib");
    expect(recents[0]!.name).toBe("lib");
  });

  it("moves existing library to front", () => {
    rememberLibrary("/path/first");
    rememberLibrary("/path/second");
    rememberLibrary("/path/first");

    const recents = get(recentLibraries);
    expect(recents).toHaveLength(2);
    expect(recents[0]!.path).toBe("/path/first");
    expect(recents[1]!.path).toBe("/path/second");
  });

  it("limits to 5 recent libraries", () => {
    for (let i = 0; i < 7; i++) {
      rememberLibrary(`/path/lib${i}`);
    }
    expect(get(recentLibraries)).toHaveLength(5);
  });

  it("persists to localStorage", () => {
    rememberLibrary("/path/to/lib");
    const stored = localStorage.getItem("sonoscope.recentLibraries");
    expect(stored).not.toBeNull();
    const parsed = JSON.parse(stored!);
    expect(parsed).toHaveLength(1);
    expect(parsed[0].path).toBe("/path/to/lib");
  });
});

describe("samples store", () => {
  it("starts empty", () => {
    expect(get(samples)).toEqual([]);
  });

  it("holds sample data", () => {
    const data = [makeSample(), makeSample()];
    samples.set(data);
    expect(get(samples)).toHaveLength(2);
  });
});
