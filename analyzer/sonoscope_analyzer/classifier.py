"""ML classification mapping for analyzer model outputs.

This module deliberately keeps model loading outside the core mapping logic so
unit tests can validate behavior with mocked predictions and no model weights.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from functools import lru_cache
from importlib.resources import files
from typing import Any, Protocol, Sequence

from sonoscope_analyzer.protocol import TagCandidate


@dataclass(frozen=True)
class ModelPrediction:
    label: str
    confidence: float


@dataclass(frozen=True)
class LoopPrediction:
    is_loop: bool
    confidence: float


@dataclass(frozen=True)
class AudioSetMappingRule:
    dimension: str
    value: str
    min_confidence: float


class InstrumentModel(Protocol):
    def predict(self, path: str) -> Sequence[ModelPrediction]: ...


class LoopDetector(Protocol):
    def predict(self, path: str) -> LoopPrediction | None: ...


def classify_audio(
    path: str,
    *,
    instrument_model: InstrumentModel | None = None,
    loop_detector: LoopDetector | None = None,
) -> list[TagCandidate]:
    """Return model-derived tag candidates for an audio file."""
    candidates: dict[tuple[str, str], TagCandidate] = {}

    if loop_detector is not None:
        loop_prediction = loop_detector.predict(path)
        if loop_prediction is not None and loop_prediction.confidence >= 0.55:
            value = "loop" if loop_prediction.is_loop else "one-shot"
            add_candidate(candidates, "Type", value, loop_prediction.confidence)

    if instrument_model is not None:
        mapping = load_audioset_mapping()
        for prediction in instrument_model.predict(path):
            rule = mapping.get(normalize_label(prediction.label))
            if rule is None or prediction.confidence < rule.min_confidence:
                continue
            add_candidate(candidates, rule.dimension, rule.value, prediction.confidence)

    return list(candidates.values())


def add_candidate(
    candidates: dict[tuple[str, str], TagCandidate],
    dimension: str,
    value: str,
    confidence: float,
) -> None:
    key = (dimension, value)
    existing = candidates.get(key)
    if existing is None or confidence > existing.confidence:
        candidates[key] = TagCandidate(
            dimension=dimension,
            value=value,
            source="model",
            confidence=confidence,
        )


def normalize_label(label: str) -> str:
    return " ".join(label.strip().lower().split())


@lru_cache(maxsize=1)
def load_audioset_mapping() -> dict[str, AudioSetMappingRule]:
    mapping_path = files("sonoscope_analyzer").joinpath("mappings/audioset_map.json")
    with mapping_path.open("r", encoding="utf-8") as handle:
        raw = json.load(handle)
    if not isinstance(raw, dict):
        raise ValueError("AudioSet mapping must be an object")

    rules: dict[str, AudioSetMappingRule] = {}
    for label, value in raw.items():
        if not isinstance(label, str) or not isinstance(value, dict):
            raise ValueError("AudioSet mapping entries must map labels to objects")
        rules[normalize_label(label)] = parse_mapping_rule(label, value)
    return rules


def parse_mapping_rule(label: str, raw_rule: dict[str, Any]) -> AudioSetMappingRule:
    dimension = raw_rule.get("dimension")
    value = raw_rule.get("value")
    min_confidence = raw_rule.get("min_confidence", 0.35)
    if not isinstance(dimension, str) or not isinstance(value, str):
        raise ValueError(f"AudioSet mapping rule for {label!r} is missing dimension or value")
    return AudioSetMappingRule(
        dimension=dimension,
        value=value,
        min_confidence=float(min_confidence),
    )
