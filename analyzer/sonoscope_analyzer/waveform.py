"""Compact waveform extraction for playback and overview rendering."""

from __future__ import annotations

import numpy as np
import soundfile
from numpy.typing import NDArray

DEFAULT_WAVEFORM_BINS = 256


def generate_waveform(path: str, bin_count: int = DEFAULT_WAVEFORM_BINS) -> list[int] | None:
    """Read an audio file and return byte-scaled peak amplitude bins."""
    try:
        data, _sample_rate = soundfile.read(path, dtype="float32", always_2d=True)
    except (RuntimeError, OSError, ValueError):
        return None

    mono = np.mean(data, axis=1, dtype=np.float32)
    return amplitude_bins(mono, bin_count)


def amplitude_bins(samples: NDArray[np.float32], bin_count: int) -> list[int]:
    """Convert mono float samples into exactly ``bin_count`` byte amplitudes."""
    if bin_count <= 0:
        raise ValueError("bin_count must be positive")
    if samples.size == 0:
        return [0] * bin_count

    amplitudes = np.clip(np.abs(samples), 0.0, 1.0)
    edges = np.linspace(0, amplitudes.size, bin_count + 1, dtype=np.int64)
    bins: list[int] = []
    for index in range(bin_count):
        start = int(edges[index])
        end = int(edges[index + 1])
        if end <= start:
            bins.append(0)
            continue
        peak = float(np.max(amplitudes[start:end]))
        bins.append(round(peak * 255))
    return bins
