"""Stdin/stdout IPC loop for the Sonoscope analysis pipeline."""

import json
import sys
from typing import Mapping, cast

from sonoscope_analyzer.classifier import (
    InstrumentModel,
    LoopDetector,
    TagModel,
    classify_audio_batch,
    load_default_model_backends,
)
from sonoscope_analyzer.heuristics import analyze_path
from sonoscope_analyzer.metadata import extract_metadata
from sonoscope_analyzer.protocol import (
    AnalyzeBatchRequest,
    AnalyzeRequest,
    AnalyzeResponse,
    TagCandidate,
)
from sonoscope_analyzer.waveform import generate_waveform


def process_request(
    request: AnalyzeRequest,
    *,
    instrument_model: InstrumentModel | None = None,
    tag_model: TagModel | None = None,
    loop_detector: LoopDetector | None = None,
) -> AnalyzeResponse:
    return process_requests(
        [request],
        instrument_model=instrument_model,
        tag_model=tag_model,
        loop_detector=loop_detector,
    )[0]


def process_requests(
    requests: list[AnalyzeRequest],
    *,
    instrument_model: InstrumentModel | None = None,
    tag_model: TagModel | None = None,
    loop_detector: LoopDetector | None = None,
) -> list[AnalyzeResponse]:
    """Process a batch of requests, always returning one response per request.

    A failure on one file must never swallow responses for the rest of the
    batch: the Rust consumer reads exactly ``len(requests)`` lines.
    """
    if not requests:
        return []

    if instrument_model is None and tag_model is None and loop_detector is None:
        backends = load_default_model_backends()
        tag_model = backends.tag_model
        loop_detector = backends.loop_detector

    try:
        model_tag_batch = classify_audio_batch(
            [request.path for request in requests],
            instrument_model=instrument_model,
            tag_model=tag_model,
            loop_detector=loop_detector,
        )
    except Exception:
        model_tag_batch = [[] for _request in requests]

    responses: list[AnalyzeResponse] = []
    for request, model_tags in zip(requests, model_tag_batch, strict=True):
        try:
            responses.append(process_request_without_models(request, model_tags))
        except Exception as exc:
            responses.append(AnalyzeResponse(id=request.id, status="error", error=str(exc)))
    return responses


def process_request_without_models(
    request: AnalyzeRequest,
    model_tags: list[TagCandidate],
) -> AnalyzeResponse:
    file_meta, metadata_tags = extract_metadata(request.path)
    heuristic_tags = analyze_path(request.relative_path)
    tags = [*metadata_tags, *heuristic_tags, *model_tags]
    waveform_data = generate_waveform(request.path)
    return AnalyzeResponse(
        id=request.id,
        status="ok",
        tags=tags,
        file_meta=file_meta,
        waveform_data=waveform_data,
    )


def write_responses(responses: list[AnalyzeResponse]) -> None:
    for response in responses:
        sys.stdout.write(response.model_dump_json() + "\n")
    sys.stdout.flush()


def batch_error_responses(raw: Mapping[str, object], error: str) -> list[AnalyzeResponse]:
    """Build one error response per raw batch entry so the consumer never stalls.

    The Rust side reads exactly as many lines as it sent requests, so even an
    invalid batch must be answered entry-for-entry.
    """
    raw_requests = raw.get("requests")
    if not isinstance(raw_requests, list) or not raw_requests:
        return [AnalyzeResponse(id="unknown", status="error", error=error)]

    responses: list[AnalyzeResponse] = []
    for raw_request in raw_requests:
        req_id = "unknown"
        if isinstance(raw_request, dict):
            request_fields = cast(Mapping[str, object], raw_request)
            req_id = str(request_fields.get("id", "unknown"))
        responses.append(AnalyzeResponse(id=req_id, status="error", error=error))
    return responses


def main() -> None:
    sys.stdout.write(json.dumps({"ready": True}) + "\n")
    sys.stdout.flush()

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        if line == '{"shutdown": true}':
            break
        raw: object = {}
        try:
            raw = json.loads(line)
            if isinstance(raw, dict) and "requests" in raw:
                try:
                    batch_request = AnalyzeBatchRequest.model_validate(raw)
                except Exception as exc:
                    write_responses(batch_error_responses(raw, str(exc)))
                    continue
                write_responses(process_requests(batch_request.requests))
            elif isinstance(raw, dict):
                request = AnalyzeRequest.model_validate(raw)
                write_responses([process_request(request)])
            else:
                raise ValueError("analyzer request must be an object")
        except Exception as exc:
            req_id = str(raw.get("id", "unknown")) if isinstance(raw, dict) else "unknown"
            write_responses([AnalyzeResponse(id=req_id, status="error", error=str(exc))])


if __name__ == "__main__":
    main()
