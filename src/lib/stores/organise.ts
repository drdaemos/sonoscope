// Pattern editing helpers for the Organise view. Parsing here mirrors the
// Rust resolver (`src-tauri/src/organise/pattern.rs`) so the UI can validate
// and highlight patterns without a round trip; the backend remains the
// authority when previewing or applying.

import type { OrganiseMode, OrganisePlanEntry } from "$lib/bindings/bindings";

export type PatternToken =
  | { kind: "literal"; text: string }
  | { kind: "placeholder"; name: string; known: boolean };

export type PatternValidation = {
  valid: boolean;
  error: string | null;
  /** Distinct dimension names referenced by the pattern, in order of first use. */
  dimensions: string[];
};

/// Split a pattern into literal and placeholder tokens for highlighting.
/// Malformed trailing input (e.g. an unclosed brace) is kept as a literal
/// token; use validatePattern for correctness.
export function tokenizePattern(
  pattern: string,
  knownDimensions: readonly string[],
): PatternToken[] {
  const tokens: PatternToken[] = [];
  let literal = "";
  let index = 0;

  while (index < pattern.length) {
    const char = pattern[index];
    if (char === "{") {
      const close = pattern.indexOf("}", index + 1);
      if (close === -1) {
        literal += pattern.slice(index);
        break;
      }
      if (literal) {
        tokens.push({ kind: "literal", text: literal });
        literal = "";
      }
      const name = pattern.slice(index + 1, close).trim();
      tokens.push({ kind: "placeholder", name, known: knownDimensions.includes(name) });
      index = close + 1;
    } else {
      literal += char;
      index += 1;
    }
  }

  if (literal) tokens.push({ kind: "literal", text: literal });
  return tokens;
}

export function validatePattern(
  pattern: string,
  knownDimensions: readonly string[],
): PatternValidation {
  const invalid = (error: string): PatternValidation => ({
    valid: false,
    error,
    dimensions: [],
  });

  const normalized = pattern.trim().replaceAll("\\", "/");
  if (!normalized) return invalid("Enter a pattern, e.g. {Type}/{Instrument}");

  const dimensions: string[] = [];
  let depth = 0;
  let placeholder = "";
  let hasSegmentContent = false;

  for (const char of normalized) {
    if (char === "{") {
      if (depth > 0) return invalid("Nested '{' in placeholder");
      depth = 1;
      placeholder = "";
    } else if (char === "}") {
      if (depth === 0) return invalid("Unmatched '}' in pattern");
      depth = 0;
      const name = placeholder.trim();
      if (!name) return invalid("Empty placeholder {}");
      if (!knownDimensions.includes(name)) return invalid(`Unknown dimension: ${name}`);
      if (!dimensions.includes(name)) dimensions.push(name);
      hasSegmentContent = true;
    } else if (depth > 0) {
      placeholder += char;
    } else if (char !== "/") {
      hasSegmentContent = true;
    }
  }

  if (depth > 0) return invalid("Unclosed '{' in pattern");
  if (!hasSegmentContent) return invalid("Pattern contains no folder segments");

  return { valid: true, error: null, dimensions };
}

/// Default name for a saved preset: the dimension order it encodes,
/// e.g. "{Type}/{Instrument}" becomes "Type / Instrument".
export function defaultPresetName(pattern: string): string {
  const names = tokenizePattern(pattern, [])
    .filter((token) => token.kind === "placeholder")
    .map((token) => (token as { name: string }).name)
    .filter((name, index, all) => name && all.indexOf(name) === index);
  return names.length > 0 ? names.join(" / ") : pattern.trim();
}

/// Which preview entries to show: everything, or only the ones carrying a
/// specific flag. "clash" is a destination-path collision (the file would be
/// skipped); "unchanged" only applies in move mode.
export type PreviewFilter = "all" | "untagged" | "clash" | "unchanged";

export function filterPreviewEntries(
  entries: readonly OrganisePlanEntry[],
  filter: PreviewFilter,
  mode: OrganiseMode,
): OrganisePlanEntry[] {
  switch (filter) {
    case "untagged":
      return entries.filter((entry) => entry.untagged);
    case "clash":
      return entries.filter((entry) => entry.conflict);
    case "unchanged":
      return mode === "move" ? entries.filter((entry) => entry.unchanged) : [];
    default:
      return [...entries];
  }
}

export function formatBatchTimestamp(unixSeconds: number): string {
  const date = new Date(unixSeconds * 1000);
  return date.toLocaleString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}
