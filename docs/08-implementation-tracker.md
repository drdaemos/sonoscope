# Sonoscope — Implementation Tracker

This document tracks implementation status by phase and feature. Update it in the same change set as the code whenever a feature is started, completed, deferred, or materially changed.

## Status Legend

- `Not started` — no meaningful implementation exists yet.
- `In progress` — implementation exists but the phase/feature is not complete or verified.
- `Implemented` — feature is implemented and covered by the required checks/tests for its phase.
- `Deferred` — intentionally moved out of the current phase.
- `Blocked` — cannot proceed until a named dependency or decision is resolved.

## Maintenance Rule

Every implementation PR or commit must update this tracker when it changes project capability. Keep entries feature-level, not commit-level. Include the important verification command or test file when marking something `Implemented`.

---

## Phase 0 — Scaffold

Goal: empty app runs; all tooling works; CI passes.

| Feature | Status | Notes / Verification |
|---|---|---|
| Tauri + Svelte + Vite scaffold | Implemented | App scaffold exists under `src/` and `src-tauri/`. |
| TypeScript strict config | Implemented | `npm run check` passes. |
| Tailwind configured | Implemented | Tailwind Vite plugin is configured. |
| shadcn-svelte configured | In progress | Manual setup is installed; component coverage continues in Phase 2. |
| TanStack Table + Virtual installed | Implemented | Dependencies are present in `package.json`. |
| tauri-specta bindings generation | Implemented | `npm run generate-bindings` writes `src/lib/bindings/bindings.ts`. |
| Python analyzer scaffold | Implemented | `analyzer/` contains uv project, Pydantic protocol models, and protocol tests. |
| Rust sqlx + migrations scaffold | Implemented | `src-tauri/migrations/001_init.sql`; `cargo test` passes. |
| GitHub Actions CI | Implemented | `.github/workflows/ci.yml` runs frontend, Rust, and analyzer checks on Windows/macOS. |

## Phase 1 — Library + Discovery

Goal: user can open a folder, discover audio files, and see them in a basic list.

| Feature | Status | Notes / Verification |
|---|---|---|
| `library_meta` and `samples` schema | Implemented | Migration `001_init.sql`; `test_open_creates_schema`. |
| Open/create library DB | Implemented | `open_library`; `test_open_creates_schema`, `test_open_is_idempotent`. |
| Recursive audio discovery | Implemented | `start_discovery`; `test_discover_inserts_only_audio_files`. |
| Atomic discovery transaction | Implemented | Discovery uses a single transaction; cancellation rollback test covers no partial rows. |
| Cancellable discovery | Implemented | `cancel_discovery`; `test_discover_cancellation_rolls_back_transaction`. |
| Discovery progress events | Implemented | Emits `discovery-progress`, `discovery-complete`, and `discovery-cancelled`. |
| Generated command bindings | Implemented | `openLibrary`, `startDiscovery`, `cancelDiscovery`, `getSamples`. |
| Library selector UI | In progress | Header selector opens libraries, shows the selected library name, and stores five recent libraries in localStorage. |
| Basic file list UI | In progress | `FileList.svelte` shows filename, path subtitle, primary/secondary tags, conflict state, and audio duration using shared UI primitives. |

## Phase 2 — UI Foundation

Goal: introduce the app shell and design system foundation before feature-heavy UI work continues.

| Feature | Status | Notes / Verification |
|---|---|---|
| Design tokens | Implemented | Manual shadcn-svelte CSS variables in `src/app.css`; `npm run build` passes. |
| Base app shell | Implemented | Top bar, workflow tabs, sidebar, main panel, and footer are present in `App.svelte`. |
| Core UI primitives | In progress | Button, badge, card, input, separator, and tabs primitives exist under `src/lib/components/ui/`. |
| Workflow view states | Implemented | Review, Organise, and History view states exist with placeholders for unavailable workflows. |
| Phase 1 UI cleanup | In progress | Library controls and file list now use shared primitives; table remains intentionally basic. |
| Component usage conventions | In progress | Phase 2 primitives exist; broader usage docs still needed once the set stabilizes. |
| UI verification | Implemented | `npm run check` and `npm run build` pass. |

## Phase 3 — Sidecar + Heuristic Analysis

Goal: files are analyzed by filename heuristics; tags appear in the list.

| Feature | Status | Notes / Verification |
|---|---|---|
| Pydantic IPC protocol models | Implemented | `analyzer/tests/test_protocol.py`; `uv run pytest`. |
| Analyzer stdin/stdout loop | Implemented | Emits ready line and processes newline-delimited requests in `sonoscope_analyzer.main`. |
| Heuristic token config | Implemented | `analyzer/sonoscope_analyzer/mappings/heuristic_tokens.json`. |
| Filename/path heuristics | Implemented | Filename-only matching with one-shot default when no Type is detected; 50+ parametrized cases in `analyzer/tests/test_heuristics.py`. |
| Metadata extraction | Implemented | Mutagen + SoundFile coverage in `analyzer/tests/test_metadata.py`. |
| Rust sidecar process manager | In progress | Long-lived uv-managed analyzer client exists; ignored Rust integration test passes when explicitly run with process-spawn access. |
| Tags schema migration | Implemented | `src-tauri/migrations/002_tags.sql`, `003_expanded_tag_values.sql`, and `004_primary_tags.sql`. |
| Seed system dimensions/values | Implemented | Covered by `test_open_seeds_system_tag_dimensions`; includes the expanded heuristic Type/Instrument vocabulary. |
| Analysis orchestrator | Implemented | Queues pending samples, supports full-library reanalysis after re-scan, dispatches to analyzer, persists auto-tags, marks auto-primary tags, and updates status. |
| Tag columns in file list | Implemented | Type + Instrument chips are shown in `FileList.svelte`. |
| Analysis progress badge | Implemented | Header uses one scan/analyze pipeline action with progress state; completed libraries show `Re-scan` and requeue samples for analysis. |

## Phase 4 — Review UI

Goal: user can review and edit tags; filtering and search work.

| Feature | Status | Notes / Verification |
|---|---|---|
| Filter sidebar | Implemented | Dimension chips with counts for Type, Instrument, and Key in `FilterSidebar.svelte`; verified by `npm run check` and `npm run build`. |
| Filename search | Implemented | Real-time filename substring filter in `src/lib/stores/review.ts`; verified by `npm run check` and `npm run build`. |
| Sortable columns | Implemented | Sample and tag-dimension sorting in `FileList.svelte`; review rows now use deterministic fixed-height virtualization to avoid stale measurement state across filter changes; verified by `npm run check`, `npm run test`, and `npm run build`. |
| Inline tag editing | Implemented | Reusable tag editor supports enum, multi-enum, and numeric dimensions from typed dimension metadata; covered by `TagValueEditor.test.ts`, `npm run check`, and `npm run build`. |
| Bulk tag editor | Implemented | Multi-select action bar uses typed dimension metadata for all editable enum, multi-enum, and numeric dimensions; row drag selection selects or deselects along a single drag path based on the starting row; verified by `npm run check` and `npm run build`. |
| Conflict indicator and panel | Implemented | Conflict resolver is an overlay panel outside table layout, shows current candidate plus alternatives, and no longer resets the review scroll position on resolve; covered by `ConflictResolver.test.ts`. |
| `tags::set_user_tag` command | Implemented | Typed `set_user_tag` command and generated `commands.setUserTag`; covered by `test_user_tag_write_and_clear_preserves_auto_tags`. |
| `tags::clear_user_tag` command | Implemented | Typed `clear_user_tag` command and generated `commands.clearUserTag`; covered by `test_user_tag_write_and_clear_preserves_auto_tags`. |
| Conflict query tests | Implemented | `test_conflict_query_returns_unresolved_auto_tag_conflicts`. |

## Phase 5 — ML Analysis

Goal: ML-based Type and Instrument classification runs as part of analysis.

| Feature | Status | Notes / Verification |
|---|---|---|
| Loop/one-shot classifier | In progress | Typed model-output mapping exists behind an optional `LoopDetector` protocol; real Essentia backend still pending. |
| Instrument classifier | In progress | Typed model-output mapping exists behind an optional `InstrumentModel` protocol; real PANNs CNN14 backend still pending. |
| AudioSet mapping config | Implemented | `analyzer/sonoscope_analyzer/mappings/audioset_map.json` maps model labels to seeded Sonoscope dimensions. |
| Waveform generation | Implemented | Analyzer emits byte-scaled peak amplitude bins; Rust persists them to `samples.waveform_data`. Covered by `test_waveform.py`. |
| Waveform DB migration | Deferred | `waveform_data` already exists in `001_init.sql`; no new migration required. |
| ML mapping unit tests | Implemented | `analyzer/tests/test_classifier.py` validates mocked model output mapping without loading models. |
| Integration fixture suite | Not started | Mark with `@pytest.mark.integration`. |
| End-to-end analysis verification | Not started | Scan fixture library and compare DB tags to manifest. |

## Phase 6 — Audio Playback

Goal: user can play samples in-app with waveform display.

| Feature | Status | Notes / Verification |
|---|---|---|
| Playback footer UI | In progress | Play/pause, loop toggle, seek, timestamp, volume, mute, and selected-sample loading exist in `PlaybackFooter.svelte`; waveform drawing still pending. Loop defaults on when the primary Type tag is `loop`, and playback uses Web Audio buffer looping to avoid native media element loop gaps. |
| Row double-click playback | Implemented | Double-clicking a non-interactive review row loads and autoplays the sample. |
| Keyboard playback controls | In progress | Space toggles play/pause when focus is not inside an input/control; row navigation still pending. |
| Local audio asset protocol | Implemented | `get_sample_playback` validates the sample path against the opened library and the UI loads it via Tauri `convertFileSrc`; asset protocol enabled in `tauri.conf.json`. |
| Waveform rendering component | Not started | Draws cached waveform data. |
| Playback store tests | Implemented | `src/lib/stores/playback.test.ts`; verified by `npm run test`. |

## Phase 7 — Organise + History

Goal: user can reorganize files and roll back.

| Feature | Status | Notes / Verification |
|---|---|---|
| Operation history schema | Not started | Planned `operation_batches` and `file_operations`. |
| Pattern resolver | Not started | Handles placeholders, missing tags, and sanitized path parts. |
| Organise preview command | Not started | Typed Tauri command. |
| Organise apply command | Not started | Move and copy modes. |
| Rollback command | Not started | Move rollback only. |
| Organise UI | Not started | Pattern editor, presets, mode selector, preview, apply. |
| History UI | Not started | Batch list, rollback confirmation. |
| File operation tests | Not started | Temp directory integration coverage. |

## Phase 8 — Settings + Polish

Goal: dimension management, preset management, and edge case handling.

| Feature | Status | Notes / Verification |
|---|---|---|
| Dimension management UI | Not started | Add/remove custom dimensions and values. |
| Preset management UI | Not started | Manage organization presets. |
| Dimension create/delete commands | Not started | Include in-use guard for delete. |
| Reset `analysing` on startup | Not started | Requeue interrupted analysis. |
| Sidecar crash handling | Not started | Restart and mark in-flight files pending. |
| E2E smoke tests | Not started | Open library, scan, edit tag, organise, rollback. |
| Performance pass | Not started | 10k row list and analysis throughput. |
