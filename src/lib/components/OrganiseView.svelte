<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import LoaderCircle from "@lucide/svelte/icons/loader-circle";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import { onDestroy, onMount } from "svelte";
  import {
    commands,
    type OrganisationPreset,
    type OrganiseApplyResult,
    type OrganiseMode,
    type OrganisePreview,
  } from "$lib/bindings/bindings";
  import {
    Badge,
    Button,
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
    Input,
  } from "$lib/components/ui";
  import { currentLibrary, samples, tagDimensions } from "$lib/stores/library";
  import {
    defaultPresetName,
    filterPreviewEntries,
    validatePattern,
    type PreviewFilter,
  } from "$lib/stores/organise";
  import {
    conflictsOnly,
    dimensionFilters,
    filenameSearch,
    unanalysedOnly,
    visibleSamples,
  } from "$lib/stores/review";

  const PREVIEW_ROW_LIMIT = 300;
  const PREVIEW_DEBOUNCE_MS = 400;

  let pattern = $state("{Type}/{Instrument}");
  let mode = $state<OrganiseMode>("move");
  let destination = $state<string | null>(null);
  let presets = $state<OrganisationPreset[]>([]);
  let preview = $state<OrganisePreview | null>(null);
  let isPreviewing = $state(false);
  let isApplying = $state(false);
  let confirmOpen = $state(false);
  let savingPreset = $state(false);
  let presetName = $state("");
  let applyResult = $state<OrganiseApplyResult | null>(null);
  let applyProgress = $state<{ done: number; total: number } | null>(null);
  let error = $state<string | null>(null);
  let previewFilter = $state<PreviewFilter>("all");

  let previewTimer: ReturnType<typeof setTimeout> | null = null;
  let previewToken = 0;
  let progressUnlisten: (() => void) | null = null;

  const dimensionNames = $derived($tagDimensions.map((dimension) => dimension.name));
  const validation = $derived(validatePattern(pattern, dimensionNames));
  const filtersActive = $derived(
    $filenameSearch.trim() !== "" ||
      $conflictsOnly ||
      $unanalysedOnly ||
      Object.values($dimensionFilters).some((values) => values.length > 0),
  );
  const scopeIds = $derived(filtersActive ? $visibleSamples.map((sample) => sample.id) : null);
  const scopeKey = $derived(scopeIds ? scopeIds.join(",") : "all");
  const actionableCount = $derived(
    preview
      ? preview.entries.filter(
          (entry) => !entry.conflict && !(mode === "move" && entry.unchanged),
        ).length
      : 0,
  );
  const canApply = $derived(
    Boolean($currentLibrary) &&
      validation.valid &&
      preview !== null &&
      actionableCount > 0 &&
      !isApplying &&
      (mode === "move" || destination !== null),
  );

  onMount(async () => {
    progressUnlisten = await listen<{ done: number; total: number }>(
      "organise-progress",
      (event) => {
        if (isApplying) applyProgress = event.payload;
      },
    );
  });

  onDestroy(() => {
    progressUnlisten?.();
    if (previewTimer) clearTimeout(previewTimer);
  });

  // Presets live in the library database, so reload them per library.
  $effect(() => {
    if ($currentLibrary) {
      void loadPresets();
    } else {
      presets = [];
    }
  });

  // "unchanged" is only meaningful when moving; drop the filter on mode switch.
  $effect(() => {
    if (mode === "copy" && previewFilter === "unchanged") previewFilter = "all";
  });

  // Recompute the preview (debounced) whenever the pattern or scope changes.
  $effect(() => {
    void scopeKey;
    void pattern;
    const library = $currentLibrary;
    if (previewTimer) clearTimeout(previewTimer);
    if (!library || !validation.valid) {
      preview = null;
      return;
    }
    previewTimer = setTimeout(() => void runPreview(), PREVIEW_DEBOUNCE_MS);
  });

  function formatError(commandError: unknown): string {
    return typeof commandError === "string" ? commandError : JSON.stringify(commandError);
  }

  async function loadPresets(): Promise<void> {
    const result = await commands.listOrganisationPresets();
    presets = result.status === "ok" ? result.data : [];
  }

  async function runPreview(): Promise<void> {
    const token = ++previewToken;
    isPreviewing = true;
    const result = await commands.previewOrganise(pattern, scopeIds);
    if (token !== previewToken) return;
    isPreviewing = false;
    if (result.status === "ok") {
      preview = result.data;
      error = null;
    } else {
      preview = null;
      error = formatError(result.error);
    }
  }

  async function chooseDestination(): Promise<void> {
    const selected = await openDialog({ directory: true, multiple: false });
    if (typeof selected === "string") destination = selected;
  }

  function startSavePreset(): void {
    presetName = defaultPresetName(pattern);
    savingPreset = true;
  }

  async function savePreset(): Promise<void> {
    const result = await commands.saveOrganisationPreset(presetName, pattern);
    if (result.status === "ok") {
      savingPreset = false;
      error = null;
      await loadPresets();
    } else {
      error = formatError(result.error);
    }
  }

  async function deletePreset(presetId: number): Promise<void> {
    const result = await commands.deleteOrganisationPreset(presetId);
    if (result.status === "ok") await loadPresets();
  }

  async function confirmApply(): Promise<void> {
    confirmOpen = false;
    isApplying = true;
    applyResult = null;
    applyProgress = null;
    const result = await commands.applyOrganise(
      pattern,
      mode,
      mode === "copy" ? destination : null,
      scopeIds,
    );
    isApplying = false;
    applyProgress = null;
    if (result.status === "ok") {
      applyResult = result.data;
      error = null;
      const refreshed = await commands.getSamples();
      if (refreshed.status === "ok") samples.set(refreshed.data);
      await runPreview();
    } else {
      error = formatError(result.error);
    }
  }

  function entryBadge(entry: OrganisePreview["entries"][number]): {
    label: string;
    variant: "secondary" | "outline" | "destructive";
  } | null {
    if (entry.conflict) return { label: "name clash", variant: "destructive" };
    if (entry.untagged) return { label: "untagged", variant: "secondary" };
    if (mode === "move" && entry.unchanged) return { label: "unchanged", variant: "outline" };
    return null;
  }

  function togglePreviewFilter(filter: PreviewFilter): void {
    previewFilter = previewFilter === filter ? "all" : filter;
  }

  const filteredPreviewEntries = $derived(
    preview ? filterPreviewEntries(preview.entries, previewFilter, mode) : [],
  );
  const visiblePreviewEntries = $derived(filteredPreviewEntries.slice(0, PREVIEW_ROW_LIMIT));
</script>

{#if !$currentLibrary}
  <div class="grid flex-1 place-items-center p-6 text-sm text-muted-foreground">
    Open a library to organise its files.
  </div>
{:else}
  <div class="flex min-h-0 flex-1 flex-col">
    <div class="shrink-0 space-y-3 border-b bg-background p-4">
      <div class="flex flex-wrap items-center gap-2">
        <span class="w-16 text-sm text-muted-foreground">Pattern</span>
        <Input
          class="w-80 font-mono"
          value={pattern}
          placeholder={"{Type}/{Instrument}"}
          aria-invalid={!validation.valid}
          oninput={(event) => (pattern = event.currentTarget.value)}
        />
        <DropdownMenu>
          <DropdownMenuTrigger>
            {#snippet child({ props })}
              <Button {...props} variant="outline" size="sm">
                Presets
                <ChevronDown />
              </Button>
            {/snippet}
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" class="w-72">
            {#each presets as preset (preset.id)}
              <DropdownMenuItem onclick={() => (pattern = preset.pattern)}>
                <div class="flex min-w-0 flex-1 items-center justify-between gap-2">
                  <div class="flex min-w-0 flex-col">
                    <span class="truncate font-medium">{preset.name}</span>
                    <span class="truncate font-mono text-xs text-muted-foreground">
                      {preset.pattern}
                    </span>
                  </div>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    aria-label={`Delete preset ${preset.name}`}
                    onclick={(event) => {
                      event.stopPropagation();
                      void deletePreset(preset.id);
                    }}
                  >
                    <Trash2 />
                  </Button>
                </div>
              </DropdownMenuItem>
            {:else}
              <DropdownMenuItem disabled>No presets</DropdownMenuItem>
            {/each}
          </DropdownMenuContent>
        </DropdownMenu>
        {#if savingPreset}
          <Input
            class="w-48"
            value={presetName}
            placeholder="Preset name"
            oninput={(event) => (presetName = event.currentTarget.value)}
          />
          <Button size="sm" disabled={!presetName.trim()} onclick={() => void savePreset()}>
            Save
          </Button>
          <Button variant="ghost" size="sm" onclick={() => (savingPreset = false)}>Cancel</Button>
        {:else}
          <Button variant="outline" size="sm" disabled={!validation.valid} onclick={startSavePreset}>
            Save as preset
          </Button>
        {/if}
      </div>

      {#if !validation.valid}
        <p class="pl-[4.5rem] text-xs text-destructive">{validation.error}</p>
      {/if}

      <div class="flex flex-wrap items-center gap-2">
        <span class="w-16 text-sm text-muted-foreground">Mode</span>
        <label class="flex cursor-pointer items-center gap-1.5 text-sm">
          <input type="radio" name="organise-mode" value="move" bind:group={mode} />
          Move within library
        </label>
        <label class="ml-3 flex cursor-pointer items-center gap-1.5 text-sm">
          <input type="radio" name="organise-mode" value="copy" bind:group={mode} />
          Copy to...
        </label>
        {#if mode === "copy"}
          <Button variant="outline" size="sm" onclick={() => void chooseDestination()}>
            <FolderOpen />
            Choose folder
          </Button>
          <span class="max-w-72 truncate font-mono text-xs text-muted-foreground">
            {destination ?? "No destination selected"}
          </span>
        {/if}
      </div>

      {#if error}
        <p class="flex items-center gap-1.5 text-xs text-destructive">
          <TriangleAlert class="size-3.5 shrink-0" />
          {error}
        </p>
      {/if}
    </div>

    <div class="flex shrink-0 items-center gap-2 border-b bg-background px-4 py-2">
      <span class="text-sm font-medium">Preview</span>
      {#if isPreviewing}
        <LoaderCircle class="size-3.5 animate-spin text-muted-foreground" />
      {/if}
      {#if preview}
        <button type="button" onclick={() => (previewFilter = "all")}>
          <Badge
            variant="secondary"
            class={previewFilter === "all" ? "ring-2 ring-ring/50" : "cursor-pointer"}
          >
            {preview.total} files
          </Badge>
        </button>
        {#if filtersActive}
          <Badge variant="outline">filtered: {preview.total} of {$samples.length}</Badge>
        {/if}
        {#if preview.untagged_count > 0}
          <button type="button" onclick={() => togglePreviewFilter("untagged")}>
            <Badge
              variant="secondary"
              class={previewFilter === "untagged" ? "ring-2 ring-ring/50" : "cursor-pointer"}
            >
              {preview.untagged_count} untagged
            </Badge>
          </button>
        {/if}
        {#if preview.conflict_count > 0}
          <button type="button" onclick={() => togglePreviewFilter("clash")}>
            <Badge
              variant="destructive"
              class={previewFilter === "clash" ? "ring-2 ring-destructive/40" : "cursor-pointer"}
            >
              {preview.conflict_count} name clashes
            </Badge>
          </button>
        {/if}
        {#if mode === "move" && preview.unchanged_count > 0}
          <button type="button" onclick={() => togglePreviewFilter("unchanged")}>
            <Badge
              variant="outline"
              class={previewFilter === "unchanged" ? "ring-2 ring-ring/50" : "cursor-pointer"}
            >
              {preview.unchanged_count} unchanged
            </Badge>
          </button>
        {/if}
        {#if previewFilter !== "all"}
          <span class="text-xs text-muted-foreground">
            showing {filteredPreviewEntries.length} matching — click badge again to clear
          </span>
        {/if}
      {/if}
    </div>

    <div class="min-h-0 flex-1 overflow-auto">
      {#if preview && filteredPreviewEntries.length > 0}
        <div class="divide-y">
          {#each visiblePreviewEntries as entry (entry.sample_id)}
            {@const badge = entryBadge(entry)}
            <div class="flex items-center gap-2 px-4 py-1.5 font-mono text-xs">
              <span class="min-w-0 flex-1 truncate text-muted-foreground" title={entry.from}>
                {entry.from}
              </span>
              <ArrowRight class="size-3 shrink-0 text-muted-foreground" />
              <span class="min-w-0 flex-1 truncate" title={entry.to}>{entry.to}</span>
              {#if badge}
                <Badge variant={badge.variant} class="shrink-0">{badge.label}</Badge>
              {/if}
            </div>
          {/each}
          {#if filteredPreviewEntries.length > PREVIEW_ROW_LIMIT}
            <div class="px-4 py-2 text-xs text-muted-foreground">
              +{filteredPreviewEntries.length - PREVIEW_ROW_LIMIT} more files
            </div>
          {/if}
        </div>
      {:else if preview && previewFilter !== "all"}
        <div class="grid h-full place-items-center text-sm text-muted-foreground">
          No files match this flag anymore.
        </div>
      {:else if validation.valid}
        <div class="grid h-full place-items-center text-sm text-muted-foreground">
          {isPreviewing ? "Computing preview..." : "No files to organise."}
        </div>
      {:else}
        <div class="grid h-full place-items-center text-sm text-muted-foreground">
          Enter a valid pattern to preview the reorganisation.
        </div>
      {/if}
    </div>

    <div class="flex shrink-0 items-center gap-3 border-t bg-background px-4 py-3">
      <Button disabled={!canApply} onclick={() => (confirmOpen = true)}>
        {#if isApplying}
          <LoaderCircle class="animate-spin" />
          {applyProgress ? `${mode === "move" ? "Moving" : "Copying"} ${applyProgress.done} / ${applyProgress.total}` : "Applying..."}
        {:else}
          Apply
        {/if}
      </Button>
      {#if applyResult}
        <span class="text-sm text-muted-foreground">
          {applyResult.processed} files {mode === "copy" ? "copied" : "moved"},
          {applyResult.skipped} skipped
        </span>
        {#if applyResult.errors.length > 0}
          <span class="truncate text-xs text-destructive" title={applyResult.errors.join("\n")}>
            {applyResult.errors.length} errors
          </span>
        {/if}
      {/if}
    </div>
  </div>
{/if}

{#if confirmOpen && preview}
  <div
    class="fixed inset-0 z-50 grid place-items-center bg-background/70 p-4 backdrop-blur-sm"
    role="presentation"
    onclick={(event) => {
      if (event.target === event.currentTarget) confirmOpen = false;
    }}
  >
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="organise-confirm-title"
      class="w-full max-w-md rounded-lg border bg-background p-5 shadow-xl"
    >
      <h2 id="organise-confirm-title" class="text-sm font-semibold">
        {mode === "move" ? "Move" : "Copy"} {actionableCount} files?
      </h2>
      <p class="mt-2 text-sm text-muted-foreground">
        {#if mode === "move"}
          Files will be moved within the library according to
          <span class="font-mono">{pattern}</span>. This can be rolled back from the History tab.
        {:else}
          Files will be copied to
          <span class="font-mono">{destination}</span> according to
          <span class="font-mono">{pattern}</span>. Copies cannot be rolled back.
        {/if}
      </p>
      <div class="mt-4 flex justify-end gap-2">
        <Button variant="outline" size="sm" onclick={() => (confirmOpen = false)}>Cancel</Button>
        <Button size="sm" onclick={() => void confirmApply()}>
          {mode === "move" ? "Move files" : "Copy files"}
        </Button>
      </div>
    </div>
  </div>
{/if}
