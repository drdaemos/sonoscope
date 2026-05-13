<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { ChevronDown, RefreshCw, X } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { commands, type CommandError } from "$lib/bindings/bindings";
  import { Badge, Button, Tabs, TabsList, TabsTrigger } from "$lib/components/ui";
  import {
    analysisProcessed,
    analysisTotal,
    currentLibrary,
    discoveryCount,
    isAnalyzing,
    isDiscovering,
    libraryDisplayName,
    recentLibraries,
    rememberLibrary,
    samples,
  } from "$lib/stores/library";

  export type AppView = "review" | "organise" | "history";

  type LibraryBarProps = {
    activeView: AppView;
    onViewChange: (view: AppView) => void;
  };

  let { activeView, onViewChange }: LibraryBarProps = $props();
  let discoveryUnlisten: (() => void) | null = null;
  let analysisUnlisten: (() => void) | null = null;
  let openError = $state<{ summary: string; detail: string } | null>(null);
  let recentMenuOpen = $state(false);

  onMount(() => {
    return () => {
      stopDiscoveryListeners();
      stopAnalysisListeners();
    };
  });

  function stopDiscoveryListeners() {
    if (discoveryUnlisten) {
      discoveryUnlisten();
      discoveryUnlisten = null;
    }
  }

  function stopAnalysisListeners() {
    if (analysisUnlisten) {
      analysisUnlisten();
      analysisUnlisten = null;
    }
  }

  async function pickLibrary() {
    openError = null;
    recentMenuOpen = false;
    const selected = await openDialog({ directory: true, multiple: false });
    if (!selected) return;

    const path = typeof selected === "string" ? selected : selected[0];
    if (!path) return;

    await openLibraryPath(path);
  }

  async function openLibraryPath(path: string) {
    recentMenuOpen = false;
    const result = await commands.openLibrary(path);
    if (result.status === "error") {
      console.error("Failed to open library:", result.error);
      openError = formatCommandError(result.error);
      return;
    }

    currentLibrary.set(result.data);
    samples.set([]);
    discoveryCount.set(0);
    analysisProcessed.set(0);
    analysisTotal.set(0);
    rememberLibrary(result.data.root_path);

    const samplesResult = await commands.getSamples();
    if (samplesResult.status === "error") {
      console.error("Failed to load samples:", samplesResult.error);
      openError = formatCommandError(samplesResult.error);
      return;
    }

    samples.set(samplesResult.data);
  }

  async function startScanAndAnalysis() {
    stopDiscoveryListeners();
    stopAnalysisListeners();
    openError = null;

    isDiscovering.set(true);
    isAnalyzing.set(false);
    discoveryCount.set(0);
    analysisProcessed.set(0);
    analysisTotal.set(0);

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
        await startAnalysis();
      }),
      listen<{ count: number }>("discovery-cancelled", (event) => {
        discoveryCount.set(event.payload.count);
        isDiscovering.set(false);
        stopDiscoveryListeners();
      }),
    ]);

    discoveryUnlisten = () => {
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

  async function startAnalysis() {
    stopAnalysisListeners();
    isAnalyzing.set(true);
    analysisProcessed.set(0);
    analysisTotal.set(0);

    const [unlistenProgress, unlistenComplete] = await Promise.all([
      listen<{ processed: number; total: number }>("analysis-progress", (event) => {
        analysisProcessed.set(event.payload.processed);
        analysisTotal.set(event.payload.total);
      }),
      listen<{ processed: number; total: number }>("analysis-complete", async (event) => {
        analysisProcessed.set(event.payload.processed);
        analysisTotal.set(event.payload.total);
        isAnalyzing.set(false);
        stopAnalysisListeners();

        const result = await commands.getSamples();
        if (result.status === "ok") samples.set(result.data);
      }),
    ]);

    analysisUnlisten = () => {
      unlistenProgress();
      unlistenComplete();
    };

    const result = await commands.startAnalysis(true);
    if (result.status === "error") {
      console.error("Failed to start analysis:", result.error);
      isAnalyzing.set(false);
      stopAnalysisListeners();
    }
  }

  function actionLabel(): string {
    if ($isDiscovering) return "Scanning";
    if ($isAnalyzing) return "Analysing";
    return $samples.length > 0 ? "Re-scan" : "Scan and analyse";
  }

  function progressPercent(): number {
    if ($isDiscovering) return 35;
    if ($isAnalyzing && $analysisTotal > 0) {
      return Math.max(35, Math.round(($analysisProcessed / $analysisTotal) * 100));
    }
    return 0;
  }

  function toggleRecentMenu() {
    recentMenuOpen = !recentMenuOpen;
  }

  function formatCommandError(error: CommandError): { summary: string; detail: string } {
    if (typeof error === "string") {
      return { summary: error, detail: error };
    }
    if ("Database" in error && error.Database) {
      return {
        summary: "Database error: remove library.db",
        detail: `Remove the existing library.db file in the selected folder, then open the folder again. Details: ${error.Database}`,
      };
    }
    if ("Io" in error && error.Io) {
      return { summary: "File access error", detail: error.Io };
    }
    if ("Analysis" in error && error.Analysis) {
      return { summary: "Analysis error", detail: error.Analysis };
    }
    if ("DiscoveryCancelled" in error && error.DiscoveryCancelled) {
      return {
        summary: "Discovery cancelled",
        detail: `${error.DiscoveryCancelled.count} files found before cancellation.`,
      };
    }
    if ("Other" in error && error.Other) {
      return { summary: "Open failed", detail: error.Other };
    }
    if (error && typeof error === "object") {
      const detail = JSON.stringify(error);
      return { summary: "Open failed", detail };
    }
    return { summary: "Open failed", detail: "Unknown error" };
  }
</script>

<header class="grid h-14 shrink-0 grid-cols-[180px_1fr_auto] items-center gap-4 border-b bg-background px-4">
  <div class="flex min-w-0 items-center gap-3">
    <div class="flex size-8 items-center justify-center rounded-md bg-primary text-primary-foreground">
      <RefreshCw class="size-4" />
    </div>
    <div class="truncate text-sm font-semibold">Sonoscope</div>
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
      <Button variant="outline" size="sm" onclick={cancelScan}>
        <X />
        Cancel
      </Button>
    {/if}

    {#if openError}
      <Badge variant="destructive" title={openError.detail}>{openError.summary}</Badge>
    {/if}

    {#if $isDiscovering || $isAnalyzing}
      <div class="flex w-36 flex-col gap-1">
        <Badge variant="secondary">
          {$isDiscovering
            ? `${$discoveryCount} discovered`
            : `${$analysisProcessed} / ${$analysisTotal} analysed`}
        </Badge>
        <div class="h-1 overflow-hidden rounded-full bg-muted">
          <div
            class="h-full bg-primary transition-[width]"
            style={`width: ${progressPercent()}%`}
          ></div>
        </div>
      </div>
    {/if}

    {#if $currentLibrary}
      <div class="flex w-36">
        <Button
          size="sm"
          class="w-full"
          onclick={startScanAndAnalysis}
          disabled={$isDiscovering || $isAnalyzing}
        >
          <RefreshCw />
          {actionLabel()}
        </Button>
      </div>
    {/if}

    <div class="relative flex">
      <Button
        variant="outline"
        size="sm"
        class="max-w-48 rounded-r-none border-r-0"
        onclick={$currentLibrary ? undefined : pickLibrary}
        title={$currentLibrary?.root_path ?? "Open library"}
      >
        <span class="truncate">
          {$currentLibrary ? libraryDisplayName($currentLibrary.root_path) : "Open Library"}
        </span>
      </Button>
      <Button
        variant="outline"
        size="icon"
        class="h-8 w-8 rounded-l-none"
        aria-label="Recent libraries"
        onclick={toggleRecentMenu}
      >
        <ChevronDown />
      </Button>

      {#if recentMenuOpen}
        <div
          class="absolute right-0 top-9 z-50 w-64 overflow-hidden rounded-md border bg-popover py-1 text-sm shadow-md"
        >
          {#each $recentLibraries.filter((library) => library.path !== $currentLibrary?.root_path) as library}
            <button
              type="button"
              class="flex w-full flex-col px-3 py-2 text-left hover:bg-accent"
              onclick={() => openLibraryPath(library.path)}
            >
              <span class="truncate font-medium">{library.name}</span>
              <span class="truncate text-xs text-muted-foreground">{library.path}</span>
            </button>
          {:else}
            <div class="px-3 py-2 text-xs text-muted-foreground">No recent libraries</div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</header>
