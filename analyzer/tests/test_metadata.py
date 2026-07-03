"""Tests for audio metadata extraction."""

from __future__ import annotations

import numpy as np
import soundfile as sf
from mutagen.flac import FLAC

from sonoscope_analyzer.metadata import extract_metadata


def test_extract_file_meta_from_wav(tmp_path) -> None:
    path = tmp_path / "tone.wav"
    sf.write(path, np.zeros(4410, dtype=np.float32), 44100)

    file_meta, tags = extract_metadata(str(path))

    assert file_meta.format == "wav"
    assert file_meta.duration_ms == 100
    assert file_meta.sample_rate == 44100
    assert file_meta.channels == 1
    assert tags == []


def test_extract_tag_candidates_from_flac(tmp_path) -> None:
    path = tmp_path / "tagged.flac"
    sf.write(path, np.zeros(4410, dtype=np.float32), 44100, format="FLAC")

    flac = FLAC(path)
    flac["BPM"] = "128"
    flac["INITIALKEY"] = "Bb minor"
    flac["GENRE"] = "Vocal"
    flac.save()

    file_meta, tags = extract_metadata(str(path))
    tag_pairs = {(tag.dimension, tag.value, tag.source) for tag in tags}

    assert file_meta.sample_rate == 44100
    assert ("Tempo", "128", "metadata") in tag_pairs
    assert ("Key", "A#", "metadata") in tag_pairs
    assert ("Mode", "minor", "metadata") in tag_pairs
    assert ("Instrument", "vocal", "metadata") in tag_pairs
