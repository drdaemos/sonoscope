"""Model interfaces and prediction types shared by all analyzer backends."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol, Sequence


@dataclass(frozen=True)
class ModelPrediction:
    label: str
    confidence: float


@dataclass(frozen=True)
class LoopPrediction:
    is_loop: bool
    confidence: float


@dataclass(frozen=True)
class TagPrediction:
    dimension: str
    value: str
    confidence: float


class InstrumentModel(Protocol):
    def predict(self, path: str) -> Sequence[ModelPrediction]: ...


class TagModel(Protocol):
    def predict_batch(self, paths: Sequence[str]) -> Sequence[Sequence[TagPrediction]]: ...


class LoopDetector(Protocol):
    def predict(self, path: str) -> LoopPrediction | None: ...


class TextAudioSimilarityModel(Protocol):
    def score_batch(
        self,
        paths: Sequence[str],
        prompts: Sequence[str],
    ) -> Sequence[Sequence[float]]: ...


def clamp_confidence(value: float) -> float:
    return max(0.0, min(1.0, float(value)))


def normalize_label(label: str) -> str:
    return " ".join(label.strip().lower().split())
