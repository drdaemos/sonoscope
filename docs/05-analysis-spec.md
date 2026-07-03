# Sonoscope — Analysis Pipeline Spec

## Overview

The analysis pipeline takes a file path and returns a set of tag candidates with source labels and confidence scores. It runs as a separate process (sidecar) invoked by Core after discovery. Three stages run sequentially per file; their results are merged before being returned.

```
file path
    │
    ▼
┌─────────────────────┐
│ Stage 1             │  format, duration, sample rate,
│ Metadata extraction │  bit depth, channels, embedded tags
└────────┬────────────┘
         │
         ▼
┌─────────────────────┐
│ Stage 2             │  Type, Instrument, Tempo, Key
│ Filename heuristics │  (from path tokens)
└────────┬────────────┘
         │
         ▼
┌─────────────────────┐
│ Stage 3             │  Type, Instrument, Tempo, Key
│ ML audio classifier │  (from audio content)
└────────┬────────────┘
         │
         ▼
    merged tag list
    (source + confidence per tag)
```

---

## ADR-1 Resolution — Language: Python

**Decision: Python sidecar.**

Evaluation of the analysis requirements against available libraries:

| Capability | Python | Rust |
|---|---|---|
| Audio metadata | `mutagen`, `soundfile` — complete | `symphonia` — capable |
| Filename heuristics | trivial in any language | trivial in any language |
| Loop/one-shot detection | `essentia` pre-trained model | no equivalent |
| Instrument classification | LAION CLAP via `transformers`, `essentia`, YAMNet | ONNX only, no pre-trained audio models |
| BPM / beat tracking | `essentia`, `librosa` | `aubio` bindings (limited) |
| Key detection | `essentia`, `librosa` | none |

The gap is decisive at Stage 3. Rust has no production-ready pre-trained models for loop detection or instrument classification. The only Rust path for ML inference would be exporting a Python-trained model to ONNX and running it via `tract` — which still requires Python for model development and retraining, adds an export step, and provides no deployment benefit given we are already bundling a sidecar.

**Python is chosen for the full pipeline** (all three stages), running as a long-lived sidecar process.

---

## Stage 1 — Metadata extraction

**Library:** `mutagen` (tag reading) + `soundfile` / `audioread` (format info)

**Current implementation decision:** Phase 3 uses `mutagen` for embedded tags and `soundfile` for technical audio properties. Sonoscope source remains MIT; bundled distributions must preserve dependency license notices, including Mutagen's GPL-2.0-or-later license. ML/DSP libraries listed later in this document are provisional and require a separate packaging/platform evaluation before adoption.

Extracts:
- Format, sample rate, bit depth, channels, duration
- Embedded text tags: title, comment, genre, BPM, key (if present)

Embedded tags are mapped to dimensions where a clear mapping exists:

| Embedded field | Dimension | Notes |
|---|---|---|
| BPM / TBPM | Tempo | parsed as float |
| KEY / TKEY | Key + Mode | normalised chromatic note name plus major/minor when present |
| GENRE | Instrument | low confidence, fuzzy match against instrument values |
| COMMENT | — | stored as-is for heuristic stage to process |

Embedded tags that map cleanly (BPM, Key, Mode) are treated as `metadata` source with confidence 0.95. Genre-based instrument guesses use confidence 0.4.

---

## Source policy by detected field

Analysis uses the most explainable source available for each field. ML is used where a model is actually trained for the question being asked; deterministic metadata and signal descriptors take precedence when they are more reliable.

| Field | Primary method | Secondary method | ML use | Emission rule |
|---|---|---|---|---|
| File metadata | Container/header parsing with `soundfile` and Mutagen | none | none | Always emit file metadata when readable. |
| Waveform | Peak bins from decoded PCM | none | none | Always emit when decode succeeds. |
| Type: loop | Filename/path tokens (`loop`, `lp`, tempo-in-name) and embedded library metadata | Essentia/Freesound loop descriptors (`bpm_loop_confidence`, beat/onset regularity) when available | Only a real binary loop/non-loop model may emit standalone ML Type. The Freesound Loop Dataset role model must not be used as loop detection. | Emit only from explicit evidence; leave empty if uncertain. |
| Type: one-shot | Filename/path tokens (`oneshot`, `one_shot`, `1shot`, etc.) | Short-duration plus envelope/onset shape descriptors may provide low-confidence evidence only after validation | No fixed fallback. No one-shot tag from a loop-role model. | Emit only from explicit filename/metadata or a validated classifier; leave empty if uncertain. |
| Instrument | Filename/path tokens | CLAP prompt scoring over shipped Instrument values | Optional Essentia/Jamendo/loop-role models may add supporting instrument evidence, but only for their trained labels. | Emit top candidates above threshold; show conflicts instead of suppressing evidence. |
| Tempo | Embedded BPM/TBPM and filename BPM | Essentia `RhythmExtractor2013` / loop BPM descriptors or `librosa.beat.beat_track` fallback | none | Prefer metadata/filename; emit audio-estimated BPM only with confidence and sane range. |
| Key | Embedded KEY/TKEY and filename key | Essentia `KeyExtractor` / HPCP key strength | none | Emit only when strength/parse confidence clears threshold. |
| Mode | Embedded/filename major-minor suffix | Essentia key scale when key strength clears threshold | CLAP mode prompts are weak supporting evidence only and capped top-1. | Prefer deterministic/tonal extraction; do not infer mode from mood words alone. |
| Mood | none yet | optional curated filename tokens | CLAP/Jamendo mood models only if exposed as separate low-trust candidates | Do not emit by default until UI can present weak semantic evidence clearly. |

Default behavior should require no user-supplied paths. The app downloads and caches supported model files from known sources; the analyzer auto-enables a backend only when both its Python package and model files are present. Missing optional backends must degrade to deterministic metadata/heuristics without inventing substitute tags.

---

## Stage 2 — Filename and path heuristics

Pattern matching against the file's basename and its parent folder names. Runs against the full relative path (e.g. `Drums/Kicks/punchy_kick_909_120bpm.wav`).

**Library:** stdlib `re` — no external dependency needed.

### Type detection

| Pattern | Tag | Confidence |
|---|---|---|
| `loop`, `lp`, `_l_` | Type: loop | 0.95 |
| `oneshot`, `one_shot`, `one-shot`, `1shot` | Type: one-shot | 0.95 |
| `\d{2,3}bpm`, `\d{2,3}_bpm` | Type: loop | 0.7 (tempo implies loop) |

No Type fallback is emitted. If neither filename heuristics nor ML classification produces a Type candidate, the analysis response leaves Type empty.

### Instrument detection

Token list matched against dimension_values for the Instrument dimension. Matching is case-insensitive, whole-word.

Examples: `kick`, `kik`, `bd`, `bass drum` → Instrument: kick (0.9); `snare`, `snr`, `sd` → Instrument: snare (0.9); `808` → Instrument: bass (0.75); `hat`, `hh`, `hihat` → Instrument: hi-hat (0.9); etc.

The full token list is maintained in a heuristic config file (JSON) shipped with the sidecar, not hardcoded. This makes it easy to extend without changing code.

### Tempo detection

`(\d{2,3})\s?bpm` → Tempo: extracted value (0.95)

### Key detection

`[A-G][b#]?\s?(maj|min|major|minor|m)?` → Key: normalised pitch class; when a mode suffix is present also emits Mode: `major` or `minor` (0.85 if both note and mode are present, 0.65 if note only)

---

## Stage 3 — ML audio classification

**Primary libraries:** CLAP via PyTorch for broad semantic tags; Essentia/librosa-style MIR descriptors for tempo, key, and loopability evidence.

Essentia is useful for deterministic MIR descriptors and some high-level models, but available model training scope matters. The Freesound Loop Dataset model classifies the instrument role of known loops; it is not a loop-vs-one-shot classifier and must not be used as one.

### 3.1 Loop vs. one-shot (Type dimension)

**Model:** `essentia-tensorflow` — DiscogEFNet or the dedicated loop/one-shot classifier

Produces: `loop` or `one-shot` with a probability score → confidence.

Fallback: none. If Essentia's loop model is unavailable, no ML Type tag is emitted.

### 3.2 Instrument classification (Instrument dimension)

**Model:** LAION CLAP (`laion/larger_clap_music`) via Hugging Face `transformers`.

CLAP scores the audio against a hand-maintained set of text prompts. A static prompt mapping file translates the highest-scoring prompts to Sonoscope instrument dimension values, including the full shipped Instrument vocabulary (`kick`, `snare`, `hi-hat`, `clap`, `percussion`, `tops`, `bass`, `chord`, `guitar`, `piano`, `brass`, `woodwind`, `strings`, `synth`, `lead`, `pad`, `vocal`, `fx`, `foley`, and `cymbal`):

```
"a kick drum sample"        → kick
"a snare drum sample"       → snare
"a hi-hat cymbal sample"    → hi-hat
"a hand clap sample"        → clap
"a bass guitar sample"      → bass
"an electric guitar sample" → guitar
"a synthesizer sample"      → synth
"a vocal sample"            → vocal
...
```

Only prompt candidates above their configured confidence threshold are emitted. The top scored candidates per dimension are returned. Confidence is the CLAP softmax score for the best prompt attached to the candidate.

Runtime device selection is owned by the Python sidecar. `SONOSCOPE_CLAP_DEVICE` defaults to `auto`, which tries CUDA first, then Apple MPS, then CPU. If an accelerator is reported available but fails while moving inputs/model state or running inference, CLAP retries on CPU for that request and keeps CPU as the active fallback device.

On CUDA, CLAP inference runs under PyTorch inference mode and enables TF32 matmul/cuDNN paths by default where PyTorch exposes them. Set `SONOSCOPE_DISABLE_CUDA_TF32=1` to disable TF32 if exact float32 behavior is needed for debugging.

On Windows, the analyzer resolves PyTorch from the CUDA 12.8 PyTorch index so GPU-capable NVIDIA environments do not silently install the CPU-only wheel. CPU fallback remains available when CUDA is not reported by PyTorch.

### 3.3 Tempo (Tempo dimension)

**Library:** `essentia.standard.RhythmExtractor2013`

Returns BPM with a confidence score. Only emitted if confidence ≥ 0.5.

### 3.4 Key (Key dimension)

**Library:** `essentia.standard.KeyExtractor`

Returns key + scale (major/minor) + strength. Key and Mode are emitted separately when strength ≥ 0.5.

---

## Confidence merging

When multiple stages produce a tag for the same (dimension, value), they are stored as separate rows in the `tags` table (different `source` values). They are not merged into one score — the UI displays them individually.

When the same source produces the same (dimension, value) twice (shouldn't happen but handled), the higher confidence wins.

After all auto-source tags are written, Core marks one primary auto tag per sample/dimension: the highest-confidence candidate wins, with insertion order as the tie-breaker. User selections later set a user primary tag for that dimension without deleting the other auto-source tags.

---

## Sidecar integration

The Python pipeline runs as a **long-lived subprocess** (one instance per library session). Core spawns it once when analysis begins and communicates over **stdin/stdout** using newline-delimited JSON.

### Process lifecycle
```
Core spawns sidecar → sidecar sends {"ready": true}
Core sends file request batches → sidecar processes and responds
Core sends {"shutdown": true} → sidecar exits cleanly
On app close / crash: Core kills the sidecar process
```

### Request format (Core → sidecar, one JSON object per line)
```json
{
  "requests": [
    {
      "id": "uuid-or-sequence-number",
      "path": "/absolute/path/to/file.wav",
      "relative_path": "Drums/Kicks/punchy_kick.wav"
    }
  ]
}
```

The request shape is batch-first. A single file is represented as a batch with one request.

### Response format (sidecar → Core, one JSON object per line)
```json
{
  "id": "uuid-or-sequence-number",
  "status": "ok",
  "tags": [
    {
      "dimension": "Type",
      "value": "loop",
      "source": "model",
      "confidence": 0.91
    },
    {
      "dimension": "Instrument",
      "value": "bass",
      "source": "heuristic",
      "confidence": 0.90
    }
  ],
  "file_meta": {
    "format": "wav",
    "duration_ms": 2048,
    "sample_rate": 44100,
    "bit_depth": 24,
    "channels": 1
  }
}
```

On failure:
```json
{
  "id": "uuid-or-sequence-number",
  "status": "error",
  "error": "unsupported format"
}
```

### Batching
Core sends analysis requests in batches to a single long-lived sidecar process. The default Core batch size is 16 and can be overridden with `SONOSCOPE_ANALYSIS_BATCH_SIZE`. The sidecar uses the same batch to run CLAP prompt scoring across multiple files in one model invocation, while metadata, heuristics, deterministic audio descriptors, and waveform generation remain per-file steps. CLAP can further split a Core batch with `SONOSCOPE_CLAP_BATCH_SIZE` when a GPU has less memory, or increase it when the GPU has headroom. The `id` field correlates every response to its request.

---

## Distribution

The sidecar is distributed as a self-contained binary alongside the Tauri app, built with **PyInstaller**. It bundles the Python runtime and libraries. Large CLAP weights and optional Essentia model files may be downloaded after install into the local model cache instead of being bundled into the app binary.

Model files are versioned and stored in the app's data directory. Core owns model cache management: it checks the required Hugging Face files for `laion/larger_clap_music` and the optional Essentia `.pb` files, downloads them on explicit user action, and launches the sidecar with `SONOSCOPE_CLAP_MODEL_PATH`, `SONOSCOPE_CLAP_LOCAL_ONLY=1`, and `SONOSCOPE_ESSENTIA_MODEL_DIR`. If a local model is incomplete or the corresponding Python package is unavailable, the analyzer skips that backend and still returns deterministic metadata/heuristic results.

**Current decision:** Model files are downloaded after install via the UI. Fully offline installers can still pre-populate the same app data model directories.

---

## Failure handling

| Scenario | Behaviour |
|---|---|
| Unsupported file format | `analysis_status = failed`, error stored, file visible in UI with no auto-tags |
| Corrupt / unreadable file | same as above |
| Stage 3 model unavailable | stages 1 and 2 results are still returned; Stage 3 skipped with a warning |
| Sidecar crashes | Core detects process exit, marks all in-flight files as `pending`, restarts sidecar |
| Sidecar unresponsive (timeout) | Core kills and restarts sidecar after configurable timeout (default: 30s per file) |
