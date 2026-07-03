"""IPC protocol models for the Sonoscope analysis pipeline."""

from typing import Literal

from pydantic import BaseModel, Field


class FileMeta(BaseModel):
    format: str | None = None
    duration_ms: int | None = None
    sample_rate: int | None = None
    bit_depth: int | None = None
    channels: int | None = None


class TagCandidate(BaseModel):
    dimension: str
    value: str
    source: Literal["heuristic", "metadata", "model"]
    confidence: float


class AnalyzeRequest(BaseModel):
    id: str
    path: str
    relative_path: str


class AnalyzeBatchRequest(BaseModel):
    requests: list[AnalyzeRequest] = Field(min_length=1)


class AnalyzeResponse(BaseModel):
    id: str
    status: Literal["ok", "error"] = "ok"
    tags: list[TagCandidate] = Field(default_factory=list)
    file_meta: FileMeta | None = None
    waveform_data: list[int] | None = None
    error: str | None = None


class AnalyzeBatchResponse(BaseModel):
    responses: list[AnalyzeResponse] = Field(default_factory=list)
