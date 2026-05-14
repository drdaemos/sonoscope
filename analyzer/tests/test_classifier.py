"""Tests for mocked ML output mapping."""

from __future__ import annotations

from collections.abc import Sequence

from sonoscope_analyzer.classifier import LoopPrediction, ModelPrediction, classify_audio


class FakeInstrumentModel:
    def __init__(self, predictions: Sequence[ModelPrediction]) -> None:
        self.predictions = predictions

    def predict(self, path: str) -> Sequence[ModelPrediction]:
        assert path
        return self.predictions


class FakeLoopDetector:
    def __init__(self, prediction: LoopPrediction | None) -> None:
        self.prediction = prediction

    def predict(self, path: str) -> LoopPrediction | None:
        assert path
        return self.prediction


def test_classify_audio_maps_audioset_label_to_instrument() -> None:
    tags = classify_audio(
        "sample.wav",
        instrument_model=FakeInstrumentModel([ModelPrediction("Bass drum", 0.82)]),
    )

    assert [(tag.dimension, tag.value, tag.source, tag.confidence) for tag in tags] == [
        ("Instrument", "kick", "model", 0.82)
    ]


def test_classify_audio_ignores_predictions_below_mapping_threshold() -> None:
    tags = classify_audio(
        "sample.wav",
        instrument_model=FakeInstrumentModel([ModelPrediction("Speech", 0.4)]),
    )

    assert tags == []


def test_classify_audio_deduplicates_mapped_values_by_highest_confidence() -> None:
    tags = classify_audio(
        "sample.wav",
        instrument_model=FakeInstrumentModel(
            [
                ModelPrediction("Electric guitar", 0.61),
                ModelPrediction("Acoustic guitar", 0.74),
            ]
        ),
    )

    assert len(tags) == 1
    assert tags[0].dimension == "Instrument"
    assert tags[0].value == "guitar"
    assert tags[0].confidence == 0.74


def test_classify_audio_maps_loop_detector_output_to_type() -> None:
    tags = classify_audio(
        "loop.wav",
        loop_detector=FakeLoopDetector(LoopPrediction(is_loop=True, confidence=0.91)),
    )

    assert [(tag.dimension, tag.value, tag.source) for tag in tags] == [("Type", "loop", "model")]


def test_classify_audio_maps_non_loop_output_to_one_shot() -> None:
    tags = classify_audio(
        "hit.wav",
        loop_detector=FakeLoopDetector(LoopPrediction(is_loop=False, confidence=0.88)),
    )

    assert [(tag.dimension, tag.value, tag.source) for tag in tags] == [
        ("Type", "one-shot", "model")
    ]


def test_classify_audio_has_no_model_dependency_by_default() -> None:
    assert classify_audio("sample.wav") == []
