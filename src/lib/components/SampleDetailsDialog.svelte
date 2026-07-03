<script lang="ts">
  import AlertTriangle from "@lucide/svelte/icons/triangle-alert";
  import Check from "@lucide/svelte/icons/check";
  import FileAudio from "@lucide/svelte/icons/file-audio";
  import Info from "@lucide/svelte/icons/info";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import X from "@lucide/svelte/icons/x";
  import { Badge, Button, Separator } from "$lib/components/ui";
  import type { SampleRow, SampleTag } from "$lib/stores/library";

  type ConflictOption = {
    value: string;
    tags: SampleTag[];
    bestConfidence: number | null;
  };

  type Props = {
    sample: SampleRow;
    onClose: () => void;
    onResolveConflict: (
      sampleId: number,
      dimension: string,
      value: string,
    ) => void | Promise<void>;
  };

  let { sample, onClose, onResolveConflict }: Props = $props();

  let modelTags = $derived(sample.tags.filter((tag) => tag.source === "model"));
  let dimensionNames = $derived(
    Array.from(new Set(sample.tags.map((tag) => tag.dimension))).sort((left, right) =>
      left.localeCompare(right),
    ),
  );

  function tagsForDimension(dimension: string): SampleTag[] {
    return sample.tags
      .filter((tag) => tag.dimension === dimension)
      .sort(compareTags);
  }

  function compareTags(left: SampleTag, right: SampleTag): number {
    if (left.is_primary !== right.is_primary) return left.is_primary ? -1 : 1;
    if (left.source === "user" && right.source !== "user") return -1;
    if (left.source !== "user" && right.source === "user") return 1;
    return (right.confidence ?? 0) - (left.confidence ?? 0);
  }

  function conflictOptions(candidates: SampleTag[]): ConflictOption[] {
    const options = new Map<string, SampleTag[]>();
    for (const candidate of candidates) {
      options.set(candidate.value, [...(options.get(candidate.value) ?? []), candidate]);
    }

    return [...options.entries()]
      .map(([value, tags]) => ({
        value,
        tags: tags.sort(compareTags),
        bestConfidence: bestConfidence(tags),
      }))
      .sort((left, right) => (right.bestConfidence ?? 0) - (left.bestConfidence ?? 0));
  }

  function bestConfidence(tags: SampleTag[]): number | null {
    const confidences = tags
      .map((tag) => tag.confidence)
      .filter((confidence): confidence is number => confidence !== null);
    if (confidences.length === 0) return null;
    return Math.max(...confidences);
  }

  function confidenceLabel(confidence: number | null): string {
    if (confidence === null) return "manual";
    return `${Math.round(confidence * 100)}%`;
  }

  function sourceLabel(source: SampleTag["source"]): string {
    if (source === "model") return "ML model";
    if (source === "heuristic") return "Filename";
    if (source === "metadata") return "Metadata";
    return "User";
  }

  function formatDuration(durationMs: number | null): string {
    if (durationMs == null) return "-";
    const totalSeconds = Math.max(0, Math.round(durationMs / 1000));
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}:${seconds.toString().padStart(2, "0")}`;
  }

  function formatBytes(value: number | null): string {
    if (value == null) return "-";
    if (value >= 1024 * 1024) return `${(value / (1024 * 1024)).toFixed(1)} MB`;
    if (value >= 1024) return `${Math.round(value / 1024)} KB`;
    return `${value} B`;
  }

  function formatNumber(value: number | null, suffix = ""): string {
    if (value == null) return "-";
    return `${value}${suffix}`;
  }
</script>

<svelte:window onkeydown={(event) => { if (event.key === "Escape") onClose(); }} />

<div
  class="fixed inset-0 z-50 grid place-items-center bg-background/70 p-4 backdrop-blur-sm"
  role="presentation"
  onclick={(event) => {
    if (event.target === event.currentTarget) onClose();
  }}
>
  <div
    role="dialog"
    aria-modal="true"
    aria-labelledby="sample-details-title"
    class="flex max-h-[min(46rem,calc(100vh-2rem))] w-full max-w-4xl flex-col overflow-hidden rounded-lg border bg-background shadow-xl"
  >
    <header class="flex items-start justify-between gap-4 border-b px-5 py-4">
      <div class="min-w-0">
        <div class="flex items-center gap-2">
          {#if sample.conflicts.length > 0}
            <AlertTriangle class="size-4 shrink-0 text-destructive" />
          {:else}
            <Info class="size-4 shrink-0 text-muted-foreground" />
          {/if}
          <h2 id="sample-details-title" class="truncate font-mono text-sm font-semibold">
            {sample.filename}
          </h2>
        </div>
        <div class="mt-1 truncate font-mono text-xs text-muted-foreground">
          {sample.relative_path}
        </div>
      </div>
      <Button variant="ghost" size="icon-sm" aria-label="Close sample details" onclick={onClose}>
        <X />
      </Button>
    </header>

    <div class="min-h-0 overflow-auto px-5 py-4">
      <div class="grid gap-5 lg:grid-cols-[17rem_minmax(0,1fr)]">
        <aside class="space-y-5">
          <section>
            <div class="mb-2 flex items-center gap-2 text-xs font-medium uppercase text-muted-foreground">
              <FileAudio class="size-3.5" />
              File
            </div>
            <dl class="grid grid-cols-[6.5rem_minmax(0,1fr)] gap-x-3 gap-y-2 text-sm">
              <dt class="text-muted-foreground">Status</dt>
              <dd><Badge variant="soft">{sample.analysis_status}</Badge></dd>
              <dt class="text-muted-foreground">Format</dt>
              <dd>{sample.format ?? "-"}</dd>
              <dt class="text-muted-foreground">Duration</dt>
              <dd>{formatDuration(sample.duration_ms)}</dd>
              <dt class="text-muted-foreground">Size</dt>
              <dd>{formatBytes(sample.size_bytes)}</dd>
              <dt class="text-muted-foreground">Sample rate</dt>
              <dd>{formatNumber(sample.sample_rate, " Hz")}</dd>
              <dt class="text-muted-foreground">Bit depth</dt>
              <dd>{formatNumber(sample.bit_depth, "-bit")}</dd>
              <dt class="text-muted-foreground">Channels</dt>
              <dd>{formatNumber(sample.channels)}</dd>
            </dl>
          </section>

          <section>
            <div class="mb-2 flex items-center gap-2 text-xs font-medium uppercase text-muted-foreground">
              <Sparkles class="size-3.5" />
              ML Detections
            </div>
            {#if modelTags.length > 0}
              <div class="space-y-2">
                {#each modelTags.sort(compareTags) as tag}
                  <div class="rounded-md border bg-muted/30 px-3 py-2">
                    <div class="flex items-center justify-between gap-2">
                      <span class="truncate text-sm font-medium">{tag.dimension}: {tag.value}</span>
                      <Badge variant="secondary">{confidenceLabel(tag.confidence)}</Badge>
                    </div>
                  </div>
                {/each}
              </div>
            {:else}
              <p class="text-sm text-muted-foreground">No ML tags were returned for this sample.</p>
            {/if}
          </section>
        </aside>

        <div class="space-y-5">
          {#if sample.conflicts.length > 0}
            <section class="rounded-lg border border-destructive/30 bg-destructive/5 p-4">
              <div class="mb-3 flex items-center gap-2">
                <AlertTriangle class="size-4 text-destructive" />
                <h3 class="text-sm font-semibold">Decisions Needed</h3>
              </div>

              <div class="space-y-3">
                {#each sample.conflicts as conflict}
                  <div class="rounded-md border bg-background p-3">
                    <div class="mb-2 text-sm font-medium">{conflict.dimension}</div>
                    <div class="grid gap-2 md:grid-cols-2">
                      {#each conflictOptions(conflict.candidates) as option}
                        <button
                          type="button"
                          class="rounded-md border p-3 text-left transition hover:border-primary hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                          onclick={() => onResolveConflict(sample.id, conflict.dimension, option.value)}
                        >
                          <div class="mb-2 flex items-center justify-between gap-2">
                            <span class="truncate text-sm font-semibold">{option.value}</span>
                            <Badge variant="soft">{confidenceLabel(option.bestConfidence)}</Badge>
                          </div>
                          <div class="flex flex-wrap gap-1">
                            {#each option.tags as tag}
                              <Badge variant={tag.source === "model" ? "secondary" : "outline"}>
                                {sourceLabel(tag.source)}
                              </Badge>
                            {/each}
                          </div>
                        </button>
                      {/each}
                    </div>
                  </div>
                {/each}
              </div>
            </section>
          {/if}

          <section>
            <div class="mb-3 flex items-center justify-between gap-3">
              <h3 class="text-sm font-semibold">All Analysis Data</h3>
              {#if sample.conflicts.length === 0}
                <Badge variant="soft"><Check class="size-3" /> No conflicts</Badge>
              {/if}
            </div>

            {#if dimensionNames.length > 0}
              <div class="overflow-hidden rounded-lg border">
                {#each dimensionNames as dimension, index}
                  {#if index > 0}<Separator />{/if}
                  <div class="grid gap-3 p-3 md:grid-cols-[8rem_minmax(0,1fr)]">
                    <div class="text-sm font-medium">{dimension}</div>
                    <div class="flex flex-wrap gap-2">
                      {#each tagsForDimension(dimension) as tag}
                        <div class="flex items-center gap-1 rounded-md border bg-muted/30 px-2 py-1">
                          <span class="max-w-32 truncate text-sm">{tag.value}</span>
                          <Badge variant={tag.source === "model" ? "secondary" : tag.source === "user" ? "default" : "outline"}>
                            {sourceLabel(tag.source)}
                          </Badge>
                          {#if tag.confidence !== null}
                            <span class="text-xs text-muted-foreground">{confidenceLabel(tag.confidence)}</span>
                          {/if}
                          {#if tag.is_primary}
                            <Badge variant="soft">primary</Badge>
                          {/if}
                        </div>
                      {/each}
                    </div>
                  </div>
                {/each}
              </div>
            {:else}
              <p class="rounded-lg border bg-muted/30 p-3 text-sm text-muted-foreground">
                No tags have been gathered yet.
              </p>
            {/if}
          </section>
        </div>
      </div>
    </div>
  </div>
</div>
