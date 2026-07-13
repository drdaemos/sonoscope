import { describe, expect, it } from "vitest";
import type { OrganisePlanEntry } from "$lib/bindings/bindings";
import {
  defaultPresetName,
  filterPreviewEntries,
  tokenizePattern,
  validatePattern,
} from "./organise";

const KNOWN = ["Type", "Instrument", "Key", "Tempo"];

describe("validatePattern", () => {
  it("accepts placeholder patterns", () => {
    const result = validatePattern("{Type}/{Instrument}", KNOWN);
    expect(result.valid).toBe(true);
    expect(result.error).toBeNull();
    expect(result.dimensions).toEqual(["Type", "Instrument"]);
  });

  it("accepts mixed literal and placeholder segments", () => {
    const result = validatePattern("Sorted/{Type}-{Key}", KNOWN);
    expect(result.valid).toBe(true);
    expect(result.dimensions).toEqual(["Type", "Key"]);
  });

  it("reports duplicate dimensions once", () => {
    const result = validatePattern("{Type}/{Type}", KNOWN);
    expect(result.dimensions).toEqual(["Type"]);
  });

  it("accepts backslash separators", () => {
    expect(validatePattern("{Type}\\{Instrument}", KNOWN).valid).toBe(true);
  });

  it("rejects empty and separator-only patterns", () => {
    expect(validatePattern("", KNOWN).valid).toBe(false);
    expect(validatePattern("   ", KNOWN).valid).toBe(false);
    expect(validatePattern("//", KNOWN).valid).toBe(false);
  });

  it("rejects malformed braces", () => {
    expect(validatePattern("{Type", KNOWN).error).toContain("Unclosed");
    expect(validatePattern("Type}", KNOWN).error).toContain("Unmatched");
    expect(validatePattern("{}", KNOWN).error).toContain("Empty placeholder");
    expect(validatePattern("{Ty{pe}}", KNOWN).error).toContain("Nested");
  });

  it("rejects unknown dimensions", () => {
    expect(validatePattern("{Nonsense}", KNOWN).error).toBe("Unknown dimension: Nonsense");
  });
});

describe("tokenizePattern", () => {
  it("splits literals and placeholders", () => {
    expect(tokenizePattern("{Type}/{Instrument}", KNOWN)).toEqual([
      { kind: "placeholder", name: "Type", known: true },
      { kind: "literal", text: "/" },
      { kind: "placeholder", name: "Instrument", known: true },
    ]);
  });

  it("marks unknown dimensions", () => {
    expect(tokenizePattern("{Nope}", KNOWN)).toEqual([
      { kind: "placeholder", name: "Nope", known: false },
    ]);
  });

  it("keeps an unclosed brace as literal text", () => {
    expect(tokenizePattern("{Type}/{Instr", KNOWN)).toEqual([
      { kind: "placeholder", name: "Type", known: true },
      { kind: "literal", text: "/{Instr" },
    ]);
  });
});

describe("defaultPresetName", () => {
  it("joins dimension order with slashes", () => {
    expect(defaultPresetName("{Type}/{Instrument}")).toBe("Type / Instrument");
    expect(defaultPresetName("{Instrument}/{Type}/{Key}")).toBe("Instrument / Type / Key");
  });

  it("falls back to the raw pattern without placeholders", () => {
    expect(defaultPresetName("  everything  ")).toBe("everything");
  });
});

describe("filterPreviewEntries", () => {
  const entry = (overrides: Partial<OrganisePlanEntry>): OrganisePlanEntry => ({
    sample_id: 1,
    from: "a.wav",
    to: "loop/a.wav",
    untagged: false,
    conflict: false,
    unchanged: false,
    ...overrides,
  });
  const entries = [
    entry({ sample_id: 1 }),
    entry({ sample_id: 2, untagged: true }),
    entry({ sample_id: 3, conflict: true }),
    entry({ sample_id: 4, unchanged: true }),
  ];

  it("returns everything for the all filter", () => {
    expect(filterPreviewEntries(entries, "all", "move")).toHaveLength(4);
  });

  it("filters untagged and clash entries", () => {
    expect(filterPreviewEntries(entries, "untagged", "move").map((e) => e.sample_id)).toEqual([2]);
    expect(filterPreviewEntries(entries, "clash", "move").map((e) => e.sample_id)).toEqual([3]);
  });

  it("filters unchanged entries only in move mode", () => {
    expect(filterPreviewEntries(entries, "unchanged", "move").map((e) => e.sample_id)).toEqual([4]);
    expect(filterPreviewEntries(entries, "unchanged", "copy")).toEqual([]);
  });
});
