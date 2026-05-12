# Sonoscope — UI Spec

## Global layout

```
┌─────────────────────────────────────────────────────────────────┐
│  [My Library ▾]     Review         Organise        History      │  top bar
├────────────┬────────────────────────────────────────────────────┤
│            │                                                    │
│  Filter    │                  main panel                        │
│  sidebar   │          (content depends on active step)          │
│            │                                                    │
│            │                                                    │
│            │                                                    │
│            │                                                    │
│            ├────────────────────────────────────────────────────┤
│            │  playback bar (footer)                             │
└────────────┴────────────────────────────────────────────────────┘
```

The sidebar and footer are persistent across all steps. The top bar controls which step's content is shown in the main panel.

---

## Top bar

**Library selector (left)**
A collapsible select showing the name of the currently open library (derived from the root folder name). Clicking it expands a dropdown listing recently opened libraries and an "Open folder…" option that triggers the OS folder picker. On first launch the selector prompts to open a library.

**Workflow steps (right of selector)**
Three step tabs: **Review**, **Organise**, **History**. These are tabs, not a forced linear wizard — the user can switch freely between them at any time. The active step is highlighted. A step may show a badge (e.g., pending analysis count on Review, unrolled batch count on History).

Scanning / analysis status is shown as a subtle progress indicator embedded in the top bar (e.g., a thin progress bar under the step tabs, or a spinner with a "Analysing 1,203 / 4,500" label). It does not block navigation.

---

## Filter sidebar

Always visible. Shows the universe of tag values present in the current library, grouped by dimension. Selecting values filters the file list in the Review step.

```
  ┌─ Type ──────────────────┐
  │  ● loop    ○ one-shot   │  ← inline chips, filled = active filter
  └─────────────────────────┘
  ┌─ Instrument ────────────┐
  │  ○ kick   ○ snare       │
  │  ○ hi-hat ○ bass        │
  │  ○ pad    ○ synth  …    │
  └─────────────────────────┘
  ┌─ Key ───────────────────┐
  │  ○ C   ○ C#  ○ D   …   │
  └─────────────────────────┘
  ┌─ Tempo ─────────────────┐
  │  [  min BPM ] [ max ]   │  ← range input for numeric dimension
  └─────────────────────────┘

  [ ⚠ Conflicts only ]       ← toggle to show only conflicted samples
  [ ○ Unanalysed only ]      ← toggle to show pending files
```

**Behaviour:**
- Chips are auto-populated from tag values present in the library. Values with zero samples are hidden.
- Within a dimension, selecting multiple chips = OR (show samples tagged with any of them).
- Across dimensions, active filters combine with AND.
- Chip counts (e.g. "kick (234)") show how many samples match. Counts update as other filters are applied.
- Filters persist while navigating between steps.
- A "Clear filters" link appears when any filter is active.

---

## Step: Review

The primary working view.

```
  [ Search filename… ]                           [ 3 selected  ▾ Edit tags ]

  filename          dur    Type      Instrument      Key    ⚠
  ──────────────────────────────────────────────────────────
  punchy_kick.wav   0:01   one-shot  kick            –
  loop_bass_110.wav 2:03   loop      bass            A      ⚠
  pad_Cmaj.wav      4:00   loop      pad  synth       C
  …
```

### File list

Each row represents one sample. Columns:

| Column | Content |
|---|---|
| filename | basename of the file; clicking selects the row, double-click plays |
| dur | duration formatted as M:SS |
| one column per active dimension | the sample's tags for that dimension, shown as small chips |
| ⚠ | conflict indicator; shown if the sample has at least one unresolved tag conflict |

Rows with `analysis_status = pending` are shown with a muted style and a small spinner in place of tag chips.

**Sorting:** clicking any column header sorts ascending; clicking again sorts descending.

**Search:** the text field at the top filters by filename substring in real time.

**Row selection:**
- Single click = select (clears previous selection)
- Ctrl/Cmd + click = add/remove from selection
- Shift + click = range select
- Clicking empty space = deselect all

### Inline tag editing (single row)

Clicking a tag chip cell on a row opens a dimension popover for that cell:
- Shows current tags for that dimension on that sample
- Allows adding values (select from a list of dimension_values, with a "create new value" option)
- Allows removing individual values (× on each chip)
- Auto-saves on close; writes a `user`-source tag to the database

### Conflict panel

Clicking the ⚠ indicator on a row expands an inline conflict panel below that row:
- Lists each conflicted dimension
- Shows competing values side by side with their source and confidence
- "Use this" button next to each value writes a `user`-source tag resolving the conflict
- "Dismiss" marks the conflict as acknowledged without resolution (hidden from the ⚠ count)

### Bulk tag editor

When 2+ rows are selected, an action bar appears above the list:

```
  3 selected    [ Set dimension ▾ ]  [ Clear dimension ▾ ]   [ Deselect ]
```

- **Set dimension**: picks a dimension, then a value; writes that tag to all selected samples (adds to existing tags, does not replace)
- **Clear dimension**: picks a dimension; removes all tags on that dimension from selected samples (user-source removal)

---

## Step: Organise

```
  Pattern   [ {Type} / {Instrument} ]   [+ Add preset]   [Presets ▾]

  Mode      ( ● ) Move within library
            (   ) Copy to…  [ Choose folder… ]   /path/to/destination

  ──────────────────────────────────────────────────────────────────
  Preview                                          2,450 files

  Drums/Kicks/punchy_kick.wav          →   one-shot/kick/punchy_kick.wav
  Loops/loop_bass_110.wav              →   loop/bass/loop_bass_110.wav
  pad_Cmaj.wav                         →   loop/pad/pad_Cmaj.wav
  (no Type tag) weird_sound.wav        →   _untagged/weird_sound.wav
  ──────────────────────────────────────────────────────────────────

  [ Apply ]
```

### Pattern editor

A text input accepting a path template with `{DimensionName}` placeholders. Dimension names are validated against known dimensions; unknown names are highlighted in red. An autocomplete dropdown suggests available dimensions as the user types `{`.

A "Presets" dropdown loads a saved preset into the pattern field. The "Add preset" button saves the current pattern as a new preset (prompting for a name, defaulting to the dimension order, e.g. "Type / Instrument").

### Mode selector

Two radio options: **Move within library** and **Copy to…**. Copy mode requires selecting a destination folder via the OS folder picker.

### Preview

A scrollable list of `original path → new path` for every file in the library (respecting active sidebar filters — if filters are active, only the matching subset is shown and a note indicates this). Files without a required tag are shown with their `_untagged/` destination.

The preview is computed on demand (a "Preview" button or auto-computed after a short debounce when the pattern changes). File count summary shown above the list.

### Apply

Disabled until a valid pattern is entered and (for copy mode) a destination is chosen. Clicking Apply shows a confirmation dialog summarising the operation (N files, mode, destination), then executes. Progress is shown inline; on completion a summary ("2,450 files moved, 3 skipped") replaces the preview.

---

## Step: History

A log of all completed operation batches, newest first.

```
  2026-05-14  14:32   Move   {Type}/{Instrument}   2,450 files   [ Roll back ]
  2026-05-12  09:10   Move   {Instrument}/{Type}     312 files   Rolled back
```

Each row shows: date/time, mode, pattern used, file count, and status. Batches with `status = completed` have a "Roll back" button. Batches already rolled back are shown as greyed out with a "Rolled back" label.

Clicking "Roll back" shows a confirmation dialog, then executes the reversal. Progress shown inline.

---

## Footer — Playback bar

```
  ▶  punchy_kick.wav     ▁▃▇▅▂▁▂▄▇▆▃▁   0:00 / 0:01   🔊 ──●─────────
```

- **Play/pause button** (left)
- **Filename** of the currently loaded sample
- **Loop button** — enables looping of the loaded sample (auto-selected based on the metadata)
- **Waveform** — static waveform image generated from the audio file, rendered once and cached; seekable by clicking; squiggly line, not chopped
- **Seek position** — overlaid on the waveform as a playhead
- **Timestamp** — current position / total duration
- **Volume slider** (right)

Playback is triggered by double-clicking a row or pressing Space when a row is selected. Pressing Space again pauses. The currently playing row is highlighted in the list.

The waveform is rendered in the sidecar (using `librosa` or `soundfile`) and returned as a compact array of amplitude values (e.g., 400 points). Core caches the result in the database (`samples` table, nullable `waveform_data` blob column). This column is not in the current data model and should be added to `04-data-model.md`.

---

## Keyboard shortcuts

| Key | Action |
|---|---|
| Space | Play / pause selected sample |
| ↑ / ↓ | Move selection up / down in list |
| Ctrl/Cmd + A | Select all visible rows |
| Escape | Deselect all / close open popover |
| Delete / Backspace | (no destructive action — reserved for future use) |

---

## Open decisions

- **Dimension column visibility**: which dimensions appear as columns in the file list by default, and can the user show/hide columns? Suggested default: Type + Instrument always visible; others togglable via a column picker.

## Resolved decisions

- **Waveform rendering**: generated by the sidecar during analysis (using `librosa` or `soundfile`) as a compact float array; stored in the database; frontend draws it directly. Avoids audio decoding in the WebView.
- **AIFF playback on Windows**: not a concern for this project's scope.
