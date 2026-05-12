# Sonoscope — Claude Context

## Project

Desktop app for curating and reorganising audio sample libraries with ML-based auto-tagging. Targets macOS and Windows. Built with Tauri + Svelte (frontend), Rust (core), and a Python sidecar (analysis pipeline).

## Documentation

All design and spec documents live in [`docs/`](docs/):

| File | Contents |
|---|---|
| [01-overview.md](docs/01-overview.md) | App concept, core workflow, key concepts and glossary |
| [02-features.md](docs/02-features.md) | Feature inventory grouped by functional area |
| [03-architecture.md](docs/03-architecture.md) | Component breakdown, data flow, tech stack decisions |
| [04-data-model.md](docs/04-data-model.md) | SQLite schema — all tables, indexes, seed data |
| [05-analysis-spec.md](docs/05-analysis-spec.md) | Python sidecar: pipeline stages, models, IPC protocol |
| [06-ui-spec.md](docs/06-ui-spec.md) | Layout, views, interactions, playback |
| [07-implementation-guide.md](docs/07-implementation-guide.md) | Type safety rules, testing strategy, project setup, implementation phases |

## Enforced implementation rules

### Type safety
- **Rust**: use `sqlx` compile-time query macros; no raw strings for enum-like fields — use Rust enums.
- **TypeScript**: `strict: true` in `tsconfig.json`; no `any`; use only the generated `tauri-specta` bindings to call Tauri commands — never raw `invoke<any>()`.
- **Python**: type hints on all public functions; `ty check` must pass; `ruff check` and `ruff format` enforced in CI; all IPC request/response models must be Pydantic v2 classes.

### Testing
- New logic ships with tests. No exceptions.
- The Python analyser has the strictest requirement: heuristic rules must have parametrized unit tests; ML mapping logic must be unit-tested with mocked model output.
- Do not load ML models in unit tests — mock the model output and test only the mapping.
- Integration tests that load real models are marked `@pytest.mark.integration` and excluded from fast CI runs.

### Database
- Schema changes go in a new numbered migration file under `src-tauri/migrations/`. Never modify an existing migration.
- All DB access goes through Core (Rust). The UI never reads or writes the database directly.

### IPC
- Tauri command signatures are the source of truth for the Rust↔UI contract. Run `tauri-specta` codegen after any command change; commit the generated bindings.
- The sidecar protocol (stdin/stdout newline-delimited JSON) is defined by Pydantic models in `sidecar/sonoscope_analyzer/protocol.py`. Change the protocol there first, then update the Rust consumer.

### Generated files
- `src/lib/bindings/` is auto-generated — do not hand-edit.
- `sidecar/mappings/*.json` are hand-maintained configuration files — do not generate them.

### Phased delivery
- Implement in the phases defined in `07-implementation-guide.md`. Each phase must leave the codebase in a working, testable state.
- No cross-phase work without completing the current phase's checklist.
