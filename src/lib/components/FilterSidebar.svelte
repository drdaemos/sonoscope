<script lang="ts">
  import { SlidersHorizontal } from "@lucide/svelte";
  import { Badge, Button, Input, Separator } from "$lib/components/ui";
  import { currentLibrary, samples } from "$lib/stores/library";
</script>

<aside class="flex w-64 shrink-0 flex-col border-r bg-sidebar text-sidebar-foreground">
  <div class="space-y-3 p-4">
    <div class="flex items-center justify-between">
      <h2 class="text-sm font-medium">Filters</h2>
      <Button variant="ghost" size="icon" aria-label="Filter options">
        <SlidersHorizontal />
      </Button>
    </div>

    <Input placeholder="Search filename..." disabled={!$currentLibrary} />
  </div>

  <Separator />

  <div class="min-h-0 flex-1 space-y-5 overflow-auto p-4">
    <section>
      <div class="mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">Type</div>
      <div class="flex flex-wrap gap-2">
        <Badge variant="outline">loop</Badge>
        <Badge variant="outline">one-shot</Badge>
      </div>
    </section>

    <section>
      <div class="mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
        Instrument
      </div>
      <div class="flex flex-wrap gap-2">
        <Badge variant="outline">kick</Badge>
        <Badge variant="outline">snare</Badge>
        <Badge variant="outline">bass</Badge>
        <Badge variant="outline">pad</Badge>
      </div>
    </section>

    <section>
      <div class="mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
        Library
      </div>
      <div class="rounded-md border bg-background p-3 text-xs text-muted-foreground">
        {$currentLibrary ? `${$samples.length} discovered files` : "Open a folder to begin."}
      </div>
    </section>
  </div>
</aside>
