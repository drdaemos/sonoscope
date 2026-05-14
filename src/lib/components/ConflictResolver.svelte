<script lang="ts">
  import { AlertTriangle, X } from "@lucide/svelte";
  import {
    Badge,
    Button,
    Card,
    CardContent,
    CardHeader,
    CardTitle,
  } from "$lib/components/ui";
  import type { SampleRow, SampleTag } from "$lib/stores/library";

  type Props = {
    sample: SampleRow;
    onResolve: (sampleId: number, dimension: string, value: string) => void | Promise<void>;
    onClose: () => void;
  };

  let { sample, onResolve, onClose }: Props = $props();

  function preferredCandidate(candidates: SampleTag[]): SampleTag | undefined {
    return [...candidates].sort((a, b) => {
      if (a.is_primary !== b.is_primary) return a.is_primary ? -1 : 1;
      return (b.confidence ?? 0) - (a.confidence ?? 0);
    })[0];
  }

  function confidenceLabel(confidence: number | null): string {
    return confidence === null ? "" : `${(confidence * 100).toFixed(0)}%`;
  }
</script>

<Card class="absolute right-3 top-3 z-30 w-[min(34rem,calc(100%-1.5rem))] shadow-lg">
  <CardHeader class="flex-row items-start justify-between space-y-0">
    <div class="min-w-0">
      <CardTitle class="flex items-center gap-2 text-sm">
        <AlertTriangle class="size-4 text-destructive" />
        Resolve tag conflict
      </CardTitle>
      <div class="mt-1 truncate font-mono text-xs text-muted-foreground">{sample.filename}</div>
    </div>
    <Button variant="ghost" size="icon-sm" aria-label="Close conflict panel" onclick={onClose}>
      <X />
    </Button>
  </CardHeader>

  <CardContent class="space-y-3">
    {#each sample.conflicts as conflict}
      {@const preferred = preferredCandidate(conflict.candidates)}
      <section class="rounded-md border bg-background p-3">
        <div class="mb-2 flex items-center justify-between gap-3">
          <div class="text-xs font-medium uppercase text-muted-foreground">
            {conflict.dimension}
          </div>
          {#if preferred}
            <Badge variant="soft">Current: {preferred.value}</Badge>
          {/if}
        </div>

        <div class="grid gap-2 sm:grid-cols-2">
          {#each conflict.candidates as candidate}
            <Button
              variant={candidate.value === preferred?.value ? "secondary" : "outline"}
              size="sm"
              class="h-auto justify-start py-2 text-left"
              onclick={() => onResolve(sample.id, conflict.dimension, candidate.value)}
            >
              <span class="min-w-0">
                <span class="block truncate text-xs font-medium">{candidate.value}</span>
                <span class="block truncate text-[11px] text-muted-foreground">
                  {candidate.source}{confidenceLabel(candidate.confidence)
                    ? ` ${confidenceLabel(candidate.confidence)}`
                    : ""}
                </span>
              </span>
            </Button>
          {/each}
        </div>
      </section>
    {/each}
  </CardContent>
</Card>
