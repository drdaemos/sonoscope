"""ML classification mapping for analyzer model outputs.

This module orchestrates the model backends (see ``adapters/``) and maps
their predictions to Sonoscope tag candidates. Model loading stays outside
the core mapping logic so unit tests can validate behavior with mocked
predictions and no model weights.
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from functools import lru_cache
from importlib.resources import files
from importlib.resources.abc import Traversable
from pathlib import Path
from typing import Sequence

from sonoscope_analyzer.adapters.clap import (
    CLAP_DEVICE_PRIORITY,
    CLAP_SAMPLE_RATE,
    DEFAULT_CLAP_BATCH_SIZE,
    DEFAULT_CLAP_DEVICE,
    DEFAULT_CLAP_MODEL_ID,
    ClapPromptTagModel,
    ClapSimilarityModel,
    PromptCandidate,
    best_available_clap_device,
    clap_model_path_ready,
    load_clap_prompt_candidates,
    normalize_clap_device,
    resolve_clap_batch_size,
    resolve_clap_device,
    resolve_clap_model_id,
)
from sonoscope_analyzer.adapters.essentia import (
    ESSENTIA_LOOP_ROLE_INSTRUMENTS,
    ESSENTIA_LOOP_ROLE_LABELS,
    ESSENTIA_LOOP_ROLE_MODEL_FILE,
    ESSENTIA_MUSICNN_MODEL_FILE,
    EssentiaLoopDetector,
    EssentiaLoopRoleTagModel,
    essentia_loop_role_model_ready,
    loop_role_predictions,
    probabilities_from_output,
)
from sonoscope_analyzer.adapters.onset import ONSET_SAMPLE_RATE, OnsetLoopDetector
from sonoscope_analyzer.audio import (
    MAX_ANALYSIS_SECONDS,
    estimate_onset_times,
    has_periodic_onsets,
    read_mono_audio,
    resample_linear,
)
from sonoscope_analyzer.interfaces import (
    InstrumentModel,
    LoopDetector,
    LoopPrediction,
    ModelPrediction,
    TagModel,
    TagPrediction,
    TextAudioSimilarityModel,
    clamp_confidence,
    normalize_label,
)
from sonoscope_analyzer.protocol import TagCandidate
from sonoscope_analyzer.torch_utils import (
    configure_torch_inference,
    env_flag,
    is_torch_device_available,
    tensor_to_float_list,
    tensor_to_nested_float_list,
    torch_inference_context,
)

__all__ = [
    "CLAP_DEVICE_PRIORITY",
    "CLAP_SAMPLE_RATE",
    "DEFAULT_CLAP_BATCH_SIZE",
    "DEFAULT_CLAP_DEVICE",
    "DEFAULT_CLAP_MODEL_ID",
    "ESSENTIA_LOOP_ROLE_INSTRUMENTS",
    "ESSENTIA_LOOP_ROLE_LABELS",
    "ESSENTIA_LOOP_ROLE_MODEL_FILE",
    "ESSENTIA_MUSICNN_MODEL_FILE",
    "MAX_ANALYSIS_SECONDS",
    "ONSET_SAMPLE_RATE",
    "ClapPromptTagModel",
    "ClapSimilarityModel",
    "CompositeTagModel",
    "EssentiaLoopDetector",
    "EssentiaLoopRoleTagModel",
    "InstrumentModel",
    "LoopDetector",
    "LoopPrediction",
    "ModelBackends",
    "ModelPrediction",
    "OnsetLoopDetector",
    "PromptCandidate",
    "TagModel",
    "TagPrediction",
    "TextAudioSimilarityModel",
    "add_candidate",
    "best_available_clap_device",
    "clamp_confidence",
    "clap_model_path_ready",
    "classify_audio",
    "classify_audio_batch",
    "configure_torch_inference",
    "env_flag",
    "essentia_loop_role_model_ready",
    "estimate_onset_times",
    "has_periodic_onsets",
    "is_torch_device_available",
    "load_clap_prompt_candidates",
    "load_default_essentia_loop_role_model",
    "load_default_model_backends",
    "load_default_tag_model",
    "loop_role_predictions",
    "normalize_clap_device",
    "normalize_label",
    "parse_labels_env",
    "parse_model_label",
    "probabilities_from_output",
    "read_mono_audio",
    "resample_linear",
    "resolve_clap_batch_size",
    "resolve_clap_device",
    "resolve_clap_model_id",
    "tensor_to_float_list",
    "tensor_to_nested_float_list",
    "torch_inference_context",
]


@dataclass(frozen=True)
class ModelBackends:
    tag_model: TagModel | None = None
    loop_detector: LoopDetector | None = None


class CompositeTagModel:
    def __init__(self, models: Sequence[TagModel]) -> None:
        self.models = list(models)

    def predict_batch(self, paths: Sequence[str]) -> Sequence[Sequence[TagPrediction]]:
        combined: list[list[TagPrediction]] = [[] for _path in paths]
        for model in self.models:
            try:
                predictions = model.predict_batch(paths)
            except (RuntimeError, OSError, ValueError):
                continue
            for row, row_predictions in zip(combined, predictions, strict=False):
                row.extend(row_predictions)
        return combined


def classify_audio(
    path: str,
    *,
    instrument_model: InstrumentModel | None = None,
    tag_model: TagModel | None = None,
    loop_detector: LoopDetector | None = None,
) -> list[TagCandidate]:
    """Return model-derived tag candidates for an audio file."""
    return classify_audio_batch(
        [path],
        instrument_model=instrument_model,
        tag_model=tag_model,
        loop_detector=loop_detector,
    )[0]


def classify_audio_batch(
    paths: Sequence[str],
    *,
    instrument_model: InstrumentModel | None = None,
    tag_model: TagModel | None = None,
    loop_detector: LoopDetector | None = None,
) -> list[list[TagCandidate]]:
    """Return model-derived tag candidates for multiple files.

    CLAP-backed tag models can score the full path batch in one model invocation.
    Other model backends keep their existing per-file behavior.
    """
    candidate_maps: list[dict[tuple[str, str], TagCandidate]] = [dict() for _path in paths]

    for index, path in enumerate(paths):
        if loop_detector is not None:
            try:
                loop_prediction = loop_detector.predict(path)
            except (RuntimeError, OSError, ValueError):
                loop_prediction = None
            if loop_prediction is not None and loop_prediction.confidence >= 0.55:
                value = "loop" if loop_prediction.is_loop else "one-shot"
                add_candidate(candidate_maps[index], "Type", value, loop_prediction.confidence)

    if tag_model is not None and paths:
        try:
            batch_tag_predictions = tag_model.predict_batch(paths)
        except (RuntimeError, OSError, ValueError):
            batch_tag_predictions = [[] for _path in paths]

        for candidate_map, tag_predictions in zip(
            candidate_maps,
            batch_tag_predictions,
            strict=False,
        ):
            for prediction in tag_predictions:
                add_candidate(
                    candidate_map,
                    prediction.dimension,
                    prediction.value,
                    prediction.confidence,
                )

    for index, path in enumerate(paths):
        if instrument_model is not None:
            try:
                predictions = instrument_model.predict(path)
            except (RuntimeError, OSError, ValueError):
                predictions = []
            for prediction in predictions:
                dimension, value = parse_model_label(prediction.label)
                add_candidate(candidate_maps[index], dimension, value, prediction.confidence)

    return [list(candidates.values()) for candidates in candidate_maps]


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


def parse_model_label(label: str) -> tuple[str, str]:
    if ":" in label:
        dimension, value = label.split(":", maxsplit=1)
        return dimension.strip(), value.strip()
    return "Instrument", normalize_label(label).replace(" ", "-")


@lru_cache(maxsize=1)
def load_default_model_backends() -> ModelBackends:
    if env_flag("SONOSCOPE_DISABLE_ML"):
        return ModelBackends()

    loop_detector: LoopDetector | None = None
    loop_model_path = os.environ.get("SONOSCOPE_ESSENTIA_LOOP_MODEL")
    if loop_model_path:
        loop_detector = EssentiaLoopDetector(
            loop_model_path,
            output_node=os.environ.get("SONOSCOPE_ESSENTIA_LOOP_OUTPUT", "model/Softmax"),
            labels=parse_labels_env("SONOSCOPE_ESSENTIA_LOOP_LABELS", ("one-shot", "loop")),
        )

    tag_models: list[TagModel] = []
    if not env_flag("SONOSCOPE_DISABLE_CLAP"):
        tag_model = load_default_tag_model()
        if tag_model is not None:
            tag_models.append(tag_model)
    if not env_flag("SONOSCOPE_DISABLE_ESSENTIA"):
        essentia_tag_model = load_default_essentia_loop_role_model()
        if essentia_tag_model is not None:
            tag_models.append(essentia_tag_model)

    tag_model = None
    if len(tag_models) == 1:
        tag_model = tag_models[0]
    elif tag_models:
        tag_model = CompositeTagModel(tag_models)

    return ModelBackends(tag_model=tag_model, loop_detector=loop_detector)


def parse_labels_env(name: str, default: Sequence[str]) -> list[str]:
    value = os.environ.get(name)
    if not value:
        return list(default)
    labels = [label.strip() for label in value.split(",") if label.strip()]
    return labels or list(default)


@lru_cache(maxsize=1)
def load_default_tag_model() -> TagModel | None:
    prompt_path = os.environ.get("SONOSCOPE_CLAP_PROMPTS")
    prompt_source: str | Path | Traversable
    if prompt_path:
        if not Path(prompt_path).exists():
            return None
        prompt_source = Path(prompt_path)
    else:
        prompt_source = files("sonoscope_analyzer").joinpath("mappings/clap_prompts.json")

    candidates = load_clap_prompt_candidates(prompt_source)
    if not candidates:
        return None

    model_id = resolve_clap_model_id()
    if model_id is None:
        return None

    similarity_model = ClapSimilarityModel(
        model_id=model_id,
        device=os.environ.get("SONOSCOPE_CLAP_DEVICE", DEFAULT_CLAP_DEVICE),
    )
    return ClapPromptTagModel(
        similarity_model,
        candidates,
        top_k_per_dimension=int(os.environ.get("SONOSCOPE_CLAP_TOP_K_PER_DIMENSION", "2")),
        top_k_by_dimension={"Mode": 1},
    )


def load_default_essentia_loop_role_model() -> TagModel | None:
    model_dir = os.environ.get("SONOSCOPE_ESSENTIA_MODEL_DIR")
    if not model_dir:
        return None
    model_path = Path(model_dir)
    if not essentia_loop_role_model_ready(model_path):
        return None
    return EssentiaLoopRoleTagModel(
        model_path,
        min_confidence=float(os.environ.get("SONOSCOPE_ESSENTIA_LOOP_ROLE_MIN_CONFIDENCE", "0.35")),
    )
