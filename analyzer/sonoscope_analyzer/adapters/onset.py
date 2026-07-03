"""Lightweight onset-based fallback for loop/one-shot detection."""

from __future__ import annotations

from sonoscope_analyzer.audio import estimate_onset_times, has_periodic_onsets, read_mono_audio
from sonoscope_analyzer.interfaces import LoopPrediction

ONSET_SAMPLE_RATE = 22_050


class OnsetLoopDetector:
    """Fallback for the Type dimension when the Essentia loop model is unavailable."""

    def __init__(self, *, min_loop_duration_seconds: float = 1.0) -> None:
        self.min_loop_duration_seconds = min_loop_duration_seconds

    def predict(self, path: str) -> LoopPrediction | None:
        try:
            audio, sample_rate = read_mono_audio(path, target_sample_rate=ONSET_SAMPLE_RATE)
        except (RuntimeError, OSError, ValueError):
            return None
        if audio.size == 0:
            return None

        duration = audio.size / sample_rate
        onset_times = estimate_onset_times(audio, sample_rate)
        if duration < 0.35 and len(onset_times) <= 1:
            return LoopPrediction(is_loop=False, confidence=0.8)
        if duration < self.min_loop_duration_seconds:
            return LoopPrediction(is_loop=False, confidence=0.65)
        if has_periodic_onsets(onset_times):
            return LoopPrediction(is_loop=True, confidence=0.68)
        if len(onset_times) <= 2:
            return LoopPrediction(is_loop=False, confidence=0.62)
        return None
