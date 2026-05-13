<script lang="ts">
  import { Boxes, History, ListMusic, Play } from "@lucide/svelte";
  import FileList from "$lib/components/FileList.svelte";
  import FilterSidebar from "$lib/components/FilterSidebar.svelte";
  import LibraryBar, { type AppView } from "$lib/components/LibraryBar.svelte";
  import { Button, Card, Separator } from "$lib/components/ui";
  import { currentLibrary } from "$lib/stores/library";

  let activeView: AppView = "review";
</script>

<div class="flex h-full min-h-0 flex-col bg-background text-foreground">
  <LibraryBar {activeView} onViewChange={(view) => (activeView = view)} />

  <div class="flex min-h-0 flex-1">
    <FilterSidebar />

    <main class="flex min-w-0 flex-1 flex-col bg-muted/30">
      {#if activeView === "review"}
        <div class="flex h-12 shrink-0 items-center justify-between border-b bg-background px-4">
          <div class="flex items-center gap-2">
            <ListMusic class="size-4 text-muted-foreground" />
            <h1 class="text-sm font-medium">Review</h1>
          </div>
          <Button variant="outline" size="sm" disabled={!$currentLibrary}>Edit tags</Button>
        </div>
        <FileList />
      {:else if activeView === "organise"}
        <div class="flex h-12 shrink-0 items-center gap-2 border-b bg-background px-4">
          <Boxes class="size-4 text-muted-foreground" />
          <h1 class="text-sm font-medium">Organise</h1>
        </div>
        <div class="grid flex-1 place-items-center p-6">
          <Card class="w-full max-w-xl p-6">
            <h2 class="text-base font-semibold">Organisation workflow</h2>
            <p class="mt-2 text-sm text-muted-foreground">
              Pattern presets, preview, and apply controls will be added after tagging data exists.
            </p>
          </Card>
        </div>
      {:else}
        <div class="flex h-12 shrink-0 items-center gap-2 border-b bg-background px-4">
          <History class="size-4 text-muted-foreground" />
          <h1 class="text-sm font-medium">History</h1>
        </div>
        <div class="grid flex-1 place-items-center p-6">
          <Card class="w-full max-w-xl p-6">
            <h2 class="text-base font-semibold">Operation history</h2>
            <p class="mt-2 text-sm text-muted-foreground">
              Move/copy batches and rollback controls will appear here once file operations exist.
            </p>
          </Card>
        </div>
      {/if}
    </main>
  </div>

  <Separator />

  <footer class="flex h-12 shrink-0 items-center gap-3 bg-background px-4 text-sm">
    <Button variant="ghost" size="icon" disabled aria-label="Play selected sample">
      <Play />
    </Button>
    <div class="min-w-48 text-muted-foreground">No sample loaded</div>
    <div class="h-2 flex-1 rounded-full bg-muted"></div>
    <div class="w-20 text-right text-xs text-muted-foreground">0:00 / 0:00</div>
  </footer>
</div>
