<script lang="ts">
  import { AlertTriangle, FileAudio, Pencil, X } from "@lucide/svelte";
  import { commands } from "$lib/bindings/bindings";
  import { Badge, Button } from "$lib/components/ui";
  import { currentLibrary, samples, type SampleRow } from "$lib/stores/library";
  import {
    clearSelection,
    displayTagValues,
    hasConflict,
    selectedSampleIds,
    setSort,
    sortDirection,
    sortKey,
    toggleSelection,
    visibleSamples,
    type SortKey,
  } from "$lib/stores/review";

  const typeOptions = ["loop", "one-shot", "fill", "break", "top-loop", "texture"];
  const instrumentOptions = [
    "kick",
    "snare",
    "hi-hat",
    "clap",
    "cymbal",
    "percussion",
    "bass",
    "guitar",
    "piano",
    "brass",
    "woodwind",
    "strings",
    "chord",
    "pad",
    "synth",
    "lead",
    "vocal",
    "fx",
    "foley",
  ];

  let editing: { sampleId: number; dimension: "Type" | "Instrument"; value: string } | null =
    null;
  let bulkDimension: "Type" | "Instrument" = "Type";
  let bulkValue = "loop";

  function formatBytes(bytes: number | null): string {
    if (bytes == null) return "-";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function sortLabel(key: SortKey): string {
    if ($sortKey !== key) return "";
    return $sortDirection === "asc" ? " asc" : " desc";
  }

  function directoryPath(sample: SampleRow): string {
    const normalizedPath = sample.relative_path.replaceAll("\\", "/");
    const filenameIndex = normalizedPath.lastIndexOf(`/${sample.filename}`);
    if (filenameIndex > 0) return normalizedPath.slice(0, filenameIndex);

    const lastSlash = normalizedPath.lastIndexOf("/");
    if (lastSlash <= 0) return "";
    return normalizedPath.slice(0, lastSlash);
  }

  function optionsForDimension(dimension: "Type" | "Instrument"): string[] {
    return dimension === "Type" ? typeOptions : instrumentOptions;
  }

  function startEditing(sample: SampleRow, dimension: "Type" | "Instrument") {
    const fallback = optionsForDimension(dimension)[0] ?? "";
    editing = {
      sampleId: sample.id,
      dimension,
      value: displayTagValues(sample, dimension)[0] ?? fallback,
    };
  }

  async function saveEditing() {
    if (!editing) return;
    const result = await commands.setUserTag(editing.sampleId, editing.dimension, editing.value);
    if (result.status === "error") {
      console.error("Failed to set tag:", result.error);
      return;
    }
    await refreshSamples();
    editing = null;
  }

  async function clearEditing() {
    if (!editing) return;
    const result = await commands.clearUserTag(editing.sampleId, editing.dimension);
    if (result.status === "error") {
      console.error("Failed to clear tag:", result.error);
      return;
    }
    await refreshSamples();
    editing = null;
  }

  async function applyBulkTag() {
    for (const sampleId of $selectedSampleIds) {
      const result = await commands.setUserTag(sampleId, bulkDimension, bulkValue);
      if (result.status === "error") {
        console.error("Failed to set bulk tag:", result.error);
      }
    }
    await refreshSamples();
    clearSelection();
  }

  async function clearBulkTag() {
    for (const sampleId of $selectedSampleIds) {
      const result = await commands.clearUserTag(sampleId, bulkDimension);
      if (result.status === "error") {
        console.error("Failed to clear bulk tag:", result.error);
      }
    }
    await refreshSamples();
    clearSelection();
  }

  async function refreshSamples() {
    const result = await commands.getSamples();
    if (result.status === "ok") samples.set(result.data);
  }
</script>

<div class="flex min-h-0 flex-1 flex-col bg-card">
  {#if $selectedSampleIds.size > 1}
    <div class="flex h-12 shrink-0 items-center gap-2 border-b bg-muted/40 px-4">
      <span class="text-sm font-medium">{$selectedSampleIds.size} selected</span>
      <select class="h-8 rounded-md border bg-background px-2 text-sm" bind:value={bulkDimension}>
        <option value="Type">Type</option>
        <option value="Instrument">Instrument</option>
      </select>
      <select class="h-8 rounded-md border bg-background px-2 text-sm" bind:value={bulkValue}>
        {#each optionsForDimension(bulkDimension) as value}
          <option value={value}>{value}</option>
        {/each}
      </select>
      <Button size="sm" onclick={applyBulkTag}>Set tag</Button>
      <Button variant="outline" size="sm" onclick={clearBulkTag}>Clear tag</Button>
      <Button variant="ghost" size="sm" onclick={clearSelection}>
        <X />
        Deselect
      </Button>
    </div>
  {/if}

  {#if $visibleSamples.length === 0}
    <div class="flex flex-1 items-center justify-center p-8">
      <div class="flex max-w-sm flex-col items-center text-center">
        <div class="mb-4 flex size-12 items-center justify-center rounded-lg border bg-muted">
          <FileAudio class="size-5 text-muted-foreground" />
        </div>
        <h2 class="text-sm font-medium">
          {$currentLibrary ? "No matching samples" : "Open a library"}
        </h2>
        <p class="mt-1 text-sm text-muted-foreground">
          {$currentLibrary
            ? "Adjust filters or run a scan to populate the review list."
            : "Choose a folder to create or load a Sonoscope library."}
        </p>
      </div>
    </div>
  {:else}
    <div class="min-h-0 flex-1 overflow-auto">
      <table class="w-full table-fixed text-left text-sm">
        <thead class="sticky top-0 z-10 border-b bg-card">
          <tr class="text-xs text-muted-foreground">
            <th class="w-9 px-2 py-2"></th>
            <th class="px-3 py-2 font-medium">
              <button type="button" onclick={() => setSort("filename")}>Sample{sortLabel("filename")}</button>
            </th>
            <th class="w-36 px-3 py-2 font-medium">
              <button type="button" onclick={() => setSort("type")}>Type{sortLabel("type")}</button>
            </th>
            <th class="w-52 px-3 py-2 font-medium">
              <button type="button" onclick={() => setSort("instrument")}>
                Instrument{sortLabel("instrument")}
              </button>
            </th>
            <th class="w-20 px-3 py-2 font-medium">Conflict</th>
            <th class="w-24 px-3 py-2 font-medium">Format</th>
            <th class="w-28 px-3 py-2 font-medium">Size</th>
          </tr>
        </thead>
        <tbody class="divide-y">
          {#each $visibleSamples as sample (sample.id)}
            {@const selected = $selectedSampleIds.has(sample.id)}
            <tr class={selected ? "bg-muted/60 hover:bg-muted/70" : "hover:bg-muted/50"}>
              <td class="px-2 py-2">
                <input
                  type="checkbox"
                  checked={selected}
                  onclick={(event) => toggleSelection(sample.id, event.ctrlKey || event.metaKey)}
                />
              </td>
              <td class="px-3 py-2">
                <div class="truncate font-mono text-xs">{sample.filename}</div>
                {#if directoryPath(sample)}
                  <div class="truncate font-mono text-[11px] text-muted-foreground">
                    {directoryPath(sample)}
                  </div>
                {/if}
              </td>
              <td class="px-3 py-2">
                <button
                  type="button"
                  class="flex max-w-full flex-wrap gap-1 text-left"
                  onclick={() => startEditing(sample, "Type")}
                >
                  {#each displayTagValues(sample, "Type") as value}
                    <Badge variant="secondary">{value}</Badge>
                  {:else}
                    <span class="text-xs text-muted-foreground">-</span>
                  {/each}
                </button>
              </td>
              <td class="px-3 py-2">
                <button
                  type="button"
                  class="flex max-w-full flex-wrap gap-1 text-left"
                  onclick={() => startEditing(sample, "Instrument")}
                >
                  {#each displayTagValues(sample, "Instrument") as value}
                    <Badge variant="outline">{value}</Badge>
                  {:else}
                    <span class="text-xs text-muted-foreground">-</span>
                  {/each}
                </button>
              </td>
              <td class="px-3 py-2">
                {#if hasConflict(sample)}
                  <Badge variant="destructive"><AlertTriangle class="size-3" /> auto</Badge>
                {:else}
                  <span class="text-xs text-muted-foreground">-</span>
                {/if}
              </td>
              <td class="px-3 py-2 text-xs uppercase text-muted-foreground">
                {sample.format ?? "-"}
              </td>
              <td class="px-3 py-2 text-xs text-muted-foreground">
                {formatBytes(sample.size_bytes)}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}

  {#if editing}
    <div class="border-t bg-background p-3">
      <div class="flex items-center gap-2">
        <Pencil class="size-4 text-muted-foreground" />
        <span class="text-sm font-medium">Edit {editing.dimension}</span>
        <select class="h-8 rounded-md border bg-background px-2 text-sm" bind:value={editing.value}>
          {#each optionsForDimension(editing.dimension) as value}
            <option value={value}>{value}</option>
          {/each}
        </select>
        <Button size="sm" onclick={saveEditing}>Save</Button>
        <Button variant="outline" size="sm" onclick={clearEditing}>Clear user tag</Button>
        <Button variant="ghost" size="sm" onclick={() => (editing = null)}>Cancel</Button>
      </div>
    </div>
  {/if}
</div>
