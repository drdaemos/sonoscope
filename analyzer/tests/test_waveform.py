"""Tests for compact waveform generation."""

from __future__ import annotations

import numpy as np
import soundfile as sf

from sonoscope_analyzer.waveform import amplitude_bins, generate_waveform


def test_amplitude_bins_returns_fixed_byte_scaled_peaks() -> None:
    samples = np.array([0.0, 0.25, -0.5, 1.0, -2.0, 0.0], dtype=np.float32)

    assert amplitude_bins(samples, 3) == [64, 255, 255]


def test_amplitude_bins_returns_silence_for_empty_input() -> None:
    assert amplitude_bins(np.array([], dtype=np.float32), 4) == [0, 0, 0, 0]


def test_generate_waveform_reads_audio_file(tmp_path) -> None:
    path = tmp_path / "tone.wav"
    sf.write(path, np.array([0.0, 0.5, -0.5, 1.0], dtype=np.float32), 44100)

    waveform = generate_waveform(str(path), bin_count=4)

    assert waveform == [0, 128, 128, 255]


def test_generate_waveform_returns_none_for_unreadable_audio(tmp_path) -> None:
    path = tmp_path / "broken.wav"
    path.write_bytes(b"not real audio")

    assert generate_waveform(str(path)) is None
