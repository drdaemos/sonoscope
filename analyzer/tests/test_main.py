"""Tests for analyzer request processing."""

from __future__ import annotations

from collections.abc import Sequence

import pytest

import sonoscope_analyzer.main as analyzer_main
from sonoscope_analyzer.classifier import ModelPrediction, TagPrediction
from sonoscope_analyzer.main import batch_error_responses, process_request, process_requests
from sonoscope_analyzer.protocol import AnalyzeRequest


class FakeInstrumentModel:
    def predict(self, path: str) -> Sequence[ModelPrediction]:
        assert path
        return [ModelPrediction("Instrument:kick", 0.82)]


class FakeTagModel:
    def __init__(self) -> None:
        self.batch_calls: list[list[str]] = []

    def predict_batch(self, paths: Sequence[str]) -> Sequence[Sequence[TagPrediction]]:
        self.batch_calls.append(list(paths))
        return [
            [TagPrediction(dimension="Instrument", value="kick", confidence=0.82)],
            [TagPrediction(dimension="Instrument", value="snare", confidence=0.78)],
        ]


class FakeTypeTagModel:
    def predict_batch(self, paths: Sequence[str]) -> Sequence[Sequence[TagPrediction]]:
        return [[TagPrediction(dimension="Type", value="loop", confidence=0.74)] for _path in paths]


def test_process_request_combines_metadata_and_heuristics(tmp_path) -> None:
    path = tmp_path / "kick_loop_120bpm.wav"
    path.write_bytes(b"not real audio")

    response = process_request(
        AnalyzeRequest(
            id="req-1",
            path=str(path),
            relative_path="Drums/Kicks/kick_loop_120bpm.wav",
        )
    )

    assert response.id == "req-1"
    assert response.status == "ok"
    assert response.file_meta is not None
    assert response.file_meta.format == "wav"
    assert response.waveform_data is None
    assert {("Instrument", "kick"), ("Type", "loop"), ("Tempo", "120")} <= {
        (tag.dimension, tag.value) for tag in response.tags
    }
    assert ("Type", "one-shot") not in {(tag.dimension, tag.value) for tag in response.tags}


def test_process_request_includes_model_tags_from_injected_backend(tmp_path) -> None:
    path = tmp_path / "untagged.wav"
    path.write_bytes(b"not real audio")

    response = process_request(
        AnalyzeRequest(
            id="req-1",
            path=str(path),
            relative_path="untagged.wav",
        ),
        instrument_model=FakeInstrumentModel(),
    )

    assert ("Instrument", "kick", "model") in {
        (tag.dimension, tag.value, tag.source) for tag in response.tags
    }
    assert not any(tag.dimension == "Type" for tag in response.tags)


def test_process_request_does_not_default_one_shot_when_model_finds_type(tmp_path) -> None:
    path = tmp_path / "untagged.wav"
    path.write_bytes(b"not real audio")

    response = process_request(
        AnalyzeRequest(
            id="req-1",
            path=str(path),
            relative_path="untagged.wav",
        ),
        tag_model=FakeTypeTagModel(),
    )

    assert ("Type", "loop", "model") in {
        (tag.dimension, tag.value, tag.source) for tag in response.tags
    }
    assert ("Type", "one-shot") not in {(tag.dimension, tag.value) for tag in response.tags}


def test_process_requests_batches_model_tags(tmp_path) -> None:
    kick_path = tmp_path / "untagged_kick.wav"
    snare_path = tmp_path / "untagged_snare.wav"
    kick_path.write_bytes(b"not real audio")
    snare_path.write_bytes(b"not real audio")
    tag_model = FakeTagModel()

    responses = process_requests(
        [
            AnalyzeRequest(id="req-1", path=str(kick_path), relative_path="untagged_kick.wav"),
            AnalyzeRequest(id="req-2", path=str(snare_path), relative_path="untagged_snare.wav"),
        ],
        tag_model=tag_model,
    )

    assert [response.id for response in responses] == ["req-1", "req-2"]
    assert tag_model.batch_calls == [[str(kick_path), str(snare_path)]]
    assert [
        next(tag.value for tag in response.tags if tag.source == "model") for response in responses
    ] == ["kick", "snare"]


def test_process_requests_isolates_per_request_failures(
    tmp_path, monkeypatch: pytest.MonkeyPatch
) -> None:
    good_path = tmp_path / "good.wav"
    bad_path = tmp_path / "bad.wav"
    good_path.write_bytes(b"not real audio")
    bad_path.write_bytes(b"not real audio")

    original = analyzer_main.extract_metadata

    def failing_extract(path: str):
        if path == str(bad_path):
            raise RuntimeError("corrupt file")
        return original(path)

    monkeypatch.setattr(analyzer_main, "extract_metadata", failing_extract)

    responses = process_requests(
        [
            AnalyzeRequest(id="req-bad", path=str(bad_path), relative_path="bad.wav"),
            AnalyzeRequest(id="req-good", path=str(good_path), relative_path="good.wav"),
        ]
    )

    assert [response.id for response in responses] == ["req-bad", "req-good"]
    assert responses[0].status == "error"
    assert responses[0].error == "corrupt file"
    assert responses[1].status == "ok"


def test_batch_error_responses_returns_one_response_per_entry() -> None:
    raw = {
        "requests": [
            {"id": "req-1", "path": "a.wav"},
            {"id": "req-2"},
            "not-an-object",
        ]
    }

    responses = batch_error_responses(raw, "validation failed")

    assert [response.id for response in responses] == ["req-1", "req-2", "unknown"]
    assert all(response.status == "error" for response in responses)
    assert all(response.error == "validation failed" for response in responses)


def test_batch_error_responses_handles_missing_requests_list() -> None:
    responses = batch_error_responses({"requests": "nope"}, "boom")

    assert len(responses) == 1
    assert responses[0].id == "unknown"
    assert responses[0].status == "error"
