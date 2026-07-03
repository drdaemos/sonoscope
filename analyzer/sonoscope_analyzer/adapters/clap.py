"""LAION CLAP zero-shot tagging adapter (Hugging Face Transformers)."""

from __future__ import annotations

import os
from dataclasses import dataclass
from importlib import import_module
from importlib.resources.abc import Traversable
from pathlib import Path
from typing import Any, Sequence

import numpy as np
from numpy.typing import NDArray
from pydantic import BaseModel, Field, field_validator

from sonoscope_analyzer.audio import read_mono_audio
from sonoscope_analyzer.interfaces import (
    TagPrediction,
    TextAudioSimilarityModel,
    clamp_confidence,
)
from sonoscope_analyzer.torch_utils import (
    configure_torch_inference,
    env_flag,
    is_torch_device_available,
    tensor_to_float_list,
    tensor_to_nested_float_list,
    torch_inference_context,
)

CLAP_SAMPLE_RATE = 48_000
DEFAULT_CLAP_MODEL_ID = "laion/larger_clap_music"
DEFAULT_CLAP_DEVICE = "auto"
DEFAULT_CLAP_BATCH_SIZE = 16
CLAP_DEVICE_PRIORITY = ("cuda", "mps", "cpu")


@dataclass(frozen=True)
class PromptCandidate:
    dimension: str
    value: str
    prompts: tuple[str, ...]
    min_confidence: float = 0.12


class _PromptCandidateModel(BaseModel):
    dimension: str
    value: str
    prompts: tuple[str, ...] = Field(min_length=1)
    min_confidence: float = 0.12

    @field_validator("prompts", mode="after")
    @classmethod
    def strip_and_require_prompts(cls, prompts: tuple[str, ...]) -> tuple[str, ...]:
        normalized = tuple(prompt.strip() for prompt in prompts if prompt.strip())
        if not normalized:
            raise ValueError("must contain at least one non-empty prompt")
        return normalized


class _PromptFileModel(BaseModel):
    candidates: list[_PromptCandidateModel]


def load_clap_prompt_candidates(source: str | Path | Traversable) -> list[PromptCandidate]:
    if isinstance(source, str | Path):
        handle_context = Path(source).open("r", encoding="utf-8")
    else:
        handle_context = source.open("r", encoding="utf-8")

    with handle_context as handle:
        parsed = _PromptFileModel.model_validate_json(handle.read())

    return [
        PromptCandidate(
            dimension=candidate.dimension,
            value=candidate.value,
            prompts=candidate.prompts,
            min_confidence=candidate.min_confidence,
        )
        for candidate in parsed.candidates
    ]


class ClapPromptTagModel:
    """Map CLAP zero-shot text/audio scores to Sonoscope tag predictions."""

    def __init__(
        self,
        similarity_model: TextAudioSimilarityModel,
        candidates: Sequence[PromptCandidate],
        *,
        top_k_per_dimension: int = 2,
        top_k_by_dimension: dict[str, int] | None = None,
    ) -> None:
        self.similarity_model = similarity_model
        self.candidates = list(candidates)
        self.top_k_per_dimension = top_k_per_dimension
        self.top_k_by_dimension = top_k_by_dimension or {}

    def predict(self, path: str) -> Sequence[TagPrediction]:
        return self.predict_batch([path])[0]

    def predict_batch(self, paths: Sequence[str]) -> Sequence[Sequence[TagPrediction]]:
        prompt_entries: list[tuple[PromptCandidate, str]] = []
        for candidate in self.candidates:
            for prompt in candidate.prompts:
                prompt_entries.append((candidate, prompt))
        if not prompt_entries:
            return [[] for _path in paths]

        prompts = [prompt for _candidate, prompt in prompt_entries]
        scores_by_path = [
            list(scores) for scores in self.similarity_model.score_batch(paths, prompts)
        ]
        if len(scores_by_path) != len(paths):
            raise RuntimeError("CLAP scorer returned an unexpected number of score rows")

        return [self._predictions_from_scores(prompt_entries, scores) for scores in scores_by_path]

    def _predictions_from_scores(
        self,
        prompt_entries: Sequence[tuple[PromptCandidate, str]],
        scores: Sequence[float],
    ) -> list[TagPrediction]:
        if len(scores) != len(prompt_entries):
            raise RuntimeError("CLAP scorer returned an unexpected number of scores")
        best_scores: dict[tuple[str, str], tuple[PromptCandidate, float]] = {}
        for (candidate, _prompt), raw_score in zip(prompt_entries, scores, strict=True):
            confidence = clamp_confidence(raw_score)
            key = (candidate.dimension, candidate.value)
            existing = best_scores.get(key)
            if existing is None or confidence > existing[1]:
                best_scores[key] = (candidate, confidence)

        by_dimension: dict[str, list[TagPrediction]] = {}
        for candidate, confidence in best_scores.values():
            if confidence < candidate.min_confidence:
                continue
            by_dimension.setdefault(candidate.dimension, []).append(
                TagPrediction(
                    dimension=candidate.dimension,
                    value=candidate.value,
                    confidence=confidence,
                )
            )

        predictions: list[TagPrediction] = []
        for dimension, dimension_predictions in by_dimension.items():
            top_k = self.top_k_by_dimension.get(dimension, self.top_k_per_dimension)
            predictions.extend(
                sorted(
                    dimension_predictions,
                    key=lambda prediction: prediction.confidence,
                    reverse=True,
                )[:top_k]
            )
        return predictions


class ClapSimilarityModel:
    """Lazy LAION CLAP zero-shot adapter using Hugging Face Transformers."""

    def __init__(
        self,
        *,
        model_id: str = DEFAULT_CLAP_MODEL_ID,
        device: str = DEFAULT_CLAP_DEVICE,
        model: Any | None = None,
        processor: Any | None = None,
    ) -> None:
        self.model_id = model_id
        self.device = resolve_clap_device(device)
        self._model = model
        self._model_device: str | None = None
        self._processor = processor

    def score(self, path: str, prompts: Sequence[str]) -> Sequence[float]:
        return self.score_batch([path], prompts)[0]

    def score_batch(
        self,
        paths: Sequence[str],
        prompts: Sequence[str],
    ) -> Sequence[Sequence[float]]:
        if not prompts:
            return [[] for _path in paths]

        score_rows = [[0.0] * len(prompts) for _path in paths]
        audio_batch: list[NDArray[np.float32]] = []
        audio_indexes: list[int] = []
        for index, path in enumerate(paths):
            try:
                audio, _sample_rate = read_mono_audio(path, target_sample_rate=CLAP_SAMPLE_RATE)
            except (RuntimeError, OSError, ValueError):
                continue
            if audio.size == 0:
                continue
            audio_batch.append(audio)
            audio_indexes.append(index)
        if not audio_batch:
            return score_rows

        try:
            torch = import_module("torch")
        except (ImportError, OSError, RuntimeError) as exc:
            raise RuntimeError("torch is not installed") from exc

        try:
            batch_scores = self._score_audio_batches_with_device(torch, audio_batch, prompts)
        except (AssertionError, RuntimeError):
            if self.device == "cpu":
                raise
            self.device = "cpu"
            batch_scores = self._score_audio_batches_with_device(torch, audio_batch, prompts)

        if len(batch_scores) != len(audio_indexes):
            raise RuntimeError("CLAP model returned an unexpected number of audio scores")
        for index, scores in zip(audio_indexes, batch_scores, strict=True):
            score_rows[index] = scores
        return score_rows

    def _score_audio_batches_with_device(
        self,
        torch: Any,
        audio_batch: Sequence[NDArray[np.float32]],
        prompts: Sequence[str],
    ) -> list[list[float]]:
        max_batch_size = resolve_clap_batch_size(
            os.environ.get("SONOSCOPE_CLAP_BATCH_SIZE"),
            len(audio_batch),
        )
        scores: list[list[float]] = []
        for start_index in range(0, len(audio_batch), max_batch_size):
            scores.extend(
                self._score_batch_with_device(
                    torch,
                    audio_batch[start_index : start_index + max_batch_size],
                    prompts,
                )
            )
        return scores

    def _score_batch_with_device(
        self,
        torch: Any,
        audio_batch: Sequence[NDArray[np.float32]],
        prompts: Sequence[str],
    ) -> list[list[float]]:
        configure_torch_inference(torch, self.device)
        processor = self._get_processor()
        inputs = processor(
            text=list(prompts),
            audios=list(audio_batch) if len(audio_batch) > 1 else audio_batch[0],
            sampling_rate=CLAP_SAMPLE_RATE,
            return_tensors="pt",
            padding=True,
        )
        to_device = getattr(inputs, "to", None)
        if callable(to_device):
            inputs = to_device(self.device)

        with torch_inference_context(torch):
            outputs = self._get_model()(**inputs)
        logits = getattr(outputs, "logits_per_audio", None)
        if logits is None:
            raise RuntimeError("CLAP model output did not include logits_per_audio")
        probabilities = torch.softmax(logits, dim=-1)
        rows = tensor_to_nested_float_list(probabilities)
        if len(audio_batch) == 1 and len(rows) != 1:
            return [tensor_to_float_list(probabilities)]
        return rows

    def _get_model(self) -> Any:
        if self._model is None:
            try:
                transformers = import_module("transformers")
            except ImportError as exc:
                raise RuntimeError("transformers is not installed") from exc
            model_class = getattr(transformers, "ClapModel")
            self._model = model_class.from_pretrained(self.model_id)
            eval_model = getattr(self._model, "eval", None)
            if callable(eval_model):
                eval_model()
        if self._model_device != self.device:
            to_device = getattr(self._model, "to", None)
            if callable(to_device):
                self._model = to_device(self.device)
            self._model_device = self.device
        return self._model

    def _get_processor(self) -> Any:
        if self._processor is None:
            try:
                transformers = import_module("transformers")
            except ImportError as exc:
                raise RuntimeError("transformers is not installed") from exc
            processor_class = getattr(transformers, "ClapProcessor")
            self._processor = processor_class.from_pretrained(self.model_id)
        return self._processor


def resolve_clap_device(requested: str | None = None) -> str:
    """Resolve a CLAP device setting to the best available PyTorch device."""
    normalized = normalize_clap_device(requested or os.environ.get("SONOSCOPE_CLAP_DEVICE"))
    if normalized in {"", "auto", "best"}:
        return best_available_clap_device()
    if is_torch_device_available(normalized):
        return normalized
    return best_available_clap_device()


def normalize_clap_device(device: str | None) -> str:
    if device is None:
        return DEFAULT_CLAP_DEVICE
    return device.strip().lower()


def best_available_clap_device() -> str:
    for device in CLAP_DEVICE_PRIORITY:
        if is_torch_device_available(device):
            return device
    return "cpu"


def resolve_clap_batch_size(requested: str | None, audio_count: int) -> int:
    if audio_count <= 0:
        return 1
    if requested is not None:
        try:
            parsed = int(requested)
        except ValueError:
            parsed = DEFAULT_CLAP_BATCH_SIZE
    else:
        parsed = DEFAULT_CLAP_BATCH_SIZE
    return max(1, min(parsed, audio_count))


def resolve_clap_model_id() -> str | None:
    local_model_path = os.environ.get("SONOSCOPE_CLAP_MODEL_PATH")
    if local_model_path:
        path = Path(local_model_path)
        if clap_model_path_ready(path):
            return str(path)
        if env_flag("SONOSCOPE_CLAP_LOCAL_ONLY"):
            return None

    return os.environ.get("SONOSCOPE_CLAP_MODEL_ID", DEFAULT_CLAP_MODEL_ID)


def clap_model_path_ready(path: Path) -> bool:
    required_files = (
        "config.json",
        "merges.txt",
        "preprocessor_config.json",
        "pytorch_model.bin",
        "special_tokens_map.json",
        "tokenizer.json",
        "tokenizer_config.json",
        "vocab.json",
    )
    return all((path / file_name).is_file() for file_name in required_files)
