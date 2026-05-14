import { convertFileSrc } from "@tauri-apps/api/core";
import { get } from "svelte/store";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { commands } from "$lib/bindings/bindings";
import {
  clearPlayback,
  commandErrorMessage,
  formatPlaybackTime,
  loadPlaybackSample,
  playbackAutoplay,
  playbackError,
  playbackSample,
  playbackStatus,
} from "$lib/stores/playback";

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: vi.fn((path: string) => `asset://${path}`),
}));

vi.mock("$lib/bindings/bindings", () => ({
  commands: {
    getSamplePlayback: vi.fn(),
  },
}));

const getSamplePlayback = vi.mocked(commands.getSamplePlayback);
const convert = vi.mocked(convertFileSrc);

beforeEach(() => {
  clearPlayback();
  vi.clearAllMocks();
});

describe("playback store", () => {
  it("loads a playable sample and converts the path into an asset URL", async () => {
    getSamplePlayback.mockResolvedValue({
      status: "ok",
      data: {
        id: 7,
        filename: "kick.wav",
        path: "E:\\Samples\\kick.wav",
        duration_ms: 500,
        waveform_data: [0, 255],
        is_loop: true,
      },
    });

    await loadPlaybackSample(7, { autoplay: true });

    expect(getSamplePlayback).toHaveBeenCalledWith(7);
    expect(convert).toHaveBeenCalledWith("E:\\Samples\\kick.wav");
    expect(get(playbackSample)).toMatchObject({
      id: 7,
      filename: "kick.wav",
      src: "asset://E:\\Samples\\kick.wav",
      is_loop: true,
    });
    expect(get(playbackAutoplay)).toBe(true);
    expect(get(playbackStatus)).toBe("ready");
    expect(get(playbackError)).toBeNull();
  });

  it("stores command errors for the footer", async () => {
    getSamplePlayback.mockResolvedValue({
      status: "error",
      error: { Other: "Unknown sample" },
    });

    await loadPlaybackSample(9);

    expect(get(playbackSample)).toBeNull();
    expect(get(playbackStatus)).toBe("error");
    expect(get(playbackError)).toBe("Unknown sample");
  });

  it("formats playback time", () => {
    expect(formatPlaybackTime(0)).toBe("0:00");
    expect(formatPlaybackTime(65.9)).toBe("1:05");
    expect(formatPlaybackTime(Number.NaN)).toBe("0:00");
  });

  it("formats structured command errors", () => {
    expect(commandErrorMessage("NoLibraryOpen")).toBe("NoLibraryOpen");
    expect(commandErrorMessage({ Io: "missing file" })).toBe("missing file");
  });
});
