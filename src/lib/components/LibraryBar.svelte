<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { BrainCircuit, ChevronDown, Download, RefreshCw, X } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { commands, type CommandError, type MlModelStatus } from "$lib/bindings/bindings";
  import {
    Badge,
    Button,
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
    Progress,
    Tabs,
    TabsList,
    TabsTrigger,
  } from "$lib/components/ui";
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
    tagDimensions,
  } from "$lib/stores/library";

  export type AppView = "review" | "organise" | "history";

  type LibraryBarProps = {
    activeView: AppView;
    onViewChange: (view: AppView) => void;
  };

  type MlModelDownloadProgress = {
    file_name: string;
    file_index: number;
    file_count: number;
    downloaded_bytes: number;
    total_bytes: number | null;
  };

  let { activeView, onViewChange }: LibraryBarProps = $props();
  let discoveryUnlisten: (() => void) | null = null;
  let analysisUnlisten: (() => void) | null = null;
  let openError = $state<{ summary: string; detail: string } | null>(null);
  let mlError = $state<{ summary: string; detail: string } | null>(null);
  let mlModelStatus = $state<MlModelStatus | null>(null);
  let isDownloadingMl = $state(false);
  let mlDownloadStatus = $state<string | null>(null);
  let recentMenuOpen = $state(false);

  let tabValue = $derived(activeView);

  onMount(() => {
    void refreshMlModelStatus();
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
    tagDimensions.set([]);
    discoveryCount.set(0);
    analysisProcessed.set(0);
    analysisTotal.set(0);
    rememberLibrary(result.data.root_path);

    const [samplesResult, dimensionsResult] = await Promise.all([
      commands.getSamples(),
      commands.listTagDimensions(),
    ]);
    if (samplesResult.status === "error") {
      console.error("Failed to load samples:", samplesResult.error);
      openError = formatCommandError(samplesResult.error);
      return;
    }
    if (dimensionsResult.status === "error") {
      console.error("Failed to load tag dimensions:", dimensionsResult.error);
      openError = formatCommandError(dimensionsResult.error);
      return;
    }

    samples.set(samplesResult.data);
    tagDimensions.set(dimensionsResult.data);
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
        // Only newly discovered (pending) samples are analysed; use
        // "Re-analyse" for a full ML re-run.
        await startAnalysis(false);
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

  async function startAnalysis(reanalyze: boolean) {
    stopAnalysisListeners();
    isAnalyzing.set(true);
    analysisProcessed.set(0);
    analysisTotal.set(0);

    const finishAnalysis = async (payload: { processed: number; total: number }) => {
      analysisProcessed.set(payload.processed);
      analysisTotal.set(payload.total);
      isAnalyzing.set(false);
      stopAnalysisListeners();

      const result = await commands.getSamples();
      if (result.status === "ok") samples.set(result.data);
    };

    const [unlistenProgress, unlistenComplete, unlistenCancelled, unlistenFailed] =
      await Promise.all([
        listen<{ processed: number; total: number }>("analysis-progress", (event) => {
          analysisProcessed.set(event.payload.processed);
          analysisTotal.set(event.payload.total);
        }),
        listen<{ processed: number; total: number }>("analysis-complete", (event) =>
          finishAnalysis(event.payload),
        ),
        listen<{ processed: number; total: number }>("analysis-cancelled", (event) =>
          finishAnalysis(event.payload),
        ),
        listen<{ error: string }>("analysis-failed", (event) => {
          openError = { summary: "Analysis failed", detail: event.payload.error };
          isAnalyzing.set(false);
          stopAnalysisListeners();
        }),
      ]);

    analysisUnlisten = () => {
      unlistenProgress();
      unlistenComplete();
      unlistenCancelled();
      unlistenFailed();
    };

    const result = await commands.startAnalysis(reanalyze);
    if (result.status === "error") {
      console.error("Failed to start analysis:", result.error);
      isAnalyzing.set(false);
      stopAnalysisListeners();
    }
  }

  async function cancelAnalysis() {
    const result = await commands.cancelAnalysis();
    if (result.status === "error") {
      console.error("Failed to cancel analysis:", result.error);
    }
  }

  async function refreshMlModelStatus(): Promise<MlModelStatus | null> {
    try {
      const result = await commands.getMlModelStatus();
      if (result.status === "error") {
        mlError = formatCommandError(result.error);
        isDownloadingMl = false;
        mlDownloadStatus = null;
        return null;
      }
      mlError = null;
      const status = result.data;
      mlModelStatus = status;
      if (status.found) {
        isDownloadingMl = false;
        mlDownloadStatus = null;
      }
      return status;
    } catch (error) {
      mlError = {
        summary: "ML status failed",
        detail: error instanceof Error ? error.message : String(error),
      };
      isDownloadingMl = false;
      mlDownloadStatus = null;
      return null;
    }
  }

  async function downloadMlModel() {
    isDownloadingMl = true;
    mlError = null;
    mlDownloadStatus = "Checking";
    let cleanupDownloadListeners: (() => void) | null = null;
    try {
      const currentStatus = await refreshMlModelStatus();
      if (currentStatus === null || currentStatus.found) return;

      mlDownloadStatus = "Preparing";
      const [unlistenProgress, unlistenComplete] = await Promise.all([
        listen<MlModelDownloadProgress>("ml-model-download-progress", (event) => {
          mlDownloadStatus = formatMlDownloadProgress(event.payload);
        }),
        listen<MlModelStatus>("ml-model-download-complete", (event) => {
          mlModelStatus = event.payload;
          isDownloadingMl = false;
          mlDownloadStatus = null;
          cleanupDownloadListeners?.();
        }),
      ]);
      cleanupDownloadListeners = () => {
        unlistenProgress();
        unlistenComplete();
        cleanupDownloadListeners = null;
      };
      const result = await commands.downloadMlModel();
      if (result.status === "error") {
        mlError = formatCommandError(result.error);
        return;
      }
      mlModelStatus = result.data;
    } catch (error) {
      mlError = {
        summary: "ML download failed",
        detail: error instanceof Error ? error.message : String(error),
      };
    } finally {
      cleanupDownloadListeners?.();
      isDownloadingMl = false;
      mlDownloadStatus = null;
    }
  }

  function formatMlDownloadProgress(progress: MlModelDownloadProgress): string {
    const prefix = `${progress.file_index}/${progress.file_count} ${progress.file_name}`;
    if (progress.total_bytes && progress.total_bytes > 0) {
      const percent = Math.floor((progress.downloaded_bytes / progress.total_bytes) * 100);
      return `${prefix} ${percent}%`;
    }
    if (progress.downloaded_bytes > 0) {
      return `${prefix} ${formatBytes(progress.downloaded_bytes)}`;
    }
    return prefix;
  }

  function formatBytes(value: number): string {
    if (value >= 1024 * 1024) return `${Math.floor(value / (1024 * 1024))} MB`;
    if (value >= 1024) return `${Math.floor(value / 1024)} KB`;
    return `${value} B`;
  }

  function actionLabel(): string {
    if ($isDiscovering) return "Scanning";
    if ($isAnalyzing) return "Analysing";
    return $samples.length > 0 ? "Re-scan" : "Scan and analyse";
  }

  function progressPercent(): number {
    if ($isAnalyzing && $analysisTotal > 0) {
      return Math.max(0, Math.min(100, Math.round(($analysisProcessed / $analysisTotal) * 100)));
    }
    return 0;
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

<header class="grid h-14 shrink-0 grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center gap-4 border-b bg-background px-4">
  <div class="flex min-w-0 items-center gap-3">
    <div class="flex size-8 items-center justify-center rounded-md bg-primary text-primary-foreground">
      <RefreshCw class="size-4" />
    </div>
    <div class="truncate text-sm font-semibold">Sonoscope</div>
  </div>

  <Tabs
    value={tabValue}
    onValueChange={(v) => { if (v) onViewChange(v as AppView); }}
    class="items-center"
  >
    <TabsList>
      <TabsTrigger value="review">Review</TabsTrigger>
      <TabsTrigger value="organise">Organise</TabsTrigger>
      <TabsTrigger value="history">History</TabsTrigger>
    </TabsList>
  </Tabs>

  <div class="flex min-w-0 items-center justify-end gap-2">
    {#if $isDiscovering}
      <Button variant="outline" size="sm" onclick={cancelScan}>
        <X />
        Cancel
      </Button>
    {/if}

    {#if $isAnalyzing}
      <Button variant="outline" size="sm" onclick={cancelAnalysis}>
        <X />
        Cancel
      </Button>
    {/if}

    {#if openError}
      <Badge variant="destructive" title={openError.detail}>{openError.summary}</Badge>
    {/if}

    {#if mlError}
      <Badge variant="destructive" title={mlError.detail}>{mlError.summary}</Badge>
    {:else if mlModelStatus?.found}
      <Badge variant="secondary" class="gap-1" title={mlModelStatus.path}>
        <BrainCircuit class="size-3" />
        ML ready
      </Badge>
    {:else}
      <Button
        variant="outline"
        size="sm"
        onclick={downloadMlModel}
        disabled={isDownloadingMl}
        title={mlModelStatus?.path ?? "Model cache unavailable"}
      >
        {#if isDownloadingMl}
          <RefreshCw class="animate-spin" />
          {mlDownloadStatus ?? "Downloading"}
        {:else}
          <Download />
          ML model
        {/if}
      </Button>
    {/if}

    {#if $isDiscovering || $isAnalyzing}
      <div class="flex w-36 flex-col gap-1">
        <Badge variant="secondary">
          {$isDiscovering
            ? `${$discoveryCount} discovered`
            : $analysisTotal > 0
              ? `${$analysisProcessed} / ${$analysisTotal} analysed`
              : "Preparing analysis"}
        </Badge>
        <Progress value={progressPercent()} class="h-1" />
      </div>
    {/if}

    {#if $currentLibrary}
      <div class="flex w-44">
        <Button
          size="sm"
          class="w-full rounded-r-none"
          onclick={startScanAndAnalysis}
          disabled={$isDiscovering || $isAnalyzing}
        >
          <RefreshCw />
          {actionLabel()}
        </Button>
        <DropdownMenu>
          <DropdownMenuTrigger>
            {#snippet child({ props })}
              <Button
                {...props}
                size="icon-sm"
                class="rounded-l-none border-l border-l-primary-foreground/20"
                aria-label="Scan options"
                disabled={$isDiscovering || $isAnalyzing}
              >
                <ChevronDown />
              </Button>
            {/snippet}
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onclick={() => startAnalysis(true)}>
              Re-analyse all samples
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    {/if}

    <div class="flex">
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

      <DropdownMenu bind:open={recentMenuOpen}>
        <DropdownMenuTrigger>
          {#snippet child({ props })}
            <Button
              {...props}
              variant="outline"
              size="icon-sm"
              class="rounded-l-none"
              aria-label="Recent libraries"
            >
              <ChevronDown />
            </Button>
          {/snippet}
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" class="w-64">
          {#each $recentLibraries.filter((library) => library.path !== $currentLibrary?.root_path) as library}
            <DropdownMenuItem onclick={() => openLibraryPath(library.path)}>
              <div class="flex flex-col">
                <span class="truncate font-medium">{library.name}</span>
                <span class="truncate text-xs text-muted-foreground">{library.path}</span>
              </div>
            </DropdownMenuItem>
          {:else}
            <DropdownMenuItem disabled>No recent libraries</DropdownMenuItem>
          {/each}
          <DropdownMenuItem onclick={pickLibrary}>Browse...</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  </div>
</header>
