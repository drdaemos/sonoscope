"""Tests for shared audio decoding and the per-batch decode cache."""

from __future__ import annotations

import numpy as np
import pytest
import soundfile

import sonoscope_analyzer.audio as audio
from sonoscope_analyzer.audio import clear_decode_cache, decode_mono, read_mono_audio
from sonoscope_analyzer.waveform import amplitude_bins, generate_waveform


@pytest.fixture(autouse=True)
def fresh_decode_cache():
    clear_decode_cache()
    yield
    clear_decode_cache()


def write_wav(path, samples: np.ndarray, sample_rate: int = 22_050) -> None:
    soundfile.write(path, samples, sample_rate)


def test_decode_mono_reads_file_once_per_batch(tmp_path, monkeypatch: pytest.MonkeyPatch) -> None:
    path = tmp_path / "sample.wav"
    write_wav(path, np.linspace(-0.5, 0.5, 2_205, dtype=np.float32))

    calls = {"count": 0}
    original_read = soundfile.read

    def counting_read(*args, **kwargs):
        calls["count"] += 1
        return original_read(*args, **kwargs)

    monkeypatch.setattr(audio.soundfile, "read", counting_read)

    decode_mono(str(path))
    read_mono_audio(str(path), target_sample_rate=48_000)
    generate_waveform(str(path))

    assert calls["count"] == 1


def test_read_mono_audio_resamples_and_truncates(tmp_path) -> None:
    path = tmp_path / "long.wav"
    write_wav(path, np.ones(44_100, dtype=np.float32), sample_rate=22_050)

    samples, sample_rate = read_mono_audio(str(path), target_sample_rate=11_025, max_seconds=1.0)

    assert sample_rate == 11_025
    assert samples.size == pytest.approx(11_025, abs=2)


def test_amplitude_bins_match_per_bin_peaks() -> None:
    samples = np.concatenate(
        [
            np.full(100, 0.25, dtype=np.float32),
            np.full(100, 1.0, dtype=np.float32),
            np.zeros(100, dtype=np.float32),
            np.full(100, 0.5, dtype=np.float32),
        ]
    )

    bins = amplitude_bins(samples, 4)

    assert bins == [64, 255, 0, 128]


def test_amplitude_bins_handle_more_bins_than_samples() -> None:
    samples = np.asarray([1.0, 0.5], dtype=np.float32)

    bins = amplitude_bins(samples, 8)

    assert len(bins) == 8
    assert max(bins) == 255
    assert all(0 <= value <= 255 for value in bins)
