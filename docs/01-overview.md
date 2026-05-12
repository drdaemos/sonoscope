# Sonoscope — Product Overview

## What it is

Sonoscope is a desktop application for curating and reorganizing local audio sample libraries. It scans a folder of audio files, automatically classifies them using a combination of filename/metadata heuristics and ML-based audio analysis, lets the user review and adjust those classifications, then reorganizes the files into a clean folder structure ready for upload to a hardware sampler.

Runs on macOS and Windows.

---

## Target context

The primary use case is a producer with a large, unstructured sample collection — typically organized by provider or acquisition source rather than by musical function — who wants to reshape that collection into a taxonomy that makes sense on a hardware sampler (e.g., by instrument type, by sample type). File reorganization is always explicit and user-initiated; the app never modifies files without the user directing it to do so.

---

## Core concepts

**Library**
A library is a root folder on disk plus an associated SQLite database stored alongside it. The database tracks every audio file discovered under that root, all tags assigned to them, the history of file operations, and original file paths. Different root folders are different libraries.

**Sample**
Any audio file discovered within a library's root folder. Sonoscope does not convert or modify audio content — files are treated as opaque blobs; only their location and metadata are managed.

**Dimension**
A classification axis. Each dimension represents one orthogonal property of a sample. Examples: Type (loop vs. one-shot), Instrument (kick, snare, bass, pad, …), Key, Tempo, Mood. Dimensions are not fixed — the system supports an extensible set.

**Tag**
A value within a dimension assigned to a sample. A sample can carry multiple tags within the same dimension (e.g., Instrument: bass + synth). Tags can originate from auto-analysis or from the user. A sample can have tags from multiple sources on the same dimension; when those sources disagree, a conflict is recorded.

**Auto-tag**
A tag produced by the analysis pipeline (heuristics + ML). Auto-tags carry a confidence score and a source label (filename heuristic, metadata, ML model). They are proposals, not edits — they require no user action to view, but user edits take precedence.

**Conflict**
When two tagging sources assign different values to the same sample on the same dimension, that is a conflict. Conflicts are surfaced visually in the review UI for the user to resolve. Resolution strategy is user-driven.

**Organization pattern**
A folder hierarchy template the user defines to describe how files should be arranged. Expressed as a path with dimension placeholders, e.g. `{Type}/{Instrument}`. Applied as either a move (within the library) or a copy (to an external destination).

**Device template**
A named preset that encodes the constraints and conventions of a target sampler. For example, an Ableton Move template might specify expected folder depth, naming conventions, or which dimensions map to which levels of the hierarchy. Templates are selectable; the first supported template is Ableton Move.

**File operation**
Any action that changes a file's location on disk. All file operations are logged in the library database with the original path, so they can be rolled back.

---

## Core workflow

```
1. Open library
   User points Sonoscope at a root folder. The app initialises (or loads)
   the library database for that folder.

2. Scan
   Sonoscope recursively discovers all audio files under the root.
   New files are added to the database; previously seen files are checked
   for moves or deletions.

3. Analyse
   Each file is passed through the analysis pipeline:
   - Filename and path heuristics
   - Audio metadata (embedded tags, format info)
   - ML-based audio classifier
   Results are stored as auto-tags with confidence scores.

4. Review
   The user sees a DAW-style file list. Each row shows the file,
   its auto-tags, and any conflicts. The user can:
   - Play back any sample in-app
   - Edit tags on individual files
   - Select multiple files and bulk-reassign tags
   - Filter and sort by any dimension

5. Organise
   The user selects an organization pattern (or picks a device template)
   and chooses a mode:
   - Move: relocate files within the library root
   - Copy: duplicate files to an external destination folder

6. Apply & track
   Sonoscope executes the file operations, logs every move in the
   database (original path → new path), and reports the result.
   Operations can be rolled back from the history log.
```

---

## What Sonoscope does not do

- Convert audio formats or modify audio content
- Push files directly to a device (files are transferred manually after export)
- Provide mixing, editing, or DAW functionality
- Enforce a fixed tag taxonomy

---

## Open decisions

The following are deferred and will be resolved in later specification documents:

- Backend language / runtime (Rust-native vs. Python subprocess) — depends on the analysis pipeline design
- Exact ML models and libraries used for classification
- Conflict resolution policy (automatic or always manual)
- Multi-library management UI
- Ableton Move template specifics
