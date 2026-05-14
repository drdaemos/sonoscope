<script lang="ts">
  import { Select as SelectPrimitive } from "bits-ui";
  import { AlertTriangle, FileAudio, Pencil, X } from "@lucide/svelte";
  import { commands } from "$lib/bindings/bindings";
  import ConflictResolver from "$lib/components/ConflictResolver.svelte";
  import TagValueEditor from "$lib/components/TagValueEditor.svelte";
  import {
    Badge,
    Button,
    Checkbox,
    Input,
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
  } from "$lib/components/ui";
  import { currentLibrary, samples, tagDimensions, type SampleRow, type TagDimension } from "$lib/stores/library";
  import { loadPlaybackSample } from "$lib/stores/playback";
  import {
    clearSelection,
    dimensionSortKey,
    displayTags,
    displayTagValues,
    hasConflict,
    reviewViewportKey,
    selectedSampleIds,
    setSelectionState,
    setSort,
    sortDirection,
    sortKey,
    toggleSelection,
    visibleSamples,
    type SortKey,
  } from "$lib/stores/review";

  const ROW_HEIGHT = 44;
  const INTERACTIVE_ROW_SELECTOR = "button, a, input, [role='button'], [data-row-action]";
  const REVIEW_COLUMN_DIMENSIONS = ["Type", "Instrument", "Key"];
  const VIRTUAL_OVERSCAN = 12;
  type DragSelectionMode = "select" | "deselect";
  type PendingDragSelection = {
    sampleId: number;
    mode: DragSelectionMode;
    clientX: number;
    clientY: number;
  };

  type VirtualRow = {
    sample: SampleRow;
    index: number;
    key: number;
    start: number;
  };

  const fallbackTagDimensions: TagDimension[] = [
    {
      name: "Type",
      value_type: "enum",
      values: ["break", "fill", "loop", "one-shot", "texture", "top-loop"],
    },
    {
      name: "Instrument",
      value_type: "multi_enum",
      values: [
        "bass",
        "brass",
        "chord",
        "clap",
        "cymbal",
        "foley",
        "fx",
        "guitar",
        "hi-hat",
        "kick",
        "lead",
        "pad",
        "percussion",
        "piano",
        "snare",
        "strings",
        "synth",
        "vocal",
        "woodwind",
      ],
    },
    {
      name: "Key",
      value_type: "enum",
      values: ["A", "A#", "B", "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#"],
    },
    { name: "Tempo", value_type: "numeric", values: [] },
  ];

  let editing = $state<{ sampleId: number; dimension: string; value: string } | null>(null);
  let bulkDimension = $state("Type");
  let bulkValue = $state("loop");
  let expandedConflictSampleId = $state<number | null>(null);
  let scrollContainerRef = $state<HTMLDivElement | null>(null);
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  let dragSelectionMode = $state<DragSelectionMode | null>(null);
  let pendingDragSelection = $state<PendingDragSelection | null>(null);
  let suppressNextRowClick = $state(false);
  let selectedCount = $derived($selectedSampleIds.size);
  let effectiveTagDimensions = $derived(
    $tagDimensions.length > 0 ? $tagDimensions : fallbackTagDimensions,
  );
  let editableTagDimensions = $derived(effectiveTagDimensions.filter(isEditableDimension));
  let columnTagDimensions = $derived(
    REVIEW_COLUMN_DIMENSIONS.map((name) => dimensionByName(name)).filter(isPresent),
  );
  let bulkTagDimension = $derived(dimensionByName(bulkDimension));
  let expandedConflictSample = $derived(
    $visibleSamples.find((sample) => sample.id === expandedConflictSampleId) ?? null,
  );
  let gridTemplateColumns = $derived(
    [
      "2.25rem",
      "minmax(10rem,1fr)",
      ...columnTagDimensions.map((dimension) => columnWidth(dimension.name)),
      "4.5rem",
      "5.5rem",
    ].join(" "),
  );
  let lastViewportKey: string | null = null;
  let virtualTotalSize = $derived($visibleSamples.length * ROW_HEIGHT);
  let virtualStartIndex = $derived(
    Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - VIRTUAL_OVERSCAN),
  );
  let virtualEndIndex = $derived(
    Math.min(
      $visibleSamples.length,
      Math.ceil((scrollTop + viewportHeight) / ROW_HEIGHT) + VIRTUAL_OVERSCAN,
    ),
  );
  let virtualItems: VirtualRow[] = $derived(
    $visibleSamples
      .slice(virtualStartIndex, virtualEndIndex)
      .map((sample, offset) => {
        const index = virtualStartIndex + offset;
        return {
          sample,
          index,
          key: sample.id,
          start: index * ROW_HEIGHT,
        };
      }),
  );

  $effect(() => {
    const el = scrollContainerRef;
    if (!el) return;

    const updateViewportHeight = () => {
      viewportHeight = el.clientHeight;
    };
    updateViewportHeight();

    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(updateViewportHeight);
    observer.observe(el);
    return () => observer.disconnect();
  });

  $effect(() => {
    const key = $reviewViewportKey;
    const el = scrollContainerRef;
    if (lastViewportKey !== null && key !== lastViewportKey) {
      expandedConflictSampleId = null;
      editing = null;
      scrollTop = 0;
      el?.scrollTo({ top: 0, left: 0 });
    }
    lastViewportKey = key;
  });

  $effect(() => {
    const el = scrollContainerRef;
    if (!el) return;
    const maxOffset = Math.max(0, virtualTotalSize - viewportHeight);
    if (scrollTop > maxOffset) {
      scrollTop = maxOffset;
      el.scrollTop = maxOffset;
    }
  });

  $effect(() => {
    if (editableTagDimensions.length === 0) return;
    if (!editableTagDimensions.some((dimension) => dimension.name === bulkDimension)) {
      bulkDimension = editableTagDimensions[0]?.name ?? "";
      return;
    }

    const dimension = dimensionByName(bulkDimension);
    if (!dimension) return;
    if (isOptionDimension(dimension) && !dimension.values.includes(bulkValue)) {
      bulkValue = defaultValueForDimension(dimension);
    }
  });

  function formatDuration(durationMs: number | null): string {
    if (durationMs == null) return "-";
    const totalSeconds = Math.max(0, Math.round(durationMs / 1000));
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}:${seconds.toString().padStart(2, "0")}`;
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

  function isPresent<T>(value: T | undefined): value is T {
    return value !== undefined;
  }

  function isEditableDimension(dimension: TagDimension): boolean {
    return dimension.value_type === "numeric" || dimension.values.length > 0;
  }

  function isOptionDimension(dimension: TagDimension): boolean {
    return ["enum", "multi_enum"].includes(dimension.value_type);
  }

  function dimensionByName(name: string): TagDimension | undefined {
    return effectiveTagDimensions.find((dimension) => dimension.name === name);
  }

  function defaultValueForDimension(dimension: TagDimension): string {
    if (dimension.value_type === "numeric") return "";
    return dimension.values[0] ?? "";
  }

  function columnWidth(dimension: string): string {
    if (dimension === "Instrument") return "10rem";
    if (dimension === "Key") return "5rem";
    return "7rem";
  }

  function startEditing(sample: SampleRow, dimension: TagDimension) {
    editing = {
      sampleId: sample.id,
      dimension: dimension.name,
      value: displayTagValues(sample, dimension.name)[0] ?? defaultValueForDimension(dimension),
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
    if (!bulkTagDimension || bulkValue.trim().length === 0) return;
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
    if (!bulkTagDimension) return;
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
    if (suppressNextRowClick) {
      suppressNextRowClick = false;
      return;
    }
    if (isInteractiveRowTarget(event.target)) return;
    toggleSelection(sampleId, event.ctrlKey || event.metaKey);
  }

  function selectRowFromKeyboard(sampleId: number, event: KeyboardEvent) {
    if (event.key !== "Enter" && event.key !== " ") return;
    if (isInteractiveRowTarget(event.target)) return;
    event.preventDefault();
    toggleSelection(sampleId, event.ctrlKey || event.metaKey);
  }

  function playRow(sampleId: number, event: MouseEvent) {
    if (isInteractiveRowTarget(event.target)) return;
    void loadPlaybackSample(sampleId, { autoplay: true });
  }

  function startDragSelection(sampleId: number, event: PointerEvent) {
    if (event.button !== 0 || isInteractiveRowTarget(event.target)) return;

    pendingDragSelection = {
      sampleId,
      mode: $selectedSampleIds.has(sampleId) ? "deselect" : "select",
      clientX: event.clientX,
      clientY: event.clientY,
    };
  }

  function continueDragSelection(sampleId: number, event: PointerEvent) {
    if ((event.buttons & 1) !== 1) {
      endDragSelection();
      return;
    }
    if (pendingDragSelection) {
      beginDragSelection();
    }
    if (!dragSelectionMode) return;
    applyDragSelection(sampleId);
  }

  function moveDragSelection(sampleId: number, event: PointerEvent) {
    if (!pendingDragSelection || pendingDragSelection.sampleId !== sampleId) return;
    if ((event.buttons & 1) !== 1) {
      endDragSelection();
      return;
    }

    const moved =
      Math.abs(event.clientX - pendingDragSelection.clientX) >= 3 ||
      Math.abs(event.clientY - pendingDragSelection.clientY) >= 3;
    if (!moved) return;

    beginDragSelection();
  }

  function beginDragSelection() {
    if (!pendingDragSelection) return;
    dragSelectionMode = pendingDragSelection.mode;
    suppressNextRowClick = true;
    applyDragSelection(pendingDragSelection.sampleId);
    pendingDragSelection = null;
  }

  function endDragSelection() {
    dragSelectionMode = null;
    pendingDragSelection = null;
  }

  function applyDragSelection(sampleId: number) {
    if (!dragSelectionMode) return;
    setSelectionState(sampleId, dragSelectionMode === "select");
  }

  function isInteractiveRowTarget(target: EventTarget | null): boolean {
    return target instanceof HTMLElement && target.closest(INTERACTIVE_ROW_SELECTOR) !== null;
  }

  function updateScrollPosition(event: Event) {
    scrollTop = event.currentTarget instanceof HTMLElement ? event.currentTarget.scrollTop : 0;
  }

  async function refreshSamples() {
    const result = await commands.getSamples();
    if (result.status === "ok") samples.set(result.data);
  }
</script>

<svelte:window onpointerup={endDragSelection} onblur={endDragSelection} />

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
      <div class="flex h-full min-h-0 flex-col">
        <div
          class="grid h-9 shrink-0 items-center border-b bg-card text-xs font-medium text-muted-foreground"
          style={`grid-template-columns: ${gridTemplateColumns};`}
        >
          <div class="px-2"></div>
          <button
            type="button"
            class="min-w-0 px-3 text-left hover:text-foreground"
            onclick={() => setSort("filename")}
          >
            Sample{sortLabel("filename")}
          </button>
          {#each columnTagDimensions as dimension}
            {@const key = dimensionSortKey(dimension.name)}
            <button
              type="button"
              class="min-w-0 px-3 text-left hover:text-foreground"
              onclick={() => setSort(key)}
            >
              {dimension.name}{sortLabel(key)}
            </button>
          {/each}
          <div class="px-3">Conflict</div>
          <div class="px-3">Duration</div>
        </div>

        <div
          class="min-h-0 flex-1 overflow-auto"
          data-testid="sample-scroll-container"
          bind:this={scrollContainerRef}
          onscroll={updateScrollPosition}
        >
          <div
            class="relative w-full"
            style={`height: ${virtualTotalSize}px;`}
          >
            {#each virtualItems as virtualRow (virtualRow.key)}
              {@const sample = $visibleSamples[virtualRow.index]!}
              {@const selected = $selectedSampleIds.has(sample.id)}
              <div
                data-index={virtualRow.index}
                data-state={selected ? "selected" : undefined}
                role="row"
                tabindex="0"
                class="absolute left-0 top-0 grid w-full min-w-full items-center border-b transition-colors hover:bg-muted/50 data-[state=selected]:bg-muted"
                style={`height: ${ROW_HEIGHT}px; transform: translateY(${virtualRow.start}px); grid-template-columns: ${gridTemplateColumns};`}
                onpointerdown={(event) => startDragSelection(sample.id, event)}
                onpointermove={(event) => moveDragSelection(sample.id, event)}
                onpointerenter={(event) => continueDragSelection(sample.id, event)}
                onclick={(event) => selectRow(sample.id, event)}
                ondblclick={(event) => playRow(sample.id, event)}
                onkeydown={(event) => selectRowFromKeyboard(sample.id, event)}
              >
                <div class="flex min-w-0 items-center px-2">
                  <Checkbox
                    aria-label={`Select ${sample.filename}`}
                    checked={selected}
                    onCheckedChange={() => toggleSelection(sample.id, true)}
                  />
                </div>
                <div class="min-w-0 overflow-hidden px-3 py-1.5">
                  <div class="truncate font-mono text-xs leading-tight">{sample.filename}</div>
                  {#if directoryPath(sample)}
                    <div class="truncate font-mono text-[11px] leading-tight text-muted-foreground">
                      {directoryPath(sample)}
                    </div>
                  {/if}
                </div>
                {#each columnTagDimensions as dimension}
                  <div class="min-w-0 overflow-hidden px-3 py-1.5">
                    <button
                      type="button"
                      class="flex max-w-full flex-nowrap gap-1 overflow-hidden text-left"
                      onclick={() => startEditing(sample, dimension)}
                    >
                      {#each displayTags(sample, dimension.name) as tag}
                        <Badge variant={tag.is_primary ? "soft" : "outline"}>
                          {tag.value}
                        </Badge>
                      {:else}
                        <span class="text-xs text-muted-foreground">-</span>
                      {/each}
                    </button>
                  </div>
                {/each}
                <div class="min-w-0 overflow-hidden px-3 py-1.5">
                  {#if hasConflict(sample)}
                    <button type="button" onclick={() => toggleConflictPanel(sample.id)}>
                      <Badge variant="destructive"><AlertTriangle class="size-3" /> auto</Badge>
                    </button>
                  {:else}
                    <span class="text-xs text-muted-foreground">-</span>
                  {/if}
                </div>
                <div class="min-w-0 truncate px-3 py-1.5 text-xs text-muted-foreground">
                  {formatDuration(sample.duration_ms)}
                </div>
              </div>
            {/each}
          </div>
        </div>
      </div>

      {#if expandedConflictSample}
        <ConflictResolver
          sample={expandedConflictSample}
          onResolve={resolveConflict}
          onClose={() => (expandedConflictSampleId = null)}
        />
      {/if}

      {#if selectedCount > 1}
        <div class="absolute bottom-3 left-3 z-20 flex h-10 items-center gap-2 rounded-md border bg-background px-3 shadow-sm">
          <span class="w-20 text-sm font-medium">{selectedCount} selected</span>

          <Select
            type="single"
            value={bulkDimension}
            onValueChange={(v) => { if (v) bulkDimension = v; }}
          >
            <SelectTrigger size="sm" class="w-32">
              <SelectPrimitive.Value placeholder="Dimension" />
            </SelectTrigger>
            <SelectContent>
              {#each editableTagDimensions as dimension}
                <SelectItem value={dimension.name}>{dimension.name}</SelectItem>
              {/each}
            </SelectContent>
          </Select>

          {#if bulkTagDimension}
            {#if isOptionDimension(bulkTagDimension)}
              <Select
                type="single"
                value={bulkValue}
                onValueChange={(v) => { if (v) bulkValue = v; }}
              >
                <SelectTrigger size="sm" class="w-32">
                  <SelectPrimitive.Value placeholder="Value" />
                </SelectTrigger>
                <SelectContent>
                  {#each bulkTagDimension.values as value}
                    <SelectItem {value}>{value}</SelectItem>
                  {/each}
                </SelectContent>
              </Select>
            {:else}
              <Input
                type="number"
                class="h-8 w-24"
                value={bulkValue}
                oninput={(event) => (bulkValue = event.currentTarget.value)}
              />
            {/if}
          {/if}

          <Button size="sm" onclick={applyBulkTag} disabled={!bulkTagDimension || bulkValue.trim().length === 0}>Set tag</Button>
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
    {@const editingDimension = dimensionByName(editing.dimension)}
    <div class="border-t bg-background p-3">
      <div class="flex items-center gap-2">
        <Pencil class="size-4 text-muted-foreground" />
        {#if editingDimension}
          <TagValueEditor
            dimension={editingDimension}
            value={editing.value}
            label={`Edit ${editing.dimension}`}
            onValueChange={(value) => {
              if (editing) editing = { ...editing, value };
            }}
            onSave={saveEditing}
            onClear={clearEditing}
            onCancel={() => (editing = null)}
          />
        {/if}
      </div>
    </div>
  {/if}
</div>
