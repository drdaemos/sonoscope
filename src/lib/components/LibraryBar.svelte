<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { FolderOpen, RefreshCw, X } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { commands } from "$lib/bindings/bindings";
  import { Badge, Button, Tabs, TabsList, TabsTrigger } from "$lib/components/ui";
  import {
    currentLibrary,
    discoveryCount,
    isDiscovering,
    samples,
  } from "$lib/stores/library";

  export type AppView = "review" | "organise" | "history";

  type LibraryBarProps = {
    activeView: AppView;
    onViewChange: (view: AppView) => void;
  };

  let { activeView, onViewChange }: LibraryBarProps = $props();
  let unlisten: (() => void) | null = null;

  onMount(() => {
    return () => {
      stopDiscoveryListeners();
    };
  });

  function stopDiscoveryListeners() {
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
  }

  function libraryName(rootPath: string): string {
    const normalized = rootPath.replaceAll("\\", "/").replace(/\/$/, "");
    return normalized.split("/").at(-1) ?? rootPath;
  }

  async function pickLibrary() {
    const selected = await openDialog({ directory: true, multiple: false });
    if (!selected) return;

    const path = typeof selected === "string" ? selected : selected[0];
    if (!path) return;

    const result = await commands.openLibrary(path);
    if (result.status === "error") {
      console.error("Failed to open library:", result.error);
      return;
    }
    currentLibrary.set(result.data);
    samples.set([]);
    discoveryCount.set(0);
  }

  async function startScan() {
    stopDiscoveryListeners();

    isDiscovering.set(true);
    discoveryCount.set(0);

    const [unlistenProgress, unlistenComplete, unlistenCancelled] = await Promise.all([
      listen<{ count: number }>("discovery-progress", (event) => {
        discoveryCount.set(event.payload.count);
      }),
      listen<{ total: number }>("discovery-complete", async (event) => {
        discoveryCount.set(event.payload.total);
        isDiscovering.set(false);
        stopDiscoveryListeners();
        const result = await commands.getSamples();
        if (result.status === "ok") samples.set(result.data);
      }),
      listen<{ count: number }>("discovery-cancelled", (event) => {
        discoveryCount.set(event.payload.count);
        isDiscovering.set(false);
        stopDiscoveryListeners();
      }),
    ]);

    unlisten = () => {
      unlistenProgress();
      unlistenComplete();
      unlistenCancelled();
    };

    const result = await commands.startDiscovery();
    if (result.status === "error") {
      console.error("Failed to start discovery:", result.error);
      isDiscovering.set(false);
      stopDiscoveryListeners();
    }
  }

  async function cancelScan() {
    const result = await commands.cancelDiscovery();
    if (result.status === "error") {
      console.error("Failed to cancel discovery:", result.error);
    }
  }
</script>

<header class="grid h-14 shrink-0 grid-cols-[minmax(260px,360px)_1fr_auto] items-center gap-4 border-b bg-background px-4">
  <div class="flex min-w-0 items-center gap-3">
    <div class="flex size-8 items-center justify-center rounded-md bg-primary text-primary-foreground">
      <RefreshCw class="size-4" />
    </div>
    <div class="min-w-0">
      <div class="truncate text-sm font-semibold">Sonoscope</div>
      <div class="truncate text-xs text-muted-foreground">
        {$currentLibrary ? libraryName($currentLibrary.root_path) : "No library open"}
      </div>
    </div>
  </div>

  <Tabs class="justify-center">
    <TabsList>
      <TabsTrigger active={activeView === "review"} onclick={() => onViewChange("review")}>
        Review
      </TabsTrigger>
      <TabsTrigger active={activeView === "organise"} onclick={() => onViewChange("organise")}>
        Organise
      </TabsTrigger>
      <TabsTrigger active={activeView === "history"} onclick={() => onViewChange("history")}>
        History
      </TabsTrigger>
    </TabsList>
  </Tabs>

  <div class="flex items-center justify-end gap-2">
    {#if $isDiscovering}
      <Badge variant="secondary">{$discoveryCount} found</Badge>
      <Button variant="outline" size="sm" onclick={cancelScan}>
        <X />
        Cancel
      </Button>
    {:else if $currentLibrary && $samples.length > 0}
      <Badge variant="muted">{$samples.length} files</Badge>
    {/if}

    {#if $currentLibrary}
      <Button size="sm" onclick={startScan} disabled={$isDiscovering}>
        <RefreshCw />
        Scan
      </Button>
    {/if}

    <Button variant="outline" size="sm" onclick={pickLibrary}>
      <FolderOpen />
      Open
    </Button>
  </div>
</header>
