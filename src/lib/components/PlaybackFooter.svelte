<script lang="ts">
  import { onDestroy } from "svelte";
  import Pause from "@lucide/svelte/icons/pause";
  import Play from "@lucide/svelte/icons/play";
  import Repeat2 from "@lucide/svelte/icons/repeat-2";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import VolumeX from "@lucide/svelte/icons/volume-x";
  import { Button, Slider, Toggle } from "$lib/components/ui";
  import {
    formatPlaybackTime,
    loadPlaybackSample,
    playbackAutoplay,
    playbackError,
    playbackSample,
    playbackStatus,
  } from "$lib/stores/playback";
  import { selectedSampleIds } from "$lib/stores/review";

  let audioContext = $state<AudioContext | null>(null);
  let gainNode = $state<GainNode | null>(null);
  let sourceNode = $state<AudioBufferSourceNode | null>(null);
  let audioBuffer = $state<AudioBuffer | null>(null);
  let currentTime = $state(0);
  let duration = $state(0);
  let volume = $state(0.85);
  let muted = $state(false);
  let loadedSampleId = $state<number | null>(null);
  let loopEnabled = $state(false);
  let startedAt = $state(0);
  let pausedAt = $state(0);
  let animationFrameId: number | null = null;
  let loadToken = 0;
  let stoppingSource = false;

  let selectedSampleId = $derived([...$selectedSampleIds][0]);
  let canPlay = $derived(audioBuffer !== null && $playbackStatus !== "loading");
  let isPlaying = $derived($playbackStatus === "playing");
  let effectiveDuration = $derived(
    duration > 0 ? duration : (($playbackSample?.duration_ms ?? 0) / 1000),
  );
  let progressValue = $derived(
    effectiveDuration > 0 ? Math.round((currentTime / effectiveDuration) * 1000) : 0,
  );
  let footerLabel = $derived(
    $playbackError ??
      $playbackSample?.filename ??
      (selectedSampleId === undefined ? "No sample loaded" : "Ready to load selected sample"),
  );

  $effect(() => {
    if (!gainNode) return;
    gainNode.gain.value = muted ? 0 : volume;
  });

  $effect(() => {
    if (!sourceNode) return;
    sourceNode.loop = loopEnabled;
  });

  $effect(() => {
    const sample = $playbackSample;
    const sampleId = sample?.id ?? null;
    if (sampleId === loadedSampleId) return;

    loadToken += 1;
    loadedSampleId = sampleId;
    resetPlaybackPosition();
    audioBuffer = null;
    duration = (sample?.duration_ms ?? 0) / 1000;
    loopEnabled = sample?.is_loop === true;
    stopSource();

    if (!sample) return;
    void decodeSample(sample.src, loadToken);
  });

  onDestroy(() => {
    loadToken += 1;
    stopSource();
    stopProgressTimer();
    void audioContext?.close();
  });

  async function togglePlayback() {
    if (!canPlay) {
      if (selectedSampleId !== undefined) {
        await loadPlaybackSample(selectedSampleId, { autoplay: true });
      }
      return;
    }

    if (isPlaying) {
      pauseAudio();
      return;
    }

    await playAudio();
  }

  async function decodeSample(src: string, token: number) {
    playbackStatus.set("loading");
    playbackError.set(null);

    try {
      const context = ensureAudioContext();
      const response = await fetch(src);
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const bytes = await response.arrayBuffer();
      const decoded = await context.decodeAudioData(bytes);
      if (token !== loadToken) return;

      audioBuffer = decoded;
      duration = decoded.duration;
      playbackStatus.set("ready");

      if ($playbackAutoplay) {
        playbackAutoplay.set(false);
        await playAudio();
      }
    } catch (error) {
      if (token !== loadToken) return;
      playbackStatus.set("error");
      playbackError.set("Unable to load sample audio");
      console.error("Failed to decode sample:", error);
    }
  }

  function ensureAudioContext(): AudioContext {
    if (!audioContext) {
      audioContext = new AudioContext();
      gainNode = audioContext.createGain();
      gainNode.gain.value = muted ? 0 : volume;
      gainNode.connect(audioContext.destination);
    }
    return audioContext;
  }

  async function playAudio() {
    if (!audioBuffer) return;
    try {
      const context = ensureAudioContext();
      await context.resume();
      stopSource();

      const startOffset = normalizedOffset(pausedAt);
      const source = context.createBufferSource();
      source.buffer = audioBuffer;
      source.loop = loopEnabled;
      source.connect(gainNode ?? context.destination);
      source.onended = () => {
        if (stoppingSource || sourceNode !== source || loopEnabled) return;
        sourceNode = null;
        resetPlaybackPosition();
        stopProgressTimer();
        playbackStatus.set("paused");
      };

      startedAt = context.currentTime - startOffset;
      pausedAt = startOffset;
      sourceNode = source;
      source.start(0, startOffset);
      playbackError.set(null);
      playbackStatus.set("playing");
      startProgressTimer();
    } catch (error) {
      playbackStatus.set("error");
      playbackError.set("Unable to play sample");
      console.error("Failed to play sample:", error);
    }
  }

  function pauseAudio() {
    pausedAt = playbackPosition();
    currentTime = pausedAt;
    stopSource();
    stopProgressTimer();
    playbackStatus.set("paused");
  }

  function stopSource() {
    const source = sourceNode;
    if (!source) return;

    stoppingSource = true;
    sourceNode = null;
    source.onended = null;
    try {
      source.stop();
    } catch {
      // Already stopped.
    } finally {
      stoppingSource = false;
    }
  }

  function playbackPosition(): number {
    if (!audioContext || !audioBuffer || $playbackStatus !== "playing") return normalizedOffset(pausedAt);
    const elapsed = Math.max(0, audioContext.currentTime - startedAt);
    if (loopEnabled) return elapsed % audioBuffer.duration;
    return Math.min(elapsed, audioBuffer.duration);
  }

  function normalizedOffset(offset: number): number {
    if (!audioBuffer || audioBuffer.duration <= 0) return 0;
    if (loopEnabled) return ((offset % audioBuffer.duration) + audioBuffer.duration) % audioBuffer.duration;
    return Math.max(0, Math.min(offset, audioBuffer.duration));
  }

  function startProgressTimer() {
    stopProgressTimer();
    const tick = () => {
      currentTime = playbackPosition();
      animationFrameId = requestAnimationFrame(tick);
    };
    animationFrameId = requestAnimationFrame(tick);
  }

  function stopProgressTimer() {
    if (animationFrameId === null) return;
    cancelAnimationFrame(animationFrameId);
    animationFrameId = null;
  }

  function resetPlaybackPosition() {
    pausedAt = 0;
    currentTime = 0;
    startedAt = audioContext?.currentTime ?? 0;
    stopProgressTimer();
  }

  function seekTo(value: number) {
    if (!audioBuffer || effectiveDuration <= 0) return;
    const nextTime = Math.max(0, Math.min(effectiveDuration, (value / 1000) * effectiveDuration));
    pausedAt = normalizedOffset(nextTime);
    currentTime = pausedAt;
    if (isPlaying) void playAudio();
  }

  function setVolume(value: number) {
    volume = Math.max(0, Math.min(1, value / 100));
    if (volume > 0) muted = false;
  }

  function toggleMute() {
    muted = !muted;
  }

  function handleGlobalKeydown(event: KeyboardEvent) {
    if (event.key !== " ") return;
    if (
      event.target instanceof HTMLElement &&
      event.target.closest("input, textarea, button, [role='slider']")
    ) {
      return;
    }
    if (!$playbackSample && selectedSampleId === undefined) return;
    event.preventDefault();
    void togglePlayback();
  }
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<footer class="flex h-12 shrink-0 items-center gap-3 bg-background px-4 text-sm">
  <Button
    variant="ghost"
    size="icon"
    disabled={$playbackStatus === "loading" || (!$playbackSample && selectedSampleId === undefined)}
    aria-label={isPlaying ? "Pause sample" : "Play sample"}
    onclick={togglePlayback}
  >
    {#if isPlaying}
      <Pause />
    {:else}
      <Play />
    {/if}
  </Button>

  <Toggle
    bind:pressed={loopEnabled}
    variant="outline"
    size="sm"
    disabled={!$playbackSample}
    aria-label={loopEnabled ? "Disable sample loop" : "Enable sample loop"}
  >
    <Repeat2 />
    Loop
  </Toggle>

  <div class="min-w-40 max-w-72 truncate text-muted-foreground">
    {footerLabel}
  </div>

  <Slider
    type="single"
    value={progressValue}
    max={1000}
    step={1}
    disabled={!$playbackSample || effectiveDuration <= 0}
    onValueChange={seekTo}
    class="flex-1"
  />

  <div class="w-24 text-right text-xs text-muted-foreground">
    {formatPlaybackTime(currentTime)} / {formatPlaybackTime(effectiveDuration)}
  </div>

  <Button
    variant="ghost"
    size="icon-sm"
    aria-label={muted ? "Unmute sample" : "Mute sample"}
    onclick={toggleMute}
  >
    {#if muted}
      <VolumeX />
    {:else}
      <Volume2 />
    {/if}
  </Button>

  <Slider
    type="single"
    value={muted ? 0 : Math.round(volume * 100)}
    max={100}
    step={1}
    onValueChange={setVolume}
    class="w-24"
  />
</footer>
