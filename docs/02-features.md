# Sonoscope — Feature Inventory

Features are grouped by functional area. Each entry describes behaviour at the product level — not implementation detail. Items marked **[open]** are unresolved and need decision before or during detailed spec.

---

## 1. Library

### 1.1 Open / create library
The user selects a root folder via the OS file picker. If no library database exists there, Sonoscope initialises one. If one exists, it is loaded. The app remembers recently opened libraries for quick access.

### 1.2 Discovery scan
On first open (or on demand), Sonoscope recursively walks the root folder and registers every audio file (WAV, AIFF, FLAC, MP3, OGG, and other common formats) in the database with its path, format, size, and duration. Discovery is atomic — if cancelled, no changes are committed and the database is left in its pre-scan state.

### 1.3 Incremental re-scan
Subsequent discovery scans detect changes since the last scan: new files added, files deleted, files moved or renamed. The database is updated accordingly. Analysis results for unchanged files are preserved.

### 1.4 Analysis
After discovery, each file is processed through the analysis pipeline (heuristics + ML). Analysis runs in batches and is resumable — results are written to the database per file as they are produced. If analysis is interrupted (app closed, crash), it resumes automatically on the next library open, skipping files already analysed. Files in the library are browsable at any point; unanalysed files are shown with an indicator that analysis is pending.

---

## 2. Audio Analysis

### 2.1 Filename and path heuristics
The analysis pipeline extracts signal from the file's name and folder path — e.g. tokens like "kick", "loop", "120bpm", note names. Rules are pattern-based and produce tags with a `heuristic` source label.

### 2.2 Metadata extraction
Embedded audio metadata (ID3, BWF, AIFF chunks, etc.) is read and mapped to tags where applicable. Produces tags with a `metadata` source label.

### 2.3 ML audio classification
An ML model analyses the audio content and produces tag candidates with confidence scores. At minimum covers the Type dimension (loop vs. one-shot) and Instrument category. Produces tags with a `model` source label.

**[open]** Which specific models or libraries are used — decided when backend and analysis pipeline are specced.

### 2.4 Confidence scores
Every auto-tag carries a numeric confidence score (0–1). Scores are stored and visible in the review UI. Low-confidence tags are visually distinguished from high-confidence ones.

### 2.5 Conflict detection
When two sources assign different values to the same sample on the same dimension, a conflict is recorded. The conflict stores all competing values and their source labels. Conflicts are not automatically resolved.

---

## 3. Tag System

### 3.1 Dimensions
The system ships with a predefined set of dimensions:

| Dimension  | Type       | Notes                          |
|------------|------------|--------------------------------|
| Type       | enum       | loop, one-shot                 |
| Instrument | multi-enum | kick, snare, hi-hat, bass, pad, synth, vocal, fx, … |
| Key        | enum       | chromatic note + octave        |
| Tempo      | numeric    | BPM                            |
| Mood       | multi-enum | dark, bright, aggressive, … — extensible |

New dimensions can be added by the user. Dimensions have a type (enum, multi-enum, numeric, free text).

**[open]** Which predefined dimensions are included at launch vs. later? Minimum viable set is Type + Instrument.

### 3.2 Tag values
Predefined tag values exist for each enum/multi-enum dimension. The user can create new values within any dimension. Deleting a value is possible only if no sample uses it (or with a bulk reassignment step).

### 3.3 Tag sources
Every tag on a sample records its source: `heuristic`, `metadata`, `model`, or `user`. User edits always take precedence in display and export. The underlying auto-tag is preserved and remains visible.

### 3.4 Multi-value tags
Samples can hold multiple values within the same dimension (e.g. Instrument: bass + synth). This applies to dimensions typed as multi-enum.

---

## 4. Review UI

### 4.1 File list
The primary view is a list of all samples in the library. Columns include filename, relative path, duration, and one column per active dimension showing the sample's current tags. Rows are selectable.

### 4.2 Sorting
Any column (including tag dimensions) is sortable ascending/descending by clicking the column header.

### 4.3 Filtering and search
The user can filter the list by one or more dimension values (e.g. show only loops tagged as kick or snare). Filters combine with AND logic across dimensions and OR logic within a dimension. A text search field filters by filename substring — e.g. typing "110" shows all files whose name contains "110".

### 4.4 Conflict indicator
Rows with one or more unresolved tag conflicts display a visual indicator. A conflict panel or inline expansion shows the competing values and their sources, allowing the user to pick one or enter a new value.

### 4.5 Inline tag editing
Clicking a tag cell on any row opens an editor for that sample's tag on that dimension. The user can add, remove, or change values. Changes are saved immediately to the database as user-source tags.

### 4.6 Bulk tag assignment
The user selects multiple rows and applies a tag edit to all of them at once. Bulk edits can set, add, or clear tag values on a dimension across the selection.

### 4.7 In-app audio playback
Clicking a sample (or pressing a keyboard shortcut) plays it back through the system audio output. A waveform or progress bar shows playback position. The user can seek. Only one sample plays at a time.


### 4.8 Confidence display
Auto-tag values are visually annotated with their confidence level (e.g. full opacity = high confidence, muted = low confidence, or a colour band). The user can show/hide this layer.

---

## 5. Organisation & Export

### 5.1 Organization pattern editor
The user defines a folder path template using dimension placeholders, e.g. `{Type}/{Instrument}`. The editor shows available dimensions and validates the pattern. Samples with missing tags on a used dimension go to a configurable fallback folder (e.g. `_untagged`).

### 5.2 Organisation pattern presets
Named presets that store a saved organization pattern. Presets are named after the dimension order they encode, e.g. "Type / Instrument" or "Instrument / Type / Key". The user can create, rename, and delete presets. A set of sensible defaults ships with the app.

### 5.3 Reorganisation preview
Before executing, the app shows a diff-like preview: current path → new path for every affected file. The user confirms before any files are moved or copied.

### 5.4 Move mode
Files are relocated within the library root according to the pattern. Original paths are logged in the database.

### 5.5 Copy mode
Files are duplicated to a user-specified external destination folder according to the pattern. The library root is not modified.

### 5.6 Untagged fallback
Files missing a tag required by the pattern are placed in a `_untagged` subfolder at the destination root, preserving their original filenames.

---

## 6. File Operation History & Rollback

### 6.1 Operation log
Every file operation (move, copy) is recorded in the database: timestamp, operation type, original path, new path.

### 6.2 Rollback
The user can roll back an entire batch (all operations from one apply action): files are moved back to their original paths. Rollback only applies to move operations within the library — copy operations cannot be reversed from the destination.

### 6.3 History view
A dedicated view lists all past file operations, filterable by date and operation type, with rollback controls.

---

## 7. Settings & Configuration

### 7.1 Dimension management
The user can add custom dimensions, configure their type, and define the allowed values.

### 7.2 Template management
The user can view, create, edit, and delete organisation pattern presets.

---

## Out of scope (confirmed)

- Audio format conversion
- Direct device transfer (USB/SD card push)
- Audio editing or mixing
- Fixed mandatory taxonomy
