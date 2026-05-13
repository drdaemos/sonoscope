<script lang="ts">
  import { FileAudio } from "@lucide/svelte";
  import { Badge } from "$lib/components/ui";
  import { currentLibrary, samples } from "$lib/stores/library";

  function formatBytes(bytes: number | null): string {
    if (bytes == null) return "-";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function statusVariant(status: string): "default" | "destructive" | "muted" | "secondary" {
    switch (status) {
      case "done":
        return "default";
      case "failed":
        return "destructive";
      case "analysing":
        return "secondary";
      default:
        return "muted";
    }
  }
</script>

<div class="flex min-h-0 flex-1 flex-col bg-card">
  {#if $samples.length === 0}
    <div class="flex flex-1 items-center justify-center p-8">
      <div class="flex max-w-sm flex-col items-center text-center">
        <div class="mb-4 flex size-12 items-center justify-center rounded-lg border bg-muted">
          <FileAudio class="size-5 text-muted-foreground" />
        </div>
        <h2 class="text-sm font-medium">
          {$currentLibrary ? "No samples discovered" : "Open a library"}
        </h2>
        <p class="mt-1 text-sm text-muted-foreground">
          {$currentLibrary
            ? "Run a scan to populate the review list."
            : "Choose a folder to create or load a Sonoscope library."}
        </p>
      </div>
    </div>
  {:else}
    <div class="min-h-0 flex-1 overflow-auto">
      <table class="w-full table-fixed text-left text-sm">
        <thead class="sticky top-0 z-10 border-b bg-card">
          <tr class="text-xs text-muted-foreground">
            <th class="w-[30%] px-4 py-2 font-medium">Filename</th>
            <th class="px-4 py-2 font-medium">Path</th>
            <th class="w-24 px-4 py-2 font-medium">Format</th>
            <th class="w-28 px-4 py-2 font-medium">Size</th>
            <th class="w-28 px-4 py-2 font-medium">Status</th>
          </tr>
        </thead>
        <tbody class="divide-y">
          {#each $samples as sample (sample.id)}
            <tr class="hover:bg-muted/50">
              <td class="truncate px-4 py-2 font-mono text-xs">{sample.filename}</td>
              <td class="truncate px-4 py-2 font-mono text-xs text-muted-foreground">
                {sample.relative_path}
              </td>
              <td class="px-4 py-2 text-xs uppercase text-muted-foreground">
                {sample.format ?? "-"}
              </td>
              <td class="px-4 py-2 text-xs text-muted-foreground">
                {formatBytes(sample.size_bytes)}
              </td>
              <td class="px-4 py-2">
                <Badge variant={statusVariant(sample.analysis_status)}>
                  {sample.analysis_status}
                </Badge>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>
