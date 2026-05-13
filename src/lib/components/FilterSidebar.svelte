<script lang="ts">
  import { FunnelX } from "@lucide/svelte";
  import { Button, Checkbox, Input, ToggleGroup, ToggleGroupItem } from "$lib/components/ui";
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
</script>

<aside class="flex w-64 shrink-0 flex-col border-r bg-sidebar text-sidebar-foreground">
  <div class="space-y-2 border-b p-3">
    <div class="flex items-center justify-between">
      <h2 class="text-sm font-medium">Filters</h2>
      <Button variant="ghost" size="icon-sm" aria-label="Clear filters" onclick={clearFilters}>
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

  <div class="min-h-0 flex-1 space-y-3 overflow-auto p-3">
    {#each dimensions as dimension}
      {@const options = [...($filterOptions.get(dimension)?.entries() ?? [])].sort()}
      {#if options.length > 0}
        <section>
          <div class="mb-1.5 text-xs font-medium uppercase text-muted-foreground">
            {dimension}
          </div>
          <ToggleGroup
            type="multiple"
            variant="chip"
            size="xs"
            spacing={1}
            value={$dimensionFilters[dimension] ?? []}
            onValueChange={(values) => {
              const current = $dimensionFilters[dimension] ?? [];
              const added = values.filter((v: string) => !current.includes(v));
              const removed = current.filter((v: string) => !values.includes(v));
              for (const v of [...added, ...removed]) toggleFilterValue(dimension, v);
            }}
            class="flex-wrap"
          >
            {#each options as [value, count]}
              <ToggleGroupItem {value} class="text-xs">
                {value} {count}
              </ToggleGroupItem>
            {/each}
          </ToggleGroup>
        </section>
      {/if}
    {/each}

    <section class="space-y-1.5">
      <label
        class="flex w-full cursor-pointer items-center gap-2 rounded-md border bg-background px-2 py-1.5 text-sm hover:bg-accent"
      >
        <Checkbox
          checked={$conflictsOnly}
          onCheckedChange={(v) => conflictsOnly.set(v === true)}
        />
        Conflicts only
      </label>
      <label
        class="flex w-full cursor-pointer items-center gap-2 rounded-md border bg-background px-2 py-1.5 text-sm hover:bg-accent"
      >
        <Checkbox
          checked={$unanalysedOnly}
          onCheckedChange={(v) => unanalysedOnly.set(v === true)}
        />
        Unanalysed only
      </label>
    </section>
  </div>
</aside>
