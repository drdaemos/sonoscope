"""Essentia TensorFlow model adapters.

Model files are supplied by the app distribution or local environment; no
network lookup happens here.
"""

from __future__ import annotations

from importlib import import_module
from pathlib import Path
from typing import Any, Sequence

import numpy as np
from numpy.typing import NDArray

from sonoscope_analyzer.interfaces import (
    LoopPrediction,
    TagPrediction,
    clamp_confidence,
    normalize_label,
)

ESSENTIA_LOOP_ROLE_MODEL_FILE = "fs_loop_ds-msd-musicnn-1.pb"
ESSENTIA_MUSICNN_MODEL_FILE = "msd-musicnn-1.pb"
ESSENTIA_LOOP_ROLE_LABELS = ("bass", "chords", "fx", "melody", "percussion")
ESSENTIA_LOOP_ROLE_INSTRUMENTS = {
    "bass": "bass",
    "chords": "chord",
    "fx": "fx",
    "melody": "lead",
    "percussion": "percussion",
}


class EssentiaLoopDetector:
    """Essentia TensorFlow loop/one-shot adapter."""

    def __init__(
        self,
        model_path: str,
        *,
        output_node: str = "model/Softmax",
        sample_rate: int = 16_000,
        labels: Sequence[str] = ("one-shot", "loop"),
    ) -> None:
        self.model_path = model_path
        self.output_node = output_node
        self.sample_rate = sample_rate
        self.labels = [normalize_label(label) for label in labels]
        self._loader_factory: Any | None = None
        self._predictor: Any | None = None

    def predict(self, path: str) -> LoopPrediction | None:
        loader_factory, predictor = self._get_essentia()
        audio = loader_factory(filename=path, sampleRate=self.sample_rate)()
        probabilities = probabilities_from_output(predictor(audio))
        if probabilities.size < 2:
            return None

        try:
            loop_index = self.labels.index("loop")
            one_shot_index = self.labels.index("one-shot")
        except ValueError:
            return None
        if max(loop_index, one_shot_index) >= probabilities.size:
            return None

        loop_confidence = float(probabilities[loop_index])
        one_shot_confidence = float(probabilities[one_shot_index])
        if loop_confidence >= one_shot_confidence:
            return LoopPrediction(is_loop=True, confidence=loop_confidence)
        return LoopPrediction(is_loop=False, confidence=one_shot_confidence)

    def _get_essentia(self) -> tuple[Any, Any]:
        if self._loader_factory is None or self._predictor is None:
            try:
                essentia_standard = import_module("essentia.standard")
            except ImportError as exc:
                raise RuntimeError("essentia-tensorflow is not installed") from exc
            self._loader_factory = getattr(essentia_standard, "MonoLoader")
            predictor_factory = getattr(essentia_standard, "TensorflowPredictMusiCNN")
            self._predictor = predictor_factory(
                graphFilename=self.model_path,
                output=self.output_node,
            )
        return self._loader_factory, self._predictor


class EssentiaLoopRoleTagModel:
    """Essentia Freesound Loop Dataset role classifier.

    This model classifies loop instrument role. It is intentionally not used as
    a loop-vs-one-shot detector because the upstream model only sees loop-role
    classes.
    """

    def __init__(
        self,
        model_dir: str | Path,
        *,
        min_confidence: float = 0.35,
        sample_rate: int = 16_000,
    ) -> None:
        self.model_dir = Path(model_dir)
        self.min_confidence = min_confidence
        self.sample_rate = sample_rate
        self.embedding_model_path = self.model_dir.joinpath(ESSENTIA_MUSICNN_MODEL_FILE)
        self.classifier_model_path = self.model_dir.joinpath(ESSENTIA_LOOP_ROLE_MODEL_FILE)
        self._loader_factory: Any | None = None
        self._embedding_model: Any | None = None
        self._classifier_model: Any | None = None

    def predict_batch(self, paths: Sequence[str]) -> Sequence[Sequence[TagPrediction]]:
        return [self.predict(path) for path in paths]

    def predict(self, path: str) -> list[TagPrediction]:
        loader_factory, embedding_model, classifier_model = self._get_essentia()
        audio = loader_factory(
            filename=path,
            sampleRate=self.sample_rate,
            resampleQuality=4,
        )()
        embeddings = embedding_model(audio)
        probabilities = probabilities_from_output(classifier_model(embeddings))
        return loop_role_predictions(probabilities, self.min_confidence)

    def _get_essentia(self) -> tuple[Any, Any, Any]:
        if (
            self._loader_factory is None
            or self._embedding_model is None
            or self._classifier_model is None
        ):
            try:
                essentia_standard = import_module("essentia.standard")
            except ImportError as exc:
                raise RuntimeError("essentia-tensorflow is not installed") from exc
            self._loader_factory = getattr(essentia_standard, "MonoLoader")
            embedding_factory = getattr(essentia_standard, "TensorflowPredictMusiCNN")
            classifier_factory = getattr(essentia_standard, "TensorflowPredict2D")
            self._embedding_model = embedding_factory(
                graphFilename=str(self.embedding_model_path),
                output="model/dense/BiasAdd",
            )
            self._classifier_model = classifier_factory(
                graphFilename=str(self.classifier_model_path),
                input="serving_default_model_Placeholder",
                output="PartitionedCall",
            )
        return self._loader_factory, self._embedding_model, self._classifier_model


def essentia_loop_role_model_ready(model_dir: Path) -> bool:
    return (
        model_dir.joinpath(ESSENTIA_MUSICNN_MODEL_FILE).is_file()
        and model_dir.joinpath(ESSENTIA_LOOP_ROLE_MODEL_FILE).is_file()
    )


def probabilities_from_output(output: object) -> NDArray[np.float32]:
    probabilities = np.asarray(output, dtype=np.float32)
    if probabilities.size == 0:
        return np.asarray([], dtype=np.float32)
    if probabilities.ndim == 1:
        return probabilities
    if probabilities.ndim == 2:
        return np.mean(probabilities, axis=0, dtype=np.float32)
    return probabilities.reshape(-1).astype(np.float32, copy=False)


def loop_role_predictions(
    probabilities: Sequence[float] | NDArray[np.float32],
    min_confidence: float,
) -> list[TagPrediction]:
    predictions: list[TagPrediction] = []
    for label, probability in zip(ESSENTIA_LOOP_ROLE_LABELS, probabilities, strict=False):
        confidence = clamp_confidence(probability)
        if confidence < min_confidence:
            continue
        value = ESSENTIA_LOOP_ROLE_INSTRUMENTS[label]
        predictions.append(
            TagPrediction(
                dimension="Instrument",
                value=value,
                confidence=confidence,
            )
        )
    return sorted(predictions, key=lambda prediction: prediction.confidence, reverse=True)[:2]
