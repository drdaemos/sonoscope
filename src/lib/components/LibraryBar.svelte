<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import BrainCircuit from "@lucide/svelte/icons/brain-circuit";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Download from "@lucide/svelte/icons/download";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import X from "@lucide/svelte/icons/x";
  import { onMount } from "svelte";
  import {
    commands,
    type AnalysisScope,
    type CommandError,
    type MlModelStatus,
  } from "$lib/bindings/bindings";
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
  let mlDownloadPercent = $state<number | null>(null);
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
        await startAnalysis("pending");
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

  async function startAnalysis(scope: AnalysisScope) {
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

    const result = await commands.startAnalysis(scope);
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
          mlDownloadPercent = overallDownloadPercent(event.payload);
        }),
        listen<MlModelStatus>("ml-model-download-complete", (event) => {
          mlModelStatus = event.payload;
          isDownloadingMl = false;
          mlDownloadStatus = null;
          mlDownloadPercent = null;
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
      mlDownloadPercent = null;
    }
  }

  /// Coarse overall progress across all files so the header control keeps a
  /// stable width while downloading.
  function overallDownloadPercent(progress: MlModelDownloadProgress): number | null {
    if (progress.file_count <= 0) return null;
    let fileFraction = 0;
    if (progress.total_bytes && progress.total_bytes > 0) {
      fileFraction = Math.min(1, progress.downloaded_bytes / progress.total_bytes);
    }
    const overall = ((progress.file_index - 1 + fileFraction) / progress.file_count) * 100;
    return Math.max(0, Math.min(100, Math.floor(overall)));
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

<header class="flex h-14 shrink-0 items-center gap-4 border-b bg-background px-4">
  <div class="flex shrink-0 items-center gap-3">
    <div class="flex size-8 items-center justify-center rounded-md bg-primary text-primary-foreground">
      <RefreshCw class="size-4" />
    </div>
    <div class="text-sm font-semibold">Sonoscope</div>

    <Tabs
      value={tabValue}
      onValueChange={(v) => { if (v) onViewChange(v as AppView); }}
      class="ml-2 items-center"
    >
      <TabsList>
        <TabsTrigger value="review">Review</TabsTrigger>
        <TabsTrigger value="organise">Organise</TabsTrigger>
        <TabsTrigger value="history">History</TabsTrigger>
      </TabsList>
    </Tabs>
  </div>

  <div class="flex min-w-0 flex-1 items-center justify-end gap-2">
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
      <Badge variant="destructive" class="min-w-0 max-w-56" title={openError.detail}>
        <span class="truncate">{openError.summary}</span>
      </Badge>
    {/if}

    {#if mlError}
      <Badge variant="destructive" class="min-w-0 max-w-56" title={mlError.detail}>
        <span class="truncate">{mlError.summary}</span>
      </Badge>
    {:else if mlModelStatus?.found}
      <Badge variant="secondary" class="gap-1" title={mlModelStatus.path}>
        <BrainCircuit class="size-3" />
        ML ready
      </Badge>
    {:else}
      <Button
        variant="outline"
        size="sm"
        class="w-28 shrink-0"
        onclick={downloadMlModel}
        disabled={isDownloadingMl}
        title={isDownloadingMl
          ? (mlDownloadStatus ?? "Downloading")
          : (mlModelStatus?.path ?? "Model cache unavailable")}
      >
        {#if isDownloadingMl}
          <RefreshCw class="shrink-0 animate-spin" />
          {mlDownloadPercent !== null ? `ML ${mlDownloadPercent}%` : "ML model"}
        {:else}
          <Download />
          ML model
        {/if}
      </Button>
    {/if}

    {#if $isDiscovering || $isAnalyzing}
      <div class="flex w-36 shrink-0 flex-col gap-1">
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
      <div class="flex shrink-0">
        <Button
          size="sm"
          class="whitespace-nowrap rounded-r-none"
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
            <DropdownMenuItem onclick={() => startAnalysis("untagged")}>
              Re-analyse untagged samples
            </DropdownMenuItem>
            <DropdownMenuItem onclick={() => startAnalysis("all")}>
              Re-analyse all samples
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    {/if}

    <div class="flex min-w-0">
      <Button
        variant="outline"
        size="sm"
        class="min-w-0 max-w-48 rounded-r-none border-r-0"
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
