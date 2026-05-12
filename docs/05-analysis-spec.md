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
| Instrument classification | `essentia`, PANNs (CNN14), YAMNet | ONNX only, no pre-trained audio models |
| BPM / beat tracking | `essentia`, `librosa` | `aubio` bindings (limited) |
| Key detection | `essentia`, `librosa` | none |

The gap is decisive at Stage 3. Rust has no production-ready pre-trained models for loop detection or instrument classification. The only Rust path for ML inference would be exporting a Python-trained model to ONNX and running it via `tract` — which still requires Python for model development and retraining, adds an export step, and provides no deployment benefit given we are already bundling a sidecar.

**Python is chosen for the full pipeline** (all three stages), running as a long-lived sidecar process.

---

## Stage 1 — Metadata extraction

**Library:** `mutagen` (tag reading) + `soundfile` / `audioread` (format info)

Extracts:
- Format, sample rate, bit depth, channels, duration
- Embedded text tags: title, comment, genre, BPM, key (if present)

Embedded tags are mapped to dimensions where a clear mapping exists:

| Embedded field | Dimension | Notes |
|---|---|---|
| BPM / TBPM | Tempo | parsed as float |
| KEY / TKEY | Key | normalised to chromatic note name |
| GENRE | Instrument | low confidence, fuzzy match against instrument values |
| COMMENT | — | stored as-is for heuristic stage to process |

Embedded tags that map cleanly (BPM, Key) are treated as `metadata` source with confidence 0.95. Genre-based instrument guesses use confidence 0.4.

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

### Instrument detection

Token list matched against dimension_values for the Instrument dimension. Matching is case-insensitive, whole-word.

Examples: `kick`, `kik`, `bd`, `bass drum` → Instrument: kick (0.9); `snare`, `snr`, `sd` → Instrument: snare (0.9); `808` → Instrument: bass (0.75); `hat`, `hh`, `hihat` → Instrument: hi-hat (0.9); etc.

The full token list is maintained in a heuristic config file (JSON) shipped with the sidecar, not hardcoded. This makes it easy to extend without changing code.

### Tempo detection

`(\d{2,3})\s?bpm` → Tempo: extracted value (0.95)

### Key detection

`[A-G][b#]?\s?(maj|min|major|minor)?` → Key: normalised value (0.85 if both note and mode present, 0.65 if note only)

---

## Stage 3 — ML audio classification

**Primary library:** [Essentia](https://essentia.upf.edu/) (MTG / Universitat Pompeu Fabra)

Essentia is chosen because it ships pre-trained TensorFlow Lite models purpose-built for music information retrieval tasks, including loop detection and instrument recognition. Models run locally with no network calls.

### 3.1 Loop vs. one-shot (Type dimension)

**Model:** `essentia-tensorflow` — DiscogEFNet or the dedicated loop/one-shot classifier

Produces: `loop` or `one-shot` with a probability score → confidence.

Fallback: if Essentia's loop model is unavailable, use onset density heuristic via `librosa.onset.onset_detect` — a file with a single onset cluster is likely a one-shot; regular periodic onsets suggest a loop.

### 3.2 Instrument classification (Instrument dimension)

**Model:** PANNs CNN14 (`panns_inference`) — pre-trained on AudioSet (527 audio classes)

PANNs outputs probabilities over 527 AudioSet classes. A static mapping file translates AudioSet class names to Sonoscope instrument dimension values:

```
"Kick drum"           → kick
"Snare drum"          → snare
"Hi-hat"              → hi-hat
"Clapping"            → clap
"Bass guitar"         → bass
"Electric guitar"     → chord / lead
"Synthesizer"         → synth
"Vocal music"         → vocal
...
```

Only AudioSet classes with probability ≥ 0.3 are emitted. The top-3 mapped Sonoscope values are returned. Confidence = AudioSet probability × mapping_weight (mapping_weight reflects how direct the mapping is; exact matches = 1.0, broad mappings = 0.7).

### 3.3 Tempo (Tempo dimension)

**Library:** `essentia.standard.RhythmExtractor2013`

Returns BPM with a confidence score. Only emitted if confidence ≥ 0.5.

### 3.4 Key (Key dimension)

**Library:** `essentia.standard.KeyExtractor`

Returns key + scale (major/minor) + strength. Only emitted if strength ≥ 0.5.

---

## Confidence merging

When multiple stages produce a tag for the same (dimension, value), they are stored as separate rows in the `tags` table (different `source` values). They are not merged into one score — the UI displays them individually.

When the same source produces the same (dimension, value) twice (shouldn't happen but handled), the higher confidence wins.

---

## Sidecar integration

The Python pipeline runs as a **long-lived subprocess** (one instance per library session). Core spawns it once when analysis begins and communicates over **stdin/stdout** using newline-delimited JSON.

### Process lifecycle
```
Core spawns sidecar → sidecar sends {"ready": true}
Core sends file requests → sidecar processes and responds
Core sends {"shutdown": true} → sidecar exits cleanly
On app close / crash: Core kills the sidecar process
```

### Request format (Core → sidecar, one JSON object per line)
```json
{
  "id": "uuid-or-sequence-number",
  "path": "/absolute/path/to/file.wav",
  "relative_path": "Drums/Kicks/punchy_kick.wav"
}
```

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
Core sends requests to the sidecar as fast as they are produced. The sidecar processes them and responds as each completes. Core does not need to wait for one response before sending the next — the `id` field correlates requests to responses. Concurrency within the sidecar is managed by its own thread pool (configurable, default: number of CPU cores).

---

## Distribution

The sidecar is distributed as a self-contained binary alongside the Tauri app, built with **PyInstaller**. It bundles the Python runtime, all libraries, and model weights. The models (Essentia TFLite + PANNs CNN14) add approximately 50–150 MB to the distribution.

Model files are versioned and stored in the app's data directory. On first launch, the sidecar checks for model files and downloads them if missing (or they can be bundled directly for fully offline installs).

**[open]** Decide whether models are bundled in the installer or downloaded on first run. Tradeoff: installer size vs. first-run experience.

---

## Failure handling

| Scenario | Behaviour |
|---|---|
| Unsupported file format | `analysis_status = failed`, error stored, file visible in UI with no auto-tags |
| Corrupt / unreadable file | same as above |
| Stage 3 model unavailable | stages 1 and 2 results are still returned; Stage 3 skipped with a warning |
| Sidecar crashes | Core detects process exit, marks all in-flight files as `pending`, restarts sidecar |
| Sidecar unresponsive (timeout) | Core kills and restarts sidecar after configurable timeout (default: 30s per file) |
