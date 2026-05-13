<script lang="ts">
  import { Select as SelectPrimitive } from "bits-ui";
  import { createVirtualizer } from "@tanstack/svelte-virtual";
  import { untrack } from "svelte";
  import { get } from "svelte/store";
  import { AlertTriangle, FileAudio, Pencil, X } from "@lucide/svelte";
  import { commands } from "$lib/bindings/bindings";
  import {
    Badge,
    Button,
    Checkbox,
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    Table,
    TableBody,
    TableCell,
    TableHead,
    TableHeader,
    TableRow,
  } from "$lib/components/ui";
  import { currentLibrary, samples, type SampleRow } from "$lib/stores/library";
  import {
    clearSelection,
    displayTags,
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

  const ROW_HEIGHT = 44;
  const INTERACTIVE_ROW_SELECTOR = "button, a, input, [role='button'], [data-row-action]";

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

  let editing = $state<{ sampleId: number; dimension: "Type" | "Instrument"; value: string } | null>(null);
  let bulkDimension = $state<"Type" | "Instrument">("Type");
  let bulkValue = $state("loop");
  let expandedConflictSampleId = $state<number | null>(null);
  let scrollContainerRef = $state<HTMLDivElement | null>(null);
  let selectedCount = $derived($selectedSampleIds.size);

  const virtualizer = createVirtualizer({
    count: 0,
    getScrollElement: () => scrollContainerRef,
    estimateSize: () => ROW_HEIGHT,
    overscan: 20,
  });

  $effect(() => {
    const count = $visibleSamples.length;
    const el = scrollContainerRef;
    untrack(() => {
      get(virtualizer).setOptions({
        count,
        getScrollElement: () => el,
        estimateSize: () => ROW_HEIGHT,
        overscan: 20,
      });
    });
    el?.scrollTo(0, 0);
  });

  $effect(() => {
    const options = optionsForDimension(bulkDimension);
    if (!options.includes(bulkValue)) {
      bulkValue = options[0] ?? "";
    }
  });

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
    const sampleIds = [...$selectedSampleIds];
    for (const sampleId of sampleIds) {
      const result = await commands.setUserTag(sampleId, bulkDimension, bulkValue);
      if (result.status === "error") {
        console.error("Failed to set bulk tag:", result.error);
      }
    }
    await refreshSamples();
    clearSelection();
  }

  async function clearBulkTag() {
    const sampleIds = [...$selectedSampleIds];
    for (const sampleId of sampleIds) {
      const result = await commands.clearUserTag(sampleId, bulkDimension);
      if (result.status === "error") {
        console.error("Failed to clear bulk tag:", result.error);
      }
    }
    await refreshSamples();
    clearSelection();
  }

  async function resolveConflict(sampleId: number, dimension: string, value: string) {
    const result = await commands.setUserTag(sampleId, dimension, value);
    if (result.status === "error") {
      console.error("Failed to resolve conflict:", result.error);
      return;
    }
    expandedConflictSampleId = null;
    await refreshSamples();
  }

  function toggleConflictPanel(sampleId: number) {
    expandedConflictSampleId = expandedConflictSampleId === sampleId ? null : sampleId;
  }

  function selectRow(sampleId: number, event: MouseEvent) {
    if (isInteractiveRowTarget(event.target)) return;
    toggleSelection(sampleId, event.ctrlKey || event.metaKey);
  }

  function isInteractiveRowTarget(target: EventTarget | null): boolean {
    return target instanceof HTMLElement && target.closest(INTERACTIVE_ROW_SELECTOR) !== null;
  }

  async function refreshSamples() {
    const result = await commands.getSamples();
    if (result.status === "ok") samples.set(result.data);
  }
</script>

<div class="flex min-h-0 flex-1 flex-col bg-card">
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
    <div class="relative min-h-0 flex-1">
      <div class="h-full overflow-auto" bind:this={scrollContainerRef}>
        <Table class="table-fixed">
          <TableHeader class="sticky top-0 z-10 bg-card">
            <TableRow class="text-xs text-muted-foreground">
              <TableHead class="w-9"></TableHead>
              <TableHead>
                <button type="button" onclick={() => setSort("filename")}>Sample{sortLabel("filename")}</button>
              </TableHead>
              <TableHead class="w-36">
                <button type="button" onclick={() => setSort("type")}>Type{sortLabel("type")}</button>
              </TableHead>
              <TableHead class="w-52">
                <button type="button" onclick={() => setSort("instrument")}>
                  Instrument{sortLabel("instrument")}
                </button>
              </TableHead>
              <TableHead class="w-20">Conflict</TableHead>
              <TableHead class="w-24">Format</TableHead>
              <TableHead class="w-28">Size</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {@const items = $virtualizer.getVirtualItems()}
            {#if items.length > 0}
              <tr style="height: {items[0]?.start ?? 0}px"></tr>
              {#each items as virtualRow (virtualRow.key)}
                {@const sample = $visibleSamples[virtualRow.index]!}
                {@const selected = $selectedSampleIds.has(sample.id)}
                <TableRow
                  data-state={selected ? "selected" : undefined}
                  class="h-[44px]"
                  onclick={(event) => selectRow(sample.id, event)}
                >
                  <TableCell class="py-1.5">
                    <Checkbox
                      aria-label={`Select ${sample.filename}`}
                      checked={selected}
                      onCheckedChange={() => toggleSelection(sample.id, true)}
                    />
                  </TableCell>
                  <TableCell class="py-1.5">
                    <div class="truncate font-mono text-xs leading-tight">{sample.filename}</div>
                    {#if directoryPath(sample)}
                      <div class="truncate font-mono text-[11px] leading-tight text-muted-foreground">
                        {directoryPath(sample)}
                      </div>
                    {/if}
                  </TableCell>
                  <TableCell class="py-1.5">
                    <button
                      type="button"
                      class="flex max-w-full flex-wrap gap-1 text-left"
                      onclick={() => startEditing(sample, "Type")}
                    >
                      {#each displayTags(sample, "Type") as tag}
                        <Badge variant={tag.is_primary ? "soft" : "outline"}>
                          {tag.value}
                        </Badge>
                      {:else}
                        <span class="text-xs text-muted-foreground">-</span>
                      {/each}
                    </button>
                  </TableCell>
                  <TableCell class="py-1.5">
                    <button
                      type="button"
                      class="flex max-w-full flex-wrap gap-1 text-left"
                      onclick={() => startEditing(sample, "Instrument")}
                    >
                      {#each displayTags(sample, "Instrument") as tag}
                        <Badge variant={tag.is_primary ? "soft" : "outline"}>
                          {tag.value}
                        </Badge>
                      {:else}
                        <span class="text-xs text-muted-foreground">-</span>
                      {/each}
                    </button>
                  </TableCell>
                  <TableCell class="py-1.5">
                    {#if hasConflict(sample)}
                      <button type="button" onclick={() => toggleConflictPanel(sample.id)}>
                        <Badge variant="destructive"><AlertTriangle class="size-3" /> auto</Badge>
                      </button>
                    {:else}
                      <span class="text-xs text-muted-foreground">-</span>
                    {/if}
                  </TableCell>
                  <TableCell class="py-1.5 text-xs uppercase text-muted-foreground">
                    {sample.format ?? "-"}
                  </TableCell>
                  <TableCell class="py-1.5 text-xs text-muted-foreground">
                    {formatBytes(sample.size_bytes)}
                  </TableCell>
                </TableRow>
                {#if expandedConflictSampleId === sample.id}
                  <TableRow class="bg-muted/40 hover:bg-muted/40">
                    <TableCell></TableCell>
                    <TableCell colspan={6}>
                      <div class="space-y-3 rounded-md border bg-background p-3">
                        {#each sample.conflicts as conflict}
                          <section>
                            <div class="mb-2 text-xs font-medium uppercase text-muted-foreground">
                              {conflict.dimension}
                            </div>
                            <div class="flex flex-wrap gap-2">
                              {#each conflict.candidates as candidate}
                                <Button
                                  variant="outline"
                                  size="sm"
                                  class="h-auto flex-col items-start py-2 text-xs"
                                  onclick={() =>
                                    resolveConflict(sample.id, conflict.dimension, candidate.value)}
                                >
                                  <div class="font-medium">{candidate.value}</div>
                                  <div class="text-muted-foreground">
                                    {candidate.source}{candidate.confidence === null
                                      ? ""
                                      : ` ${(candidate.confidence * 100).toFixed(0)}%`}
                                  </div>
                                </Button>
                              {/each}
                            </div>
                          </section>
                        {/each}
                      </div>
                    </TableCell>
                  </TableRow>
                {/if}
              {/each}
              <tr style="height: {$virtualizer.getTotalSize() - (items[items.length - 1]?.end ?? 0)}px"></tr>
            {/if}
          </TableBody>
        </Table>
      </div>

      {#if selectedCount > 1}
        <div class="absolute bottom-3 left-3 z-20 flex h-10 items-center gap-2 rounded-md border bg-background px-3 shadow-sm">
          <span class="w-20 text-sm font-medium">{selectedCount} selected</span>

          <Select
            type="single"
            value={bulkDimension}
            onValueChange={(v) => { if (v) bulkDimension = v as "Type" | "Instrument"; }}
          >
            <SelectTrigger size="sm" class="w-32">
              <SelectPrimitive.Value placeholder="Dimension" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="Type">Type</SelectItem>
              <SelectItem value="Instrument">Instrument</SelectItem>
            </SelectContent>
          </Select>

          <Select
            type="single"
            value={bulkValue}
            onValueChange={(v) => { if (v) bulkValue = v; }}
          >
            <SelectTrigger size="sm" class="w-32">
              <SelectPrimitive.Value placeholder="Value" />
            </SelectTrigger>
            <SelectContent>
              {#each optionsForDimension(bulkDimension) as value}
                <SelectItem {value}>{value}</SelectItem>
              {/each}
            </SelectContent>
          </Select>

          <Button size="sm" onclick={applyBulkTag}>Set tag</Button>
          <Button variant="outline" size="sm" onclick={clearBulkTag}>
            Clear tag
          </Button>
          <Button variant="ghost" size="sm" onclick={clearSelection}>
            <X />
            Deselect
          </Button>
        </div>
      {/if}
    </div>
  {/if}

  {#if editing}
    <div class="border-t bg-background p-3">
      <div class="flex items-center gap-2">
        <Pencil class="size-4 text-muted-foreground" />
        <span class="text-sm font-medium">Edit {editing.dimension}</span>

        <Select
          type="single"
          value={editing.value}
          onValueChange={(v) => { if (editing && v) editing = { ...editing, value: v }; }}
        >
          <SelectTrigger size="sm" class="w-36">
            <SelectPrimitive.Value placeholder="Select..." />
          </SelectTrigger>
          <SelectContent>
            {#each optionsForDimension(editing.dimension) as value}
              <SelectItem {value}>{value}</SelectItem>
            {/each}
          </SelectContent>
        </Select>

        <Button size="sm" onclick={saveEditing}>Save</Button>
        <Button variant="outline" size="sm" onclick={clearEditing}>Clear user tag</Button>
        <Button variant="ghost" size="sm" onclick={() => (editing = null)}>Cancel</Button>
      </div>
    </div>
  {/if}
</div>
