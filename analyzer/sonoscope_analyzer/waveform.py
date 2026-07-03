"""Compact waveform extraction for playback and overview rendering."""

from __future__ import annotations

import numpy as np
from numpy.typing import NDArray

from sonoscope_analyzer.audio import decode_mono

DEFAULT_WAVEFORM_BINS = 256


def generate_waveform(path: str, bin_count: int = DEFAULT_WAVEFORM_BINS) -> list[int] | None:
    """Read an audio file and return byte-scaled peak amplitude bins."""
    try:
        decoded = decode_mono(path)
    except (RuntimeError, OSError, ValueError):
        return None

    return amplitude_bins(decoded.samples, bin_count)


def amplitude_bins(samples: NDArray[np.float32], bin_count: int) -> list[int]:
    """Convert mono float samples into exactly ``bin_count`` byte amplitudes."""
    if bin_count <= 0:
        raise ValueError("bin_count must be positive")
    if samples.size == 0:
        return [0] * bin_count

    amplitudes = np.clip(np.abs(samples), 0.0, 1.0)
    edges = np.linspace(0, amplitudes.size, bin_count + 1, dtype=np.int64)
    starts = edges[:-1]
    # reduceat needs strictly valid slice starts; empty bins (start == end)
    # would otherwise pick up the next sample, so zero them explicitly.
    peaks = np.maximum.reduceat(amplitudes, np.minimum(starts, amplitudes.size - 1))
    empty = starts >= edges[1:]
    peaks[empty] = 0.0
    return [int(round(float(peak) * 255)) for peak in peaks]
