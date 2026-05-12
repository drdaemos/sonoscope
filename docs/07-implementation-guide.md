# Sonoscope — Implementation Guide

## Principles

- **Type safety everywhere**: Rust for Core (enforced by the compiler), TypeScript strict mode for the UI, typed Python with Pydantic for the sidecar protocol. No raw `any`, no untyped JSON boundaries.
- **Small, shippable chunks**: each phase leaves the codebase in a working, testable state. No phase should take more than a week of focused work.
- **Tests before merge**: new logic ships with tests. The analyser pipeline has the highest test coverage requirement.
- **IPC contract is the seam**: the Tauri command signatures and the sidecar JSON protocol are the two integration boundaries. Both are typed and tested independently.

---

## Type safety strategy

### Rust (Core)
- Use `sqlx` with compile-time query validation (`query!` / `query_as!` macros, offline mode for CI).
- All domain types are Rust enums and structs — no raw strings for status fields, source labels, or operation types.
- Tauri command arguments and return types are derived via `specta` (`tauri-specta`), which auto-generates TypeScript bindings from Rust signatures. This means the IPC layer has a single source of truth in Rust.

### TypeScript (UI)
- `tsconfig.json` with `strict: true`, `noUncheckedIndexedAccess: true`.
- All Tauri command call-sites use the generated bindings from `tauri-specta` — no manual `invoke<any>()`.
- Svelte component props typed explicitly; no implicit `any` in event handlers.

### Python (Sidecar)
- All public functions annotated with type hints.
- `mypy` in strict mode (`--strict`) run as part of CI.
- Request/response protocol models defined with **Pydantic v2** — parsing and validation happen at the IO boundary. Internal functions receive typed dataclasses, not raw dicts.

---

## Testing strategy

### Rust — Core
| Layer | Approach | Tools |
|---|---|---|
| DB access | Integration tests against a real in-memory SQLite instance | `sqlx` test helpers, `tempfile` |
| File operations | Integration tests using a temp directory tree | `tempfile` |
| Scan logic | Unit tests with mock file trees | std |
| Tauri commands | Integration tests via `tauri::test` harness | `tauri::test` |

Run with: `cargo test`

### Python — Analysis pipeline
The sidecar has the highest test coverage requirement. Tests are split into three layers:

| Layer | Approach | Tools |
|---|---|---|
| Heuristics | Parametrized unit tests: input filename → expected tags. Cover common tokens, edge cases, conflicting tokens, empty paths. | `pytest`, `pytest-parametrize` |
| Metadata extraction | Unit tests with fixture files covering each supported format (WAV, FLAC, MP3, AIFF with known embedded tags). | `pytest`, fixture files in `tests/fixtures/` |
| ML model output mapping | Unit tests that mock the raw model output (confidence vectors) and test only the mapping logic (AudioSet class → Sonoscope dimension value). No model loaded. | `pytest`, `unittest.mock` |
| Full pipeline integration | Integration tests using a small set of labeled fixture audio files (~20 files covering each instrument + type category). Assert that the pipeline produces the expected top-1 tag. Model is loaded; tests are marked `@pytest.mark.integration` and excluded from fast CI runs. | `pytest`, real model weights |
| IPC protocol | Unit tests for request parsing and response serialisation via Pydantic models. | `pytest` |

Run fast tests with: `pytest -m "not integration"`
Run all tests with: `pytest`

**Fixture audio files** live in `sidecar/tests/fixtures/` and are committed to the repo. They should be short (< 2s), low bit-rate, and cover: kick, snare, hi-hat, bass, pad, synth, loop, one-shot. A `fixtures/manifest.json` records the expected tags for each file, used by integration tests.

### Svelte — UI
| Layer | Approach | Tools |
|---|---|---|
| Component logic | Unit tests for non-trivial reactive logic (filter state, pattern validation, conflict detection queries) | Vitest |
| Component rendering | Component tests for critical UI pieces (file list row, tag chip, conflict panel) | Vitest + Testing Library |
| E2E | Smoke tests covering the core workflow (open library → scan → edit tag → organise) | Playwright |

Run with: `npm run test` (Vitest), `npm run test:e2e` (Playwright)

---

## Project structure

```
sonoscope/
├── src-tauri/               # Rust — Tauri Core
│   ├── src/
│   │   ├── main.rs
│   │   ├── commands/        # Tauri command handlers (thin layer, delegates to lib)
│   │   ├── library/         # Library open/create, scan orchestration
│   │   ├── db/              # SQLite access layer (sqlx)
│   │   ├── analysis/        # Sidecar process management, IPC protocol types
│   │   └── files/           # File operations, rollback
│   ├── tests/               # Integration tests
│   ├── build.rs             # tauri-specta type generation
│   └── Cargo.toml
│
├── src/                     # Svelte UI
│   ├── lib/
│   │   ├── components/      # shadcn-svelte + custom components
│   │   ├── stores/          # Svelte stores for app state
│   │   ├── bindings/        # Auto-generated Tauri command bindings (do not edit)
│   │   └── types/           # Additional TS types
│   ├── routes/              # Top-level views (Review, Organise, History)
│   └── app.html
│
├── sidecar/                 # Python analysis pipeline
│   ├── sonoscope_analyzer/
│   │   ├── __init__.py
│   │   ├── main.py          # Stdin/stdout IPC loop
│   │   ├── protocol.py      # Pydantic request/response models
│   │   ├── metadata.py      # Stage 1: metadata extraction
│   │   ├── heuristics.py    # Stage 2: filename/path heuristics
│   │   ├── classifier.py    # Stage 3: ML classification
│   │   ├── waveform.py      # Waveform data generation
│   │   └── mappings/
│   │       ├── heuristic_tokens.json   # Token → tag mappings
│   │       └── audioset_map.json       # AudioSet class → Sonoscope value
│   ├── tests/
│   │   └── fixtures/        # Labeled audio files + manifest.json
│   ├── pyproject.toml
│   └── sonoscope_analyzer.spec  # PyInstaller spec
│
└── docs/
```

---

## Project setup

### Prerequisites
- **Rust**: install via [rustup](https://rustup.rs/), stable toolchain
- **Node.js**: 20 LTS or later
- **Python**: 3.11 or later
- **Tauri prerequisites**: platform-specific system dependencies per [Tauri docs](https://tauri.app/start/prerequisites/)
- **uv**: Python package manager (`pip install uv` or via installer)

### Initial scaffold

```bash
# 1. Scaffold Tauri + Svelte + Vite
npm create tauri-app@latest sonoscope -- --template svelte-ts
cd sonoscope

# 2. Install frontend dependencies
npm install
npm install -D @tanstack/svelte-table @tanstack/svelte-virtual
npm install -D tailwindcss @tailwindcss/vite
# shadcn-svelte init (interactive)
npx shadcn-svelte@latest init

# 3. Add Rust dependencies (Cargo.toml)
# sqlx, tauri-specta, serde, serde_json, tokio, uuid, xxhash-rust

# 4. Set up Python sidecar
cd sidecar
uv init
uv add pydantic mutagen soundfile librosa essentia-tensorflow panns-inference
uv add --dev pytest ty pytest-mock ruff
```

### Running in development

```bash
# Terminal 1 — Tauri dev (starts Vite + Rust watcher)
npm run tauri dev

# Terminal 2 — Python sidecar (development mode, Tauri spawns this automatically
#               when configured as a sidecar, but can also be run standalone)
cd sidecar && uv run python -m sonoscope_analyzer --dev
```

In dev mode, the sidecar reads from stdin and writes to stdout as normal. Pass `--dev` to enable verbose logging to stderr (which Tauri captures separately).

### Running tests

```bash
# Rust
cargo test

# Python (fast, no model loading)
cd sidecar && uv run pytest -m "not integration"

# Python (full, including ML integration tests — slow)
cd sidecar && uv run pytest

# UI
npm run test

# E2E
npm run test:e2e
```

### Building for production

```bash
# 1. Build the Python sidecar into a self-contained binary
cd sidecar && uv run pyinstaller sonoscope_analyzer.spec
# Output: sidecar/dist/sonoscope-analyzer(.exe on Windows)

# 2. Copy sidecar binary to Tauri's sidecar directory
# (configured in tauri.conf.json under externalBin)

# 3. Build the Tauri app
npm run tauri build
```

### CI (GitHub Actions)

Two jobs, matrix: `[macos-latest, windows-latest]`

1. **Test**
   - `cargo test`
   - `cd sidecar && uv run ruff check sonoscope_analyzer`
   - `cd sidecar && uv run ruff format --check sonoscope_analyzer`
   - `cd sidecar && uv run ty check sonoscope_analyzer`
   - `cd sidecar && uv run pytest -m "not integration"`
   - `npm run test`

2. **Build** (on `main` only)
   - Full sidecar + Tauri production build

---

## Implementation phases

### Phase 0 — Scaffold (setup)
**Goal:** empty app runs; all tooling works; CI passes.

- [ ] Tauri + Svelte + Vite project created
- [ ] Tailwind + shadcn-svelte configured
- [ ] TanStack Table + Virtual installed
- [ ] `tauri-specta` wired up; `npm run generate-bindings` produces output
- [ ] Python sidecar project created with `uv`; Pydantic installed; `mypy --strict` passes on empty module
- [ ] `sqlx` added to Rust; migrations directory created; `sqlx-cli` installed
- [ ] GitHub Actions CI passes (nothing to test yet, just build checks)

### Phase 1 — Library + Discovery
**Goal:** user can open a folder, the app discovers audio files, and they appear in a basic list.

- [ ] DB schema: `library_meta`, `samples` table, migration 001
- [ ] Rust: `library::open` — initialise or load library DB
- [ ] Rust: `library::discover` — recursive walk, atomic transaction, cancellable
- [ ] Tauri commands: `open_library`, `start_discovery`; bindings generated
- [ ] UI: library selector in top bar; file list (filename + path only, no tags yet)
- [ ] UI: discovery progress indicator in top bar
- **Tests**: DB integration (open creates schema), discovery (temp dir tree), scan cancellation

### Phase 2 — Sidecar + Heuristic Analysis
**Goal:** files are analysed by filename heuristics; tags appear in the list.

- [ ] Python: Pydantic protocol models (request, response, tag)
- [ ] Python: `heuristics.py` with token config loaded from `heuristic_tokens.json`
- [ ] Python: `metadata.py` with `mutagen` + `soundfile`
- [ ] Python: `main.py` IPC loop (stdin/stdout, newline-delimited JSON)
- [ ] Rust: sidecar process manager (spawn, keep-alive, send/receive)
- [ ] DB schema: `dimensions`, `dimension_values`, `tags`; seed data; migration 002
- [ ] Rust: analysis orchestrator — queue pending files, dispatch to sidecar, write results
- [ ] UI: tag columns in file list (Type + Instrument); chips per row
- [ ] UI: analysis progress badge on Review tab
- **Tests**: heuristic parametrized suite (50+ cases); metadata fixture files; IPC protocol round-trip; Rust sidecar manager (mock sidecar process)

### Phase 3 — Review UI
**Goal:** user can fully review and edit tags; filtering and search work.

- [ ] UI: filter sidebar with dimension chips + counts
- [ ] UI: filename search
- [ ] UI: sortable columns
- [ ] UI: inline tag editing popover (single row)
- [ ] UI: bulk tag editor action bar
- [ ] UI: conflict indicator + inline conflict panel
- [ ] Rust: `tags::set_user_tag`, `tags::clear_user_tag` commands
- [ ] Rust: conflict detection query
- **Tests**: filter state logic (Vitest); tag editing component (Testing Library); conflict query (DB integration)

### Phase 4 — ML Analysis
**Goal:** ML-based Type and Instrument classification runs as part of analysis.

- [ ] Python: `classifier.py` — Essentia loop detector + PANNs CNN14 integration
- [ ] Python: `audioset_map.json` mapping file
- [ ] Python: waveform generation (`waveform.py`)
- [ ] DB schema: `waveform_data` column on `samples`; migration 003
- [ ] Integration test suite with fixture audio files (`@pytest.mark.integration`)
- [ ] Verify end-to-end: scan a test library, check DB tags match fixture manifest
- **Tests**: ML mapping unit tests (mocked model output); integration fixture suite

### Phase 5 — Audio Playback
**Goal:** user can play samples in-app with waveform display.

- [ ] UI: playback footer (play/pause, waveform canvas, seek, timestamp, volume)
- [ ] UI: double-click row to load sample
- [ ] UI: Space key to play/pause; ↑/↓ to move selection
- [ ] Tauri: asset protocol configured for local audio file serving
- [ ] UI: waveform drawn from `waveform_data` blob retrieved via Tauri command
- **Tests**: playback store logic (Vitest); waveform rendering component

### Phase 6 — Organise + History
**Goal:** user can reorganise files and roll back.

- [ ] DB schema: `operation_batches`, `file_operations`; migration 004
- [ ] Rust: pattern resolver (parse `{Dimension}` placeholders, resolve against sample tags, handle `_untagged`)
- [ ] Rust: `organise::preview` command
- [ ] Rust: `organise::apply` command (move + copy modes)
- [ ] Rust: `organise::rollback` command
- [ ] UI: Organise step (pattern editor, preset dropdown, mode selector, preview list, apply button)
- [ ] UI: History step (batch list, rollback button, confirmation dialog)
- **Tests**: pattern resolver (unit, edge cases: missing tags, multi-value dimensions, special characters in values); file ops integration (temp dir); rollback correctness

### Phase 7 — Settings + Polish
**Goal:** dimension management, preset management, edge case handling.

- [ ] UI: Settings panel — add/remove custom dimensions and values
- [ ] UI: Settings panel — manage organisation presets
- [ ] Rust: `dimensions::create`, `dimensions::delete` (with in-use guard)
- [ ] Handle `analysis_status = analysing` reset on startup
- [ ] Handle sidecar crash + restart
- [ ] E2E smoke tests (Playwright): open library → scan → edit tag → organise → rollback
- [ ] Performance pass: file list with 10k rows, analysis queue throughput

---

## Conventions

### Git
- Branch per phase: `phase/01-library-discovery`, `phase/02-sidecar-heuristics`, etc.
- Commits are small and buildable. No "WIP" commits on main.
- PR per phase; CI must pass before merge.

### Migrations
- SQL migration files in `src-tauri/migrations/` named `001_init.sql`, `002_tags.sql`, etc.
- Run automatically on library open via `sqlx::migrate!()`.
- Never modify an existing migration — add a new one.

### Generated files
- `src/lib/bindings/` is auto-generated by `tauri-specta`; do not hand-edit.
- `sidecar/mappings/` JSON files are hand-maintained configuration, not generated.
