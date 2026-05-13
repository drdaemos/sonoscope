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
| Library selector UI | In progress | `LibraryBar.svelte` now uses shared UI primitives inside the Phase 2 app shell. |
| Basic file list UI | In progress | `FileList.svelte` shows filename, path, format, size, and status using shared UI primitives. |

## Phase 2 — UI Foundation

Goal: introduce the app shell and design system foundation before feature-heavy UI work continues.

| Feature | Status | Notes / Verification |
|---|---|---|
| Design tokens | Implemented | Manual shadcn-svelte CSS variables in `src/app.css`; `npm run build` passes. |
| Base app shell | Implemented | Top bar, workflow tabs, sidebar, main panel, and footer are present in `App.svelte`. |
| Core UI primitives | In progress | Button, badge, card, input, separator, and tabs primitives exist under `src/lib/components/ui/`. |
| Workflow view states | Implemented | Review, Organise, and History view states exist with placeholders for unavailable workflows. |
| Phase 1 UI cleanup | In progress | Library controls and file list now use shared primitives; table remains intentionally basic. |
| Component usage conventions | Not started | Needs a short docs section once primitive set stabilizes. |
| UI verification | Implemented | `npm run check` and `npm run build` pass. |

## Phase 3 — Sidecar + Heuristic Analysis

Goal: files are analyzed by filename heuristics; tags appear in the list.

| Feature | Status | Notes / Verification |
|---|---|---|
| Pydantic IPC protocol models | Implemented | `analyzer/tests/test_protocol.py`; `uv run pytest`. |
| Analyzer stdin/stdout loop | In progress | Basic loop exists; `process_request` currently returns no tags. |
| Heuristic token config | Not started | Planned `analyzer/sonoscope_analyzer/mappings/heuristic_tokens.json`. |
| Filename/path heuristics | Not started | Requires parametrized unit coverage. |
| Metadata extraction | Not started | Requires fixture-based tests; no model loading. |
| Rust sidecar process manager | Not started | Should use a mock sidecar process in tests. |
| Tags schema migration | Not started | Planned `002_tags.sql` for dimensions, values, and tags. |
| Seed system dimensions/values | Not started | Type and Instrument are minimum required for Phase 3 analysis UI. |
| Analysis orchestrator | Not started | Queue pending samples, dispatch to analyzer, persist results. |
| Tag columns in file list | Not started | Type + Instrument chips per row using Phase 2 components. |
| Analysis progress badge | Not started | Review tab/top-bar progress indicator. |

## Phase 4 — Review UI

Goal: user can review and edit tags; filtering and search work.

| Feature | Status | Notes / Verification |
|---|---|---|
| Filter sidebar | Not started | Dimension chips with counts. |
| Filename search | Not started | Real-time filename substring filter. |
| Sortable columns | Not started | Include tag dimension columns. |
| Inline tag editing | Not started | Single-row editor, user-source tags. |
| Bulk tag editor | Not started | Selection action bar. |
| Conflict indicator and panel | Not started | Query-time conflict detection. |
| `tags::set_user_tag` command | Not started | Must use typed Rust command and generated binding. |
| `tags::clear_user_tag` command | Not started | Must preserve auto-tags. |
| Conflict query tests | Not started | DB integration coverage required. |

## Phase 5 — ML Analysis

Goal: ML-based Type and Instrument classification runs as part of analysis.

| Feature | Status | Notes / Verification |
|---|---|---|
| Loop/one-shot classifier | Not started | Essentia integration planned. |
| Instrument classifier | Not started | PANNs CNN14 mapping planned. |
| AudioSet mapping config | Not started | Planned `audioset_map.json`. |
| Waveform generation | Not started | Compact amplitude array for playback UI. |
| Waveform DB migration | Deferred | `waveform_data` already exists in `001_init.sql`; verify whether migration 003 remains needed. |
| ML mapping unit tests | Not started | Mock model output only. |
| Integration fixture suite | Not started | Mark with `@pytest.mark.integration`. |
| End-to-end analysis verification | Not started | Scan fixture library and compare DB tags to manifest. |

## Phase 6 — Audio Playback

Goal: user can play samples in-app with waveform display.

| Feature | Status | Notes / Verification |
|---|---|---|
| Playback footer UI | Not started | Play/pause, waveform, seek, timestamp, volume. |
| Row double-click playback | Not started | Loads selected sample. |
| Keyboard playback controls | Not started | Space and row navigation. |
| Local audio asset protocol | Not started | Tauri serving for local audio files. |
| Waveform rendering component | Not started | Draws cached waveform data. |
| Playback store tests | Not started | Vitest required. |

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
