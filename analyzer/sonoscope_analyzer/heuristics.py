"""Filename heuristic analysis."""

from __future__ import annotations

import re
from dataclasses import dataclass
from functools import lru_cache
from importlib.resources import files
from pathlib import PurePath

from pydantic import BaseModel

from sonoscope_analyzer.protocol import TagCandidate


@dataclass(frozen=True)
class TokenRule:
    value: str
    tokens: tuple[str, ...]
    confidence: float


class _TokenRuleModel(BaseModel):
    value: str
    tokens: tuple[str, ...]
    confidence: float


class _TokenRuleFileModel(BaseModel):
    type: list[_TokenRuleModel] = []
    instrument: list[_TokenRuleModel] = []


TOKEN_SPLIT_RE = re.compile(r"[^a-z0-9#]+")
TEMPO_RE = re.compile(r"(?<!\d)([5-9]\d|1\d{2}|2[0-4]\d|250)\s*_?\s*bpm(?![a-z])", re.I)
# Detects tempo embedded as _NNN_ or _NNN. without an explicit "bpm" suffix (Splice convention).
# Requires an underscore before to avoid matching version numbers like "Studio101_".
INLINE_TEMPO_RE = re.compile(r"(?<=_)([5-9]\d|1\d{2}|2[0-4]\d|250)(?=[_.\s]|$)")
KEY_RE = re.compile(
    r"(?<![a-z])([a-g])([#b]?)(?:[-_]?\s*(maj(?:or)?|min(?:or)?|m))?(?![a-z0-9])",
    re.I,
)

KEY_ALIASES = {
    "db": "C#",
    "eb": "D#",
    "gb": "F#",
    "ab": "G#",
    "bb": "A#",
}


def analyze_path(relative_path: str) -> list[TagCandidate]:
    """Return heuristic tag candidates for a sample filename."""
    filename = PurePath(relative_path).name
    normalized_filename = normalize_path(filename)
    tokens = tokenize(normalized_filename)
    candidates: dict[tuple[str, str, str], TagCandidate] = {}

    for rule in load_token_rules("type"):
        if rule_matches(rule, normalized_filename, tokens):
            add_candidate(candidates, "Type", rule.value, rule.confidence)

    tempo_match = TEMPO_RE.search(filename)
    if tempo_match is not None:
        add_candidate(candidates, "Type", "loop", 0.7)

    for rule in load_token_rules("instrument"):
        if rule_matches(rule, normalized_filename, tokens):
            add_candidate(candidates, "Instrument", rule.value, rule.confidence)

    if tempo_match is not None:
        add_candidate(candidates, "Tempo", tempo_match.group(1), 0.95)
    elif (inline_match := INLINE_TEMPO_RE.search(filename)) is not None:
        add_candidate(candidates, "Tempo", inline_match.group(1), 0.7)

    key_match = KEY_RE.search(filename)
    if key_match is not None:
        note = normalize_key(key_match.group(1), key_match.group(2))
        mode = normalize_mode(key_match.group(3))
        confidence = 0.85 if mode else 0.65
        add_candidate(candidates, "Key", note, confidence)
        if mode is not None:
            add_candidate(candidates, "Mode", mode, confidence)

    return list(candidates.values())


def normalize_path(relative_path: str) -> str:
    parts = PurePath(relative_path.lower()).parts
    return " ".join(parts)


def tokenize(value: str) -> set[str]:
    return {token for token in TOKEN_SPLIT_RE.split(value.lower()) if token}


def normalize_key(note: str, accidental: str) -> str:
    raw = f"{note.lower()}{accidental.lower()}"
    if raw in KEY_ALIASES:
        return KEY_ALIASES[raw]
    return raw[0].upper() + raw[1:]


def normalize_mode(mode: str | None) -> str | None:
    if mode is None:
        return None
    normalized = mode.strip().lower()
    if normalized in {"maj", "major"}:
        return "major"
    if normalized in {"min", "minor", "m"}:
        return "minor"
    return None


def rule_matches(rule: TokenRule, normalized_path: str, tokens: set[str]) -> bool:
    for token in rule.tokens:
        normalized_token = token.lower()
        if " " in normalized_token or "-" in normalized_token or "_" in normalized_token:
            pattern = r"(?<![a-z0-9])" + re.escape(normalized_token).replace(r"\ ", r"[\s_-]+")
            pattern += r"(?![a-z0-9])"
            if re.search(pattern, normalized_path):
                return True
        elif normalized_token in tokens:
            return True
    return False


def add_candidate(
    candidates: dict[tuple[str, str, str], TagCandidate],
    dimension: str,
    value: str,
    confidence: float,
) -> None:
    key = (dimension, value, "heuristic")
    existing = candidates.get(key)
    if existing is None or confidence > existing.confidence:
        candidates[key] = TagCandidate(
            dimension=dimension,
            value=value,
            source="heuristic",
            confidence=confidence,
        )


@lru_cache(maxsize=1)
def load_config() -> _TokenRuleFileModel:
    config_path = files("sonoscope_analyzer").joinpath("mappings/heuristic_tokens.json")
    with config_path.open("r", encoding="utf-8") as handle:
        return _TokenRuleFileModel.model_validate_json(handle.read())


def load_token_rules(group: str) -> tuple[TokenRule, ...]:
    raw_rules: list[_TokenRuleModel] = getattr(load_config(), group, [])
    return tuple(
        TokenRule(value=rule.value, tokens=rule.tokens, confidence=rule.confidence)
        for rule in raw_rules
    )
