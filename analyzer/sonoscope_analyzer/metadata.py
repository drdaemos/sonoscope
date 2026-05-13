"""Audio metadata extraction for the analyzer sidecar."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import mutagen
import soundfile

from sonoscope_analyzer.protocol import FileMeta, TagCandidate

KEY_FIELDS = ("initialkey", "key", "tkey")
BPM_FIELDS = ("bpm", "tbpm", "tempo")
GENRE_FIELDS = ("genre", "tcon")

GENRE_TO_INSTRUMENT = {
    "bass": "bass",
    "drum": "percussion",
    "drums": "percussion",
    "fx": "fx",
    "kick": "kick",
    "pad": "pad",
    "snare": "snare",
    "synth": "synth",
    "vocal": "vocal",
    "vocals": "vocal",
}


def extract_metadata(path: str) -> tuple[FileMeta, list[TagCandidate]]:
    """Extract technical file metadata and mapped embedded tags."""
    file_path = Path(path)
    file_meta = extract_file_meta(file_path)
    tags = extract_tag_candidates(file_path)
    return file_meta, tags


def extract_file_meta(path: Path) -> FileMeta:
    """Extract technical audio metadata with SoundFile and Mutagen fallbacks."""
    file_meta = FileMeta(format=path.suffix.removeprefix(".").lower() or None)

    try:
        info = soundfile.info(str(path))
        file_meta.duration_ms = round((info.frames / info.samplerate) * 1000)
        file_meta.sample_rate = int(info.samplerate)
        file_meta.channels = int(info.channels)
        file_meta.format = info.format.lower()
    except (RuntimeError, OSError, ValueError):
        pass

    try:
        mutagen_file = mutagen.File(path)
    except mutagen.MutagenError:
        mutagen_file = None

    if mutagen_file is not None and getattr(mutagen_file, "info", None) is not None:
        info = mutagen_file.info
        if file_meta.duration_ms is None and hasattr(info, "length"):
            file_meta.duration_ms = round(float(info.length) * 1000)
        if file_meta.sample_rate is None and hasattr(info, "sample_rate"):
            file_meta.sample_rate = int(info.sample_rate)
        if file_meta.channels is None and hasattr(info, "channels"):
            file_meta.channels = int(info.channels)
        if file_meta.bit_depth is None and hasattr(info, "bits_per_sample"):
            file_meta.bit_depth = int(info.bits_per_sample)

    return file_meta


def extract_tag_candidates(path: Path) -> list[TagCandidate]:
    """Map embedded metadata tags to Sonoscope tag candidates."""
    try:
        mutagen_file = mutagen.File(path)
    except mutagen.MutagenError:
        return []

    if mutagen_file is None or mutagen_file.tags is None:
        return []

    flattened = flatten_tags(dict(mutagen_file.tags))
    candidates: list[TagCandidate] = []

    bpm = first_field(flattened, BPM_FIELDS)
    if bpm is not None:
        tempo = parse_tempo(bpm)
        if tempo is not None:
            candidates.append(
                TagCandidate(
                    dimension="Tempo",
                    value=str(tempo),
                    source="metadata",
                    confidence=0.95,
                )
            )

    key = first_field(flattened, KEY_FIELDS)
    if key is not None:
        normalized_key = parse_key(key)
        if normalized_key is not None:
            candidates.append(
                TagCandidate(
                    dimension="Key",
                    value=normalized_key,
                    source="metadata",
                    confidence=0.95,
                )
            )

    genre = first_field(flattened, GENRE_FIELDS)
    if genre is not None:
        instrument = GENRE_TO_INSTRUMENT.get(str(genre).strip().lower())
        if instrument is not None:
            candidates.append(
                TagCandidate(
                    dimension="Instrument",
                    value=instrument,
                    source="metadata",
                    confidence=0.4,
                )
            )

    return candidates


def flatten_tags(tags: dict[str, Any]) -> dict[str, str]:
    flattened: dict[str, str] = {}
    for key, raw_value in tags.items():
        normalized_key = key.lower()
        if isinstance(raw_value, (list, tuple)):
            value = raw_value[0] if raw_value else None
        else:
            value = raw_value
        if value is None:
            continue
        if hasattr(value, "text"):
            text_value = value.text[0] if value.text else None
        else:
            text_value = value
        if text_value is not None:
            flattened[normalized_key] = str(text_value)
    return flattened


def first_field(tags: dict[str, str], field_names: tuple[str, ...]) -> str | None:
    for field_name in field_names:
        value = tags.get(field_name)
        if value:
            return value
    return None


def parse_tempo(value: str) -> int | None:
    try:
        tempo = round(float(value.strip()))
    except ValueError:
        return None
    if 20 <= tempo <= 300:
        return tempo
    return None


def parse_key(value: str) -> str | None:
    normalized = value.strip().replace("♯", "#").replace("♭", "b")
    if not normalized:
        return None
    note = normalized.split()[0].split("-")[0]
    aliases = {"Db": "C#", "Eb": "D#", "Gb": "F#", "Ab": "G#", "Bb": "A#"}
    note = note[0].upper() + note[1:]
    return aliases.get(note, note)
