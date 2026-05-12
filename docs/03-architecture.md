# Sonoscope — Architecture

## Component overview

```
┌─────────────────────────────────────────────────────┐
│                   Tauri Desktop App                 │
│                                                     │
│  ┌─────────────────────┐   ┌─────────────────────┐  │
│  │    UI (Svelte)      │   │   Core (Rust)       │  │
│  │                     │◄──►                     │  │
│  │  - File list view   │   │  - Library manager  │  │
│  │  - Tag editor       │   │  - Scan orchestrator│  │
│  │  - Organise panel   │   │  - File operations  │  │
│  │  - History view     │   │  - DB access layer  │  │
│  │  - Audio player     │   │  - IPC commands     │  │
│  └─────────────────────┘   └──────────┬──────────┘  │
│          Tauri IPC (invoke/events)    │             │
└───────────────────────────────────────┼─────────────┘
                                        │
                          ┌─────────────▼──────────────┐
                          │    Analysis Pipeline        │
                          │    Python sidecar (§ADR-1) │
                          │                             │
                          │  - Filename heuristics      │
                          │  - Metadata extraction      │
                          │  - ML audio classifier      │
                          │  - Confidence scoring       │
                          └─────────────────────────────┘
                                        │
                          ┌─────────────▼──────────────┐
                          │     SQLite Database         │
                          │     (library.db)            │
                          └─────────────────────────────┘
```

---

## Components

### UI — Svelte + Vite
Runs inside the Tauri WebView. Responsible for all visual rendering and user interaction. Has no direct access to the file system or database — all data operations go through the Tauri IPC layer.

**Frontend stack:**
- **Svelte + Vite** — no SvelteKit; the three workflow tabs are component-level state, not routes
- **shadcn-svelte** (Bits UI + Tailwind CSS) — base component library: popovers, dropdowns, dialogs, badges, sliders, progress indicators
- **TanStack Table** (`@tanstack/svelte-table`) — file list column management, sorting, and row selection logic
- **TanStack Virtual** (`@tanstack/svelte-virtual`) — row virtualisation for the file list; required for libraries of thousands of files

Communicates with Core via:
- **Commands** (`invoke`): request/response for actions (open library, apply tags, start scan, execute reorganise)
- **Events** (`listen`): streaming updates pushed from Core (scan progress, analysis results arriving, file operation completion)

### Core — Rust (inside Tauri process)
The application logic layer. Owns all access to the file system and the database. Exposes a set of Tauri commands that the UI calls into. Orchestrates the scan flow: discovers files, dispatches them to the Analysis Pipeline, receives results, and writes them to the database.

Responsibilities:
- Library initialisation and loading
- Recursive file discovery
- Dispatching files to the analysis pipeline and collecting results
- All SQLite read/write via an internal database module
- Executing and logging file operations (move/copy)
- Rollback of logged batches

### Analysis Pipeline — language TBD (see ADR-1)
A self-contained unit responsible for taking a file path and returning a set of tag candidates with source labels and confidence scores. Invoked per-file (or in batches) by the Core during a scan.

Regardless of language, this component exposes a stable interface:

**Input:** file path (+ optional hints such as parent folder path)

**Output (per file):**
```
{
  tags: [
    { dimension: "Type", value: "loop", source: "model", confidence: 0.91 },
    { dimension: "Instrument", value: "bass", source: "heuristic", confidence: 1.0 },
    ...
  ]
}
```

This interface is the integration boundary. The Core does not care how the pipeline is implemented — only that it accepts file paths and returns structured tag results.

### SQLite Database
One `library.db` file per library, stored in the library root folder. Accessed exclusively through Core. Schema is detailed in `04-data-model.md`.

---

## Data flow — key scenarios

### Discovery scan
```
UI: "Scan library"
  → Core: walk root folder, collect all audio file paths (atomic)
  → Core: write all discovered files to DB in a single transaction
  ← Core: emit "discovery complete" event with file count
  ← UI: populate file list (all files visible, analysis status = pending)
```

### Analysis (resumable)
```
On library open (and after discovery):
  Core: query DB for all files with analysis_status = pending
  Core: process files in batches through Analysis Pipeline
  ← Analysis Pipeline: tag candidates + confidence scores per file
  → Core: write auto-tags to DB, set analysis_status = done
  → Core: emit per-file or per-batch progress events
  ← UI: update rows incrementally as events arrive

On interruption (app closed / crash):
  Partially analysed files remain as analysis_status = pending in DB.
  On next library open, Core resumes from the pending queue automatically.
```

### Tag edit
```
UI: user edits a tag on a sample
  → Core: write user-source tag to DB (dimension, value, sample_id)
  ← Core: confirm write
  ← UI: re-render row
```

### Organise (move)
```
UI: user picks pattern + confirms preview
  → Core: resolve pattern against each sample's tags
  → Core: compute current path → new path for every file
  ← Core: return preview list to UI
UI: user confirms
  → Core: execute file moves
  → Core: log each operation as a batch in DB
  ← Core: completion event
  ← UI: update file paths in list
```

### Rollback
```
UI: user selects a batch in history view
  → Core: load batch operations from DB
  → Core: execute reverse moves (new path → original path)
  → Core: mark batch as rolled back in DB
  ← UI: update file list
```

---

## ADR-1 — Analysis pipeline language

**Status:** Open — to be decided when the analysis pipeline is specced.

**Context:** The pipeline needs filename/path pattern matching, audio metadata parsing, and ML-based audio classification. The language choice affects the ML library ecosystem available, deployment complexity, and runtime performance.

### Option A — Rust-native (embedded in Tauri process)

The pipeline is a Rust module compiled into the same binary as Core. No subprocess boundary; Core calls it as a library.

| | |
|---|---|
| **Pro** | Single binary, no external runtime, fast file I/O, simple distribution |
| **Pro** | Direct function calls — no serialisation overhead for large audio buffers |
| **Con** | Rust audio ML ecosystem is thin; inference likely requires ONNX Runtime (`tract`) with a pre-exported model |
| **Con** | More custom code for feature extraction (spectral analysis, onset detection) |

### Option B — Python sidecar (separate process)

The pipeline is a standalone Python script or package, bundled as a Tauri sidecar binary (frozen via PyInstaller or similar). Core spawns it as a subprocess and communicates over stdin/stdout or a local socket using a simple JSON protocol.

| | |
|---|---|
| **Pro** | Full access to Python audio/ML ecosystem: `librosa`, `essentia`, `transformers`, Hugging Face models |
| **Pro** | Easy to iterate on models and heuristics independently of the Rust app |
| **Con** | Requires bundling a Python runtime; distribution size is larger (~50–150 MB overhead) |
| **Con** | Subprocess startup latency (mitigated by keeping it alive for the duration of a scan) |
| **Con** | JSON serialisation boundary; audio data passed by file path, not in-memory |

### Decision criteria
The decision hinges on what ML models are chosen for audio classification. If a suitable pre-trained ONNX model exists for the task (instrument + loop/oneshot classification), Option A is viable. If the best available models are Python-native (Torch, TensorFlow), Option B is the practical choice.

**Decision to be made in `05-analysis-spec.md`** after evaluating available models.

---

## Constraints and assumptions

- The Tauri WebView handles audio playback via the HTML5 `<audio>` element, reading files from disk via Tauri's asset protocol. No custom audio decoding in Core is required for playback.
- The database is accessed by a single process (Core). No concurrent writers; WAL mode is enabled so the UI can read (browse) while Core is writing analysis results.
- Analysis status is tracked per sample (`pending` / `done` / `failed`). On library open, Core automatically resumes analysis for any files in `pending` state, enabling resumability across sessions without extra user action.
- The Analysis Pipeline processes one file at a time or in small batches; parallelism is managed by Core (thread pool), not by the pipeline itself.
- All file paths stored in the database are absolute. Relative paths are computed from the library root only for display and pattern resolution.
