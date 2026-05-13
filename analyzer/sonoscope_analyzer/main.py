"""Stdin/stdout IPC loop for the Sonoscope analysis pipeline."""

import json
import sys

from sonoscope_analyzer.protocol import AnalyzeRequest, AnalyzeResponse


def process_request(request: AnalyzeRequest) -> AnalyzeResponse:
    return AnalyzeResponse(id=request.id, status="ok")


def main() -> None:
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
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
