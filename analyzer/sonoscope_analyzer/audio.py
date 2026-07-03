"""Shared audio decoding and lightweight DSP helpers.

Decoding is the most expensive I/O step in the pipeline and several stages
(CLAP scoring, loop detection, waveform extraction) need the same file. The
module keeps a small LRU of decoded files sized to the analysis batch so each
file is read from disk once per batch.
"""

from __future__ import annotations

from dataclasses import dataclass
from functools import lru_cache
from typing import Sequence

import numpy as np
import soundfile
from numpy.typing import NDArray

MAX_ANALYSIS_SECONDS = 30.0
# Matches the default analysis batch size in the Rust core; a batch is decoded
# once even though multiple pipeline stages iterate over the same paths.
DECODE_CACHE_SIZE = 16


@dataclass(frozen=True)
class DecodedAudio:
    """Full-length mono audio at the file's native sample rate."""

    samples: NDArray[np.float32]
    sample_rate: int

    def at_rate(
        self,
        target_sample_rate: int,
        *,
        max_seconds: float = MAX_ANALYSIS_SECONDS,
    ) -> NDArray[np.float32]:
        """Return samples truncated to ``max_seconds`` and resampled."""
        samples = self.samples
        max_samples = round(max_seconds * self.sample_rate)
        if max_samples > 0 and samples.size > max_samples:
            samples = samples[:max_samples]
        if self.sample_rate == target_sample_rate:
            return samples.astype(np.float32, copy=False)
        return resample_linear(samples, self.sample_rate, target_sample_rate)


def decode_mono(path: str) -> DecodedAudio:
    """Decode a file to mono float32, cached per path for the current batch.

    Files are assumed not to change while a batch is being analysed.
    """
    return _decode_mono_cached(path)


@lru_cache(maxsize=DECODE_CACHE_SIZE)
def _decode_mono_cached(path: str) -> DecodedAudio:
    data, sample_rate = soundfile.read(path, dtype="float32", always_2d=True)
    mono = np.mean(data, axis=1, dtype=np.float32)
    return DecodedAudio(samples=mono, sample_rate=int(sample_rate))


def clear_decode_cache() -> None:
    _decode_mono_cached.cache_clear()


def read_mono_audio(
    path: str,
    *,
    target_sample_rate: int,
    max_seconds: float = MAX_ANALYSIS_SECONDS,
) -> tuple[NDArray[np.float32], int]:
    decoded = decode_mono(path)
    return decoded.at_rate(target_sample_rate, max_seconds=max_seconds), target_sample_rate


def resample_linear(
    samples: NDArray[np.float32],
    source_sample_rate: int,
    target_sample_rate: int,
) -> NDArray[np.float32]:
    if samples.size == 0 or source_sample_rate == target_sample_rate:
        return samples.astype(np.float32, copy=False)
    target_size = max(1, round(samples.size * target_sample_rate / source_sample_rate))
    source_positions = np.linspace(0, samples.size - 1, num=samples.size, dtype=np.float32)
    target_positions = np.linspace(0, samples.size - 1, num=target_size, dtype=np.float32)
    resampled = np.interp(target_positions, source_positions, samples)
    return resampled.astype(np.float32, copy=False)


def estimate_onset_times(
    samples: NDArray[np.float32],
    sample_rate: int,
    *,
    frame_size: int = 1024,
    hop_size: int = 256,
) -> list[float]:
    if samples.size < frame_size:
        peak = float(np.max(np.abs(samples))) if samples.size else 0.0
        return [0.0] if peak > 0.05 else []

    frame_count = 1 + (samples.size - frame_size) // hop_size
    energies = np.empty(frame_count, dtype=np.float32)
    for index in range(frame_count):
        start = index * hop_size
        frame = samples[start : start + frame_size]
        energies[index] = float(np.sqrt(np.mean(np.square(frame), dtype=np.float32)))

    flux = np.diff(energies, prepend=energies[0])
    flux = np.maximum(flux, 0.0)
    if float(np.max(flux)) <= 0.0:
        return []

    threshold = float(np.mean(flux) + np.std(flux))
    min_gap_frames = max(1, round(0.08 * sample_rate / hop_size))
    onsets: list[float] = []
    last_index = -min_gap_frames
    for index, value in enumerate(flux):
        if value < threshold or index - last_index < min_gap_frames:
            continue
        onsets.append(index * hop_size / sample_rate)
        last_index = index
    return onsets


def has_periodic_onsets(onset_times: Sequence[float]) -> bool:
    if len(onset_times) < 3:
        return False
    intervals = np.diff(np.asarray(onset_times, dtype=np.float32))
    intervals = intervals[intervals > 0.05]
    if intervals.size < 2:
        return False
    mean_interval = float(np.mean(intervals))
    if mean_interval <= 0.0:
        return False
    coefficient_of_variation = float(np.std(intervals) / mean_interval)
    return coefficient_of_variation <= 0.25
