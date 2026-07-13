<script lang="ts">
  import LoaderCircle from "@lucide/svelte/icons/loader-circle";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import Undo2 from "@lucide/svelte/icons/undo-2";
  import { commands, type OperationBatch } from "$lib/bindings/bindings";
  import { Badge, Button } from "$lib/components/ui";
  import { currentLibrary, samples } from "$lib/stores/library";
  import { formatBatchTimestamp } from "$lib/stores/organise";

  let batches = $state<OperationBatch[]>([]);
  let isLoading = $state(false);
  let rollbackTarget = $state<OperationBatch | null>(null);
  let rollingBackId = $state<number | null>(null);
  let lastRollback = $state<{ restored: number; skipped: number } | null>(null);
  let error = $state<string | null>(null);

  // Batches live in the library database, so reload them per library.
  $effect(() => {
    if ($currentLibrary) {
      void loadBatches();
    } else {
      batches = [];
    }
  });

  function formatError(commandError: unknown): string {
    return typeof commandError === "string" ? commandError : JSON.stringify(commandError);
  }

  async function loadBatches(): Promise<void> {
    isLoading = true;
    const result = await commands.listOperationBatches();
    isLoading = false;
    if (result.status === "ok") {
      batches = result.data;
      error = null;
    } else {
      batches = [];
      error = formatError(result.error);
    }
  }

  async function rollback(batch: OperationBatch): Promise<void> {
    rollbackTarget = null;
    rollingBackId = batch.id;
    lastRollback = null;
    const result = await commands.rollbackOperationBatch(batch.id);
    rollingBackId = null;
    if (result.status === "ok") {
      lastRollback = result.data;
      error = null;
      const refreshed = await commands.getSamples();
      if (refreshed.status === "ok") samples.set(refreshed.data);
      await loadBatches();
    } else {
      error = formatError(result.error);
    }
  }
</script>

{#if !$currentLibrary}
  <div class="grid flex-1 place-items-center p-6 text-sm text-muted-foreground">
    Open a library to see its file operation history.
  </div>
{:else}
  <div class="flex min-h-0 flex-1 flex-col">
    {#if error}
      <p class="flex items-center gap-1.5 border-b bg-background px-4 py-2 text-xs text-destructive">
        <TriangleAlert class="size-3.5 shrink-0" />
        {error}
      </p>
    {/if}
    {#if lastRollback}
      <p class="border-b bg-background px-4 py-2 text-xs text-muted-foreground">
        Rolled back: {lastRollback.restored} files restored, {lastRollback.skipped} skipped.
      </p>
    {/if}

    <div class="min-h-0 flex-1 overflow-auto">
      {#if batches.length > 0}
        <div class="divide-y">
          {#each batches as batch (batch.id)}
            <div
              class="flex items-center gap-3 px-4 py-2 text-sm {batch.status === 'rolled_back'
                ? 'text-muted-foreground'
                : ''}"
            >
              <span class="w-36 shrink-0 tabular-nums text-muted-foreground">
                {formatBatchTimestamp(batch.created_at)}
              </span>
              <Badge variant="outline" class="w-14 shrink-0 justify-center capitalize">
                {batch.mode}
              </Badge>
              <span class="min-w-0 flex-1 truncate font-mono text-xs" title={batch.pattern}>
                {batch.pattern}
              </span>
              <span class="shrink-0 tabular-nums text-muted-foreground">
                {batch.file_count} files
              </span>
              {#if batch.status === "rolled_back"}
                <Badge variant="secondary" class="shrink-0">Rolled back</Badge>
              {:else if batch.mode === "move"}
                <Button
                  variant="outline"
                  size="sm"
                  class="shrink-0"
                  disabled={rollingBackId !== null}
                  onclick={() => (rollbackTarget = batch)}
                >
                  {#if rollingBackId === batch.id}
                    <LoaderCircle class="animate-spin" />
                  {:else}
                    <Undo2 />
                  {/if}
                  Roll back
                </Button>
              {:else}
                <span class="shrink-0 text-xs text-muted-foreground">Not reversible</span>
              {/if}
            </div>
          {/each}
        </div>
      {:else}
        <div class="grid h-full place-items-center text-sm text-muted-foreground">
          {isLoading ? "Loading history..." : "No file operations yet."}
        </div>
      {/if}
    </div>
  </div>
{/if}

{#if rollbackTarget}
  {@const target = rollbackTarget}
  <div
    class="fixed inset-0 z-50 grid place-items-center bg-background/70 p-4 backdrop-blur-sm"
    role="presentation"
    onclick={(event) => {
      if (event.target === event.currentTarget) rollbackTarget = null;
    }}
  >
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="rollback-confirm-title"
      class="w-full max-w-md rounded-lg border bg-background p-5 shadow-xl"
    >
      <h2 id="rollback-confirm-title" class="text-sm font-semibold">
        Roll back {target.file_count} files?
      </h2>
      <p class="mt-2 text-sm text-muted-foreground">
        Files moved by <span class="font-mono">{target.pattern}</span> on
        {formatBatchTimestamp(target.created_at)} will be returned to their original locations.
      </p>
      <div class="mt-4 flex justify-end gap-2">
        <Button variant="outline" size="sm" onclick={() => (rollbackTarget = null)}>Cancel</Button>
        <Button size="sm" onclick={() => void rollback(target)}>Roll back</Button>
      </div>
    </div>
  </div>
{/if}
