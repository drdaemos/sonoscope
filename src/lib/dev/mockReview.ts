import type { LibraryMeta, SampleRow, SampleTag, TagDimension } from "$lib/stores/library";
import type { TagConflict } from "$lib/bindings/bindings";

const types = ["loop", "one-shot", "fill", "break", "top-loop", "texture"] as const;
const instruments = [
  "kick",
  "snare",
  "hi-hat",
  "clap",
  "cymbal",
  "percussion",
  "bass",
  "guitar",
  "piano",
  "brass",
  "woodwind",
  "strings",
  "chord",
  "pad",
  "synth",
  "lead",
  "vocal",
  "fx",
  "foley",
] as const;
const keys = ["A", "A#", "B", "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#"] as const;

export function makeMockLibrary(): LibraryMeta {
  return {
    root_path: "E:\\Mock Libraries\\Large Review Set",
    created_at: Date.now(),
    last_discovered_at: Date.now(),
  };
}

export function makeMockTagDimensions(): TagDimension[] {
  return [
    { name: "Type", value_type: "enum", values: [...types] },
    { name: "Instrument", value_type: "multi_enum", values: [...instruments] },
    { name: "Key", value_type: "enum", values: [...keys] },
    { name: "Tempo", value_type: "numeric", values: [] },
    { name: "Mood", value_type: "multi_enum", values: ["dark", "bright", "aggressive", "warm"] },
  ];
}

export function makeMockSamples(count = 1200): SampleRow[] {
  return Array.from({ length: count }, (_, index) => {
    const id = index + 1;
    const type = types[index % types.length]!;
    const instrument = instruments[(index * 5) % instruments.length]!;
    const secondaryInstrument = instruments[(index * 5 + 3) % instruments.length]!;
    const key = keys[(index * 7) % keys.length]!;
    const hasConflict = index % 17 === 0;
    const filename = `${String(id).padStart(4, "0")}_${instrument}_${type}_${96 + (index % 48)}bpm.wav`;

    const tags: SampleTag[] = [
      tag("Type", type, "heuristic", 0.74 + ((index % 20) / 100), true),
      tag("Instrument", instrument, "heuristic", 0.68 + ((index % 25) / 100), true),
      tag("Instrument", secondaryInstrument, "metadata", 0.52, false),
      tag("Key", key, "metadata", 0.8, true),
      tag("Tempo", String(96 + (index % 48)), "metadata", 0.95, true),
    ];

    const conflicts: TagConflict[] = [];
    if (hasConflict) {
      const alternative = type === "loop" ? "one-shot" : "loop";
      const conflictTags = [
        tag("Type", type, "heuristic", 0.72, true),
        tag("Type", alternative, "model", 0.69, false),
      ];
      tags.push(conflictTags[1]!);
      conflicts.push({ dimension: "Type", candidates: conflictTags });
    }

    return {
      id,
      filename,
      relative_path: `${instrument}/${type}/${filename}`,
      format: "wav",
      size_bytes: 80_000 + index * 127,
      duration_ms: 320 + (index % 48) * 125,
      analysis_status: index % 23 === 0 ? "pending" : "done",
      tags,
      conflicts,
    };
  });
}

function tag(
  dimension: string,
  value: string,
  source: SampleTag["source"],
  confidence: number | null,
  isPrimary: boolean,
): SampleTag {
  return {
    dimension,
    value,
    source,
    confidence,
    is_primary: isPrimary,
  };
}
