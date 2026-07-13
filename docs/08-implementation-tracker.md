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
| Filename/path heuristics | Implemented | Filename-only matching without Type defaulting; if neither filename nor ML produces Type evidence, Type is left empty. Covered by `analyzer/tests/test_heuristics.py` and `analyzer/tests/test_main.py`. |
| Metadata extraction | Implemented | Mutagen + SoundFile coverage in `analyzer/tests/test_metadata.py`. |
| Rust sidecar process manager | In progress | Long-lived uv-managed analyzer client exists, sends batch-first IPC requests, and has an ignored Rust integration test that passes when explicitly run with process-spawn access. Response reads are guarded by a configurable timeout (`SONOSCOPE_ANALYZER_TIMEOUT_SECS`), and the sidecar is killed and respawned after a failed batch so a desynchronized pipe cannot stall later batches. The analyzer guarantees one response line per request (per-request error isolation; batch validation failures answer entry-for-entry), covered by `analyzer/tests/test_main.py`. |
| Tags schema migration | Implemented | `src-tauri/migrations/002_tags.sql`, `003_expanded_tag_values.sql`, and `004_primary_tags.sql`. |
| Seed system dimensions/values | Implemented | Covered by `test_open_seeds_system_tag_dimensions`; includes the expanded heuristic Type/Instrument vocabulary. Migration `008_drums_instrument.sql` adds the full-kit `drums` Instrument value (heuristic tokens, CLAP prompts, and embedded-genre mapping updated to match). |
| Analysis orchestrator | Implemented | Queues pending samples, dispatches configurable sidecar batches, persists auto-tags per sample in one transaction using a preloaded dimension lookup, marks auto-primary tags via the shared `tags` module, and updates status. Re-scan now analyses only pending samples; `start_analysis` takes a typed scope (`pending` \| `untagged` \| `all`), and header menu actions requeue either the full library (`Re-analyse all samples`) or only samples missing a Type or Instrument tag (`Re-analyse untagged samples`, covered by `test_requeue_untagged_samples_targets_missing_type_or_instrument`). Supports cancellation (`cancel_analysis`) and emits `analysis-cancelled`/`analysis-failed` events surfaced in the UI. |
| Tag columns in file list | Implemented | Type + Instrument chips are shown in `FileList.svelte`. |
| Analysis progress badge | Implemented | Header uses one scan/analyze pipeline action with progress state; completed libraries show `Re-scan` and requeue samples for analysis. |

## Phase 4 — Review UI

Goal: user can review and edit tags; filtering and search work.

| Feature | Status | Notes / Verification |
|---|---|---|
| Filter sidebar | Implemented | Dimension chips with counts for Type, Instrument, and Key in `FilterSidebar.svelte`; each dimension also offers an "(untagged)" chip (sentinel value in `review.ts`) matching samples with no tag on that dimension, so files headed for `_untagged` can be isolated and bulk-tagged. Verified by `review.test.ts`, `npm run check`, and `npm run build`. |
| Filename search | Implemented | Real-time filename substring filter in `src/lib/stores/review.ts`; verified by `npm run check` and `npm run build`. |
| Sortable columns | Implemented | Sample and tag-dimension sorting in `FileList.svelte`; review rows now use deterministic fixed-height virtualization to avoid stale measurement state across filter changes; verified by `npm run check`, `npm run test`, and `npm run build`. |
| Inline tag editing | Implemented | Reusable tag editor supports enum, multi-enum, and numeric dimensions from typed dimension metadata; tag cells show a hover affordance and tooltip so single-row editing is discoverable. Covered by `TagValueEditor.test.ts`, `npm run check`, and `npm run build`. |
| Bulk tag editor | Implemented | Selection action bar appears from one selected row (single-sample tagging included) and uses typed dimension metadata for all editable enum, multi-enum, and numeric dimensions; row drag selection selects or deselects along a single drag path based on the starting row. Bulk edits go through single typed `set_user_tag_bulk`/`clear_user_tag_bulk` commands, and tag-edit failures are surfaced in the review UI; verified by `npm run check` and `npm run test`. |
| Sample details and conflict decisions | Implemented | Review rows use an info/warning icon button instead of a conflict column. The modal shows file metadata, ML detections, all gathered tag evidence, inline conflict choices, and an Edit Tags section (dimension picker + tag editor) for tagging the sample directly; covered by `SampleDetailsDialog.test.ts`. |
| Major/minor mode detection | Implemented | Filename and embedded key metadata emit separate `Mode` tags; CLAP prompt scoring includes major/minor mode prompts with top-1 mode output. Covered by analyzer heuristic, metadata, and classifier tests. |
| `tags::set_user_tag` command | Implemented | Typed `set_user_tag` command and generated `commands.setUserTag`; covered by `test_user_tag_write_and_clear_preserves_auto_tags`. |
| `tags::clear_user_tag` command | Implemented | Typed `clear_user_tag` command and generated `commands.clearUserTag`; covered by `test_user_tag_write_and_clear_preserves_auto_tags`. |
| Conflict query tests | Implemented | `test_conflict_query_returns_unresolved_auto_tag_conflicts`; conflicts are now computed in Rust from a single bulk tag fetch (`sample_rows`), with parity between bulk and single-sample paths covered by `test_sample_rows_returns_tags_and_conflicts_in_bulk`. |
| Single-sample refresh | Implemented | Typed `get_sample` command; inline edits and conflict resolutions refresh one row instead of reloading the library. |

## Phase 5 — ML Analysis

Goal: ML-based Type and Instrument classification runs as part of analysis.

| Feature | Status | Notes / Verification |
|---|---|---|
| Loop/one-shot classifier | In progress | Runtime uses Essentia TFLite only when `SONOSCOPE_ESSENTIA_LOOP_MODEL` is configured. No fixed-confidence fallback Type is emitted when a real loop model is unavailable. Bundled model selection remains pending. |
| Instrument classifier | In progress | Runtime now supports batched LAION CLAP zero-shot scoring with a hand-maintained prompt map (`mappings/clap_prompts.json`) covering the full shipped Instrument vocabulary, Windows CUDA PyTorch resolution, auto device selection (`cuda` -> `mps` -> `cpu`), CUDA inference tuning, configurable CLAP micro-batches, and CPU retry fallback; real-audio fixture validation remains pending. |
| ML model cache management | Implemented | Rust exposes typed `get_ml_model_status` and `download_ml_model` commands, stores LAION CLAP plus optional Essentia model files under app data, and passes model paths to the Python sidecar. UI shows readiness and a download action in `LibraryBar.svelte`. |
| Analysis source policy | Implemented | `docs/05-analysis-spec.md` defines the method priority for file metadata, waveform, Type, Instrument, Tempo, Key, Mode, and Mood so weak/fallback evidence stays visible as uncertainty instead of being promoted to truth. |
| AudioSet mapping config | Deferred | Kept as legacy config for older AudioSet-style classifiers; active Instrument classification now uses `clap_prompts.json`. |
| Waveform generation | Implemented | Analyzer emits byte-scaled peak amplitude bins; Rust persists them to `samples.waveform_data`. Covered by `test_waveform.py` and `test_audio.py`. Audio decoding is shared across CLAP, loop detection, and waveform stages via a per-batch decode cache (`sonoscope_analyzer/audio.py`) so each file is read once per batch. |
| Waveform DB migration | Deferred | `waveform_data` already exists in `001_init.sql`; no new migration required. |
| ML mapping unit tests | Implemented | `analyzer/tests/test_classifier.py` validates mocked model output mapping, batched CLAP prompt-score mapping, CLAP device selection/fallback, and runtime adapters without loading real model weights. Model backends live in `sonoscope_analyzer/adapters/` (CLAP, Essentia, onset fallback) with shared interfaces in `interfaces.py`, torch helpers in `torch_utils.py`, and Pydantic-validated mapping config loaders. |
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
| Operation history schema | Implemented | Migration `007_organise.sql`: `operation_batches`, `file_operations` (indexed by batch), and `organisation_presets` with seeded system presets. |
| Pattern resolver | Implemented | `src-tauri/src/organise/pattern.rs`: `{Dimension}` placeholders, literal segments, `_untagged` fallback, per-segment sanitization (invalid characters, trailing dots, Windows reserved names). Unit tests in the module. |
| Organise preview command | Implemented | Typed `preview_organise` with optional sample-id scope (used by the UI to respect active review filters); entries flag untagged fallback, in-plan target collisions, and files already at their target. Covered by `src-tauri/tests/organise_tests.rs`. |
| Organise apply command | Implemented | Typed `apply_organise` with move and copy modes. Never overwrites: occupied targets, in-plan collisions, and no-op moves are skipped and counted. Records per-file operations, updates sample paths (move), emits `organise-progress`, and requires copy destinations to be existing folders outside the library. |
| Rollback command | Implemented | Typed `rollback_operation_batch`; move batches only, reverses operations newest-first, restores sample paths, marks the batch `rolled_back`. Covered by `organise_tests.rs`. |
| Preset commands | Implemented | Typed `list/save/delete_organisation_preset`; save validates the pattern and upserts by name. |
| Organise UI | Implemented | `OrganiseView.svelte`: pattern editor with live validation (`src/lib/stores/organise.ts`), preset dropdown with save/delete, move/copy mode with folder picker, debounced preview with count badges and filtered-subset note, confirmation dialog, apply progress and summary. Preview count badges are clickable filters (untagged / name clash / unchanged) so affected files can be inspected; destination collisions are labelled "name clash" to avoid confusion with tag conflicts. |
| History UI | Implemented | `HistoryView.svelte`: batch list (date, mode, pattern, file count, status), rollback confirmation dialog, rollback result summary. |
| File operation tests | Implemented | `src-tauri/tests/organise_tests.rs` (13 temp-directory integration tests: preview, move, copy, filters, collisions, rollback, presets) and `src/lib/stores/organise.test.ts` (pattern validation/tokenizer). `cargo test`, `npm run test`, `npm run check`, `npm run build` all pass. |

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
