"""Unit tests for the IPC protocol models."""

from typing import Literal

import pytest
from pydantic import ValidationError

from sonoscope_analyzer.protocol import (
    AnalyzeRequest,
    AnalyzeResponse,
    FileMeta,
    TagCandidate,
)


def test_request_parses_all_fields() -> None:
    raw = '{"id": "abc-123", "path": "/audio/kick.wav", "relative_path": "Drums/kick.wav"}'
    req = AnalyzeRequest.model_validate_json(raw)
    assert req.id == "abc-123"
    assert req.path == "/audio/kick.wav"
    assert req.relative_path == "Drums/kick.wav"


@pytest.mark.parametrize(
    "raw_json, expected_id, expected_path",
    [
        (
            '{"id":"a1","path":"/lib/bass.wav","relative_path":"bass.wav"}',
            "a1",
            "/lib/bass.wav",
        ),
        (
            '{"id":"b2","path":"/x/y/snare.flac","relative_path":"y/snare.flac"}',
            "b2",
            "/x/y/snare.flac",
        ),
        (
            '{"id":"c3","path":"/samples/pad 001.aiff","relative_path":"pad 001.aiff"}',
            "c3",
            "/samples/pad 001.aiff",
        ),
    ],
)
def test_request_roundtrip(raw_json: str, expected_id: str, expected_path: str) -> None:
    req = AnalyzeRequest.model_validate_json(raw_json)
    assert req.id == expected_id
    assert req.path == expected_path


def test_response_ok_serializes() -> None:
    resp = AnalyzeResponse(
        id="abc-123",
        status="ok",
        tags=[TagCandidate(dimension="Type", value="loop", source="heuristic", confidence=0.95)],
        file_meta=FileMeta(
            format="wav", duration_ms=2048, sample_rate=44100, bit_depth=24, channels=1
        ),
    )
    parsed = AnalyzeResponse.model_validate_json(resp.model_dump_json())
    assert parsed.id == "abc-123"
    assert parsed.status == "ok"
    assert len(parsed.tags) == 1
    assert parsed.tags[0].dimension == "Type"
    assert parsed.tags[0].confidence == 0.95
    assert parsed.file_meta is not None
    assert parsed.file_meta.format == "wav"
    assert parsed.file_meta.sample_rate == 44100


def test_response_error_serializes() -> None:
    resp = AnalyzeResponse(id="xyz", status="error", error="unsupported format")
    parsed = AnalyzeResponse.model_validate_json(resp.model_dump_json())
    assert parsed.id == "xyz"
    assert parsed.status == "error"
    assert parsed.error == "unsupported format"
    assert parsed.tags == []
    assert parsed.file_meta is None


def test_response_defaults_to_ok() -> None:
    resp = AnalyzeResponse(id="q1")
    assert resp.status == "ok"
    assert resp.tags == []


@pytest.mark.parametrize(
    "source",
    ["heuristic", "metadata", "model"],
)
def test_tag_candidate_valid_sources(source: Literal["heuristic", "metadata", "model"]) -> None:
    tag = TagCandidate(dimension="Instrument", value="kick", source=source, confidence=0.9)
    assert tag.source == source


def test_tag_candidate_rejects_unknown_source() -> None:
    with pytest.raises(ValidationError):
        TagCandidate.model_validate(
            {
                "dimension": "Type",
                "value": "loop",
                "source": "unknown",
                "confidence": 0.9,
            }
        )


def test_file_meta_all_nullable() -> None:
    meta = FileMeta()
    assert meta.format is None
    assert meta.duration_ms is None
    assert meta.sample_rate is None
    assert meta.bit_depth is None
    assert meta.channels is None
