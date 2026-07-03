<script lang="ts">
  import Boxes from "@lucide/svelte/icons/boxes";
  import History from "@lucide/svelte/icons/history";
  import ListMusic from "@lucide/svelte/icons/list-music";
  import { onMount } from "svelte";
  import FileList from "$lib/components/FileList.svelte";
  import FilterSidebar from "$lib/components/FilterSidebar.svelte";
  import LibraryBar, { type AppView } from "$lib/components/LibraryBar.svelte";
  import PlaybackFooter from "$lib/components/PlaybackFooter.svelte";
  import { Badge, Card, CardHeader, CardTitle, CardContent, Separator } from "$lib/components/ui";
  import { currentLibrary, samples, tagDimensions } from "$lib/stores/library";

  let activeView: AppView = "review";

  onMount(async () => {
    if (!import.meta.env.DEV) return;
    const params = new URLSearchParams(window.location.search);
    if (!params.has("mockReview")) return;

    const { makeMockLibrary, makeMockSamples, makeMockTagDimensions } = await import(
      "$lib/dev/mockReview"
    );
    currentLibrary.set(makeMockLibrary());
    tagDimensions.set(makeMockTagDimensions());
    samples.set(makeMockSamples());
  });
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
            <Badge variant="secondary">
              {$currentLibrary ? `${$samples.length} files` : "No library"}
            </Badge>
          </div>
        </div>
        <FileList />
      {:else if activeView === "organise"}
        <div class="flex h-12 shrink-0 items-center gap-2 border-b bg-background px-4">
          <Boxes class="size-4 text-muted-foreground" />
          <h1 class="text-sm font-medium">Organise</h1>
        </div>
        <div class="grid flex-1 place-items-center p-6">
          <Card class="w-full max-w-xl">
            <CardHeader>
              <CardTitle>Organisation workflow</CardTitle>
            </CardHeader>
            <CardContent>
              <p class="text-sm text-muted-foreground">
                Pattern presets, preview, and apply controls will be added after tagging data exists.
              </p>
            </CardContent>
          </Card>
        </div>
      {:else}
        <div class="flex h-12 shrink-0 items-center gap-2 border-b bg-background px-4">
          <History class="size-4 text-muted-foreground" />
          <h1 class="text-sm font-medium">History</h1>
        </div>
        <div class="grid flex-1 place-items-center p-6">
          <Card class="w-full max-w-xl">
            <CardHeader>
              <CardTitle>Operation history</CardTitle>
            </CardHeader>
            <CardContent>
              <p class="text-sm text-muted-foreground">
                Move/copy batches and rollback controls will appear here once file operations exist.
              </p>
            </CardContent>
          </Card>
        </div>
      {/if}
    </main>
  </div>

  <Separator />

  <PlaybackFooter />
</div>
