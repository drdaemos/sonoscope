<script lang="ts">
  import { Check, FunnelX } from "@lucide/svelte";
  import { Badge, Button, Input } from "$lib/components/ui";
  import {
    clearFilters,
    conflictsOnly,
    dimensionFilters,
    filenameSearch,
    filterOptions,
    toggleFilterValue,
    unanalysedOnly,
  } from "$lib/stores/review";
  import { currentLibrary } from "$lib/stores/library";

  const dimensions = ["Type", "Instrument", "Key"];

  function isActive(dimension: string, value: string): boolean {
    return ($dimensionFilters[dimension] ?? []).includes(value);
  }

  function toggleConflictsOnly() {
    conflictsOnly.update((value) => !value);
  }

  function toggleUnanalysedOnly() {
    unanalysedOnly.update((value) => !value);
  }
</script>

<aside class="flex w-64 shrink-0 flex-col border-r bg-sidebar text-sidebar-foreground">
  <div class="space-y-3 p-4">
    <div class="flex items-center justify-between">
      <h2 class="text-sm font-medium">Filters</h2>
      <Button variant="ghost" size="icon" aria-label="Clear filters" onclick={clearFilters}>
        <FunnelX />
      </Button>
    </div>

    <Input
      placeholder="Search filename..."
      disabled={!$currentLibrary}
      value={$filenameSearch}
      oninput={(event) => filenameSearch.set(event.currentTarget.value)}
    />
  </div>

  <div class="min-h-0 flex-1 space-y-5 overflow-auto p-4">
    {#each dimensions as dimension}
      {@const options = [...($filterOptions.get(dimension)?.entries() ?? [])].sort()}
      {#if options.length > 0}
        <section>
          <div class="mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
            {dimension}
          </div>
          <div class="flex flex-wrap gap-2">
            {#each options as [value, count]}
              <button type="button" onclick={() => toggleFilterValue(dimension, value)}>
                <Badge
                  variant={isActive(dimension, value) ? "default" : "outline"}
                  class={isActive(dimension, value) ? "border-primary" : ""}
                >
                  {value} {count}
                </Badge>
              </button>
            {/each}
          </div>
        </section>
      {/if}
    {/each}

    <section class="space-y-2">
      <button
        type="button"
        class="flex w-full items-center gap-2 rounded-md border bg-background px-3 py-2 text-left text-sm hover:bg-accent"
        onclick={toggleConflictsOnly}
        aria-pressed={$conflictsOnly}
      >
        <span
          class={`flex size-4 items-center justify-center rounded border ${
            $conflictsOnly ? "border-primary bg-primary text-primary-foreground" : "bg-background"
          }`}
        >
          {#if $conflictsOnly}
            <Check class="size-3" />
          {/if}
        </span>
        Conflicts only
      </button>
      <button
        type="button"
        class="flex w-full items-center gap-2 rounded-md border bg-background px-3 py-2 text-left text-sm hover:bg-accent"
        onclick={toggleUnanalysedOnly}
        aria-pressed={$unanalysedOnly}
      >
        <span
          class={`flex size-4 items-center justify-center rounded border ${
            $unanalysedOnly
              ? "border-primary bg-primary text-primary-foreground"
              : "bg-background"
          }`}
        >
          {#if $unanalysedOnly}
            <Check class="size-3" />
          {/if}
        </span>
        Unanalysed only
      </button>
    </section>
  </div>
</aside>
