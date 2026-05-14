"""Stdin/stdout IPC loop for the Sonoscope analysis pipeline."""

import json
import sys

from sonoscope_analyzer.classifier import classify_audio
from sonoscope_analyzer.heuristics import analyze_path
from sonoscope_analyzer.metadata import extract_metadata
from sonoscope_analyzer.protocol import AnalyzeRequest, AnalyzeResponse
from sonoscope_analyzer.waveform import generate_waveform


def process_request(request: AnalyzeRequest) -> AnalyzeResponse:
    file_meta, metadata_tags = extract_metadata(request.path)
    tags = [*metadata_tags, *analyze_path(request.relative_path), *classify_audio(request.path)]
    waveform_data = generate_waveform(request.path)
    return AnalyzeResponse(
        id=request.id,
        status="ok",
        tags=tags,
        file_meta=file_meta,
        waveform_data=waveform_data,
    )


def main() -> None:
    sys.stdout.write(json.dumps({"ready": True}) + "\n")
    sys.stdout.flush()

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        if line == '{"shutdown": true}':
            break
        raw: dict[str, object] = {}
        try:
            raw = json.loads(line)
            request = AnalyzeRequest.model_validate(raw)
            response = process_request(request)
            sys.stdout.write(response.model_dump_json() + "\n")
            sys.stdout.flush()
        except Exception as exc:
            req_id = str(raw.get("id", "unknown"))
            error_response = AnalyzeResponse(id=req_id, status="error", error=str(exc))
            sys.stdout.write(error_response.model_dump_json() + "\n")
            sys.stdout.flush()


if __name__ == "__main__":
    main()
