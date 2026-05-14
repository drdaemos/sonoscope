import { convertFileSrc } from "@tauri-apps/api/core";
import { writable } from "svelte/store";
import { commands, type CommandError, type PlaybackSample } from "$lib/bindings/bindings";

export type PlaybackStatus = "idle" | "loading" | "ready" | "playing" | "paused" | "error";

export type PlayableSample = PlaybackSample & {
  src: string;
};

export const playbackSample = writable<PlayableSample | null>(null);
export const playbackStatus = writable<PlaybackStatus>("idle");
export const playbackError = writable<string | null>(null);
export const playbackAutoplay = writable<boolean>(false);

export async function loadPlaybackSample(
  sampleId: number,
  options: { autoplay?: boolean } = {},
): Promise<void> {
  playbackStatus.set("loading");
  playbackError.set(null);

  const result = await commands.getSamplePlayback(sampleId);
  if (result.status === "error") {
    playbackSample.set(null);
    playbackAutoplay.set(false);
    playbackStatus.set("error");
    playbackError.set(commandErrorMessage(result.error));
    return;
  }

  playbackAutoplay.set(options.autoplay === true);
  playbackSample.set({
    ...result.data,
    src: convertFileSrc(result.data.path),
  });
  playbackStatus.set("ready");
}

export function clearPlayback(): void {
  playbackSample.set(null);
  playbackStatus.set("idle");
  playbackError.set(null);
  playbackAutoplay.set(false);
}

export function commandErrorMessage(error: CommandError): string {
  if (typeof error === "string") return error;
  if ("Database" in error && error.Database) return error.Database;
  if ("Io" in error && error.Io) return error.Io;
  if ("Analysis" in error && error.Analysis) return error.Analysis;
  if ("Other" in error && error.Other) return error.Other;
  if ("DiscoveryCancelled" in error && error.DiscoveryCancelled) {
    return `Discovery cancelled after ${error.DiscoveryCancelled.count} files`;
  }
  return "Unknown playback error";
}

export function formatPlaybackTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "0:00";

  const totalSeconds = Math.floor(seconds);
  const minutes = Math.floor(totalSeconds / 60);
  const remainingSeconds = totalSeconds % 60;
  return `${minutes}:${remainingSeconds.toString().padStart(2, "0")}`;
}
