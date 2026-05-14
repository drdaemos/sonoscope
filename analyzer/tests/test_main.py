"""Tests for analyzer request processing."""

from __future__ import annotations

from sonoscope_analyzer.main import process_request
from sonoscope_analyzer.protocol import AnalyzeRequest


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
