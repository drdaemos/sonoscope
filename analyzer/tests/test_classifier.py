"""Tests for mocked ML output mapping."""

from __future__ import annotations

from collections.abc import Sequence
from importlib.resources import files

import numpy as np
import pytest
import soundfile

from sonoscope_analyzer.classifier import (
    ClapPromptTagModel,
    ClapSimilarityModel,
    CompositeTagModel,
    EssentiaLoopRoleTagModel,
    LoopPrediction,
    ModelPrediction,
    OnsetLoopDetector,
    PromptCandidate,
    TagPrediction,
    classify_audio,
    classify_audio_batch,
    load_clap_prompt_candidates,
    load_default_model_backends,
    load_default_tag_model,
    loop_role_predictions,
    resolve_clap_batch_size,
    resolve_clap_device,
    resolve_clap_model_id,
)


class FakeInstrumentModel:
    def __init__(self, predictions: Sequence[ModelPrediction]) -> None:
        self.predictions = predictions

    def predict(self, path: str) -> Sequence[ModelPrediction]:
        assert path
        return self.predictions


class FakeLoopDetector:
    def __init__(self, prediction: LoopPrediction | None) -> None:
        self.prediction = prediction

    def predict(self, path: str) -> LoopPrediction | None:
        assert path
        return self.prediction


class FailingInstrumentModel:
    def predict(self, path: str) -> Sequence[ModelPrediction]:
        assert path
        raise RuntimeError("model unavailable")


class FailingTagModel:
    def predict_batch(self, paths: Sequence[str]) -> Sequence[Sequence[TagPrediction]]:
        assert paths
        raise RuntimeError("model unavailable")


class FakeSimilarityModel:
    def __init__(self, scores_by_prompt: dict[str, float]) -> None:
        self.scores_by_prompt = scores_by_prompt

    def score_batch(
        self,
        paths: Sequence[str],
        prompts: Sequence[str],
    ) -> Sequence[Sequence[float]]:
        assert paths
        return [[self.scores_by_prompt.get(prompt, 0.0) for prompt in prompts] for _path in paths]


class FakeBatchSimilarityModel:
    def __init__(self, scores_by_path: dict[str, dict[str, float]]) -> None:
        self.scores_by_path = scores_by_path
        self.batch_calls: list[tuple[list[str], list[str]]] = []

    def score(self, path: str, prompts: Sequence[str]) -> Sequence[float]:
        return self.score_batch([path], prompts)[0]

    def score_batch(
        self,
        paths: Sequence[str],
        prompts: Sequence[str],
    ) -> Sequence[Sequence[float]]:
        self.batch_calls.append((list(paths), list(prompts)))
        return [
            [self.scores_by_path.get(path, {}).get(prompt, 0.0) for prompt in prompts]
            for path in paths
        ]


class FakeEssentiaFactory:
    def __init__(self, result: object) -> None:
        self.result = result
        self.kwargs: dict[str, object] | None = None
        self.configured = False

    def __call__(self, *args: object, **kwargs: object) -> object:
        if args or (self.configured and not kwargs):
            return self.result
        self.kwargs = kwargs
        self.configured = True
        return self


class FakeEssentiaStandard:
    def __init__(self) -> None:
        self.loader = FakeEssentiaFactory(np.asarray([0.0, 0.1], dtype=np.float32))
        self.embedding = FakeEssentiaFactory(np.asarray([[0.1, 0.2]], dtype=np.float32))
        self.classifier = FakeEssentiaFactory(
            np.asarray([[0.1, 0.42, 0.05, 0.7, 0.2]], dtype=np.float32)
        )

    def MonoLoader(self, **kwargs: object) -> object:
        return self.loader(**kwargs)

    def TensorflowPredictMusiCNN(self, **kwargs: object) -> object:
        return self.embedding(**kwargs)

    def TensorflowPredict2D(self, **kwargs: object) -> object:
        return self.classifier(**kwargs)


class FakeCuda:
    def __init__(self, available: bool, device_count: int = 1) -> None:
        self.available = available
        self._device_count = device_count

    def is_available(self) -> bool:
        return self.available

    def device_count(self) -> int:
        return self._device_count


class FakeMps:
    def __init__(self, available: bool) -> None:
        self.available = available

    def is_available(self) -> bool:
        return self.available


class FakeBackends:
    def __init__(self, *, mps_available: bool) -> None:
        self.mps = FakeMps(mps_available)


class FakeNoGrad:
    def __enter__(self) -> None:
        return None

    def __exit__(self, exc_type: object, exc: object, traceback: object) -> bool:
        return False


class FakeTorch:
    def __init__(
        self,
        *,
        cuda_available: bool = False,
        cuda_device_count: int = 1,
        mps_available: bool = False,
    ) -> None:
        self.cuda = FakeCuda(cuda_available, cuda_device_count)
        self.backends = FakeBackends(mps_available=mps_available)

    def no_grad(self) -> FakeNoGrad:
        return FakeNoGrad()

    def softmax(self, values: object, *, dim: int) -> np.ndarray:
        assert values is not None
        assert dim == -1
        logits = np.asarray(values, dtype=np.float32)
        if logits.ndim <= 1:
            return np.asarray([0.25, 0.75], dtype=np.float32)
        rows = []
        for index in range(logits.shape[0]):
            rows.append([0.25, 0.75] if index == 0 else [0.8, 0.2])
        return np.asarray(rows, dtype=np.float32)


class FakeBatchEncoding(dict[str, object]):
    def __init__(self, batch_size: int, failing_device: str | None = None) -> None:
        super().__init__(input_values=object(), batch_size=batch_size)
        self.failing_device = failing_device
        self.devices: list[str] = []

    def to(self, device: str) -> "FakeBatchEncoding":
        self.devices.append(device)
        if device == self.failing_device:
            raise RuntimeError(f"{device} unavailable")
        return self


class FakeClapProcessor:
    def __init__(self, failing_device: str | None = None) -> None:
        self.failing_device = failing_device
        self.inputs: list[FakeBatchEncoding] = []
        self.audio_batch_sizes: list[int] = []

    def __call__(
        self,
        *,
        text: list[str],
        audios: object,
        sampling_rate: int,
        return_tensors: str,
        padding: bool,
    ) -> FakeBatchEncoding:
        assert text
        if isinstance(audios, list):
            batch_size = len(audios)
            assert batch_size > 0
            assert all(isinstance(audio, np.ndarray) and audio.size > 0 for audio in audios)
        else:
            batch_size = 1
            assert isinstance(audios, np.ndarray)
            assert audios.size > 0
        self.audio_batch_sizes.append(batch_size)
        assert sampling_rate > 0
        assert return_tensors == "pt"
        assert padding is True
        inputs = FakeBatchEncoding(batch_size, self.failing_device)
        self.inputs.append(inputs)
        return inputs


class FakeClapOutput:
    def __init__(self, batch_size: int) -> None:
        self.logits_per_audio = [[0.0, 1.0] for _index in range(batch_size)]


class FakeClapModel:
    def __init__(self) -> None:
        self.devices: list[str] = []

    def to(self, device: str) -> "FakeClapModel":
        self.devices.append(device)
        return self

    def __call__(self, **inputs: object) -> FakeClapOutput:
        assert inputs
        raw_batch_size = inputs.get("batch_size", 1)
        batch_size = raw_batch_size if isinstance(raw_batch_size, int) else 1
        return FakeClapOutput(batch_size)


def test_classify_audio_maps_explicit_model_label_to_instrument() -> None:
    tags = classify_audio(
        "sample.wav",
        instrument_model=FakeInstrumentModel([ModelPrediction("Instrument:kick", 0.82)]),
    )

    assert [(tag.dimension, tag.value, tag.source, tag.confidence) for tag in tags] == [
        ("Instrument", "kick", "model", 0.82)
    ]


def test_classify_audio_keeps_normalized_legacy_instrument_label() -> None:
    tags = classify_audio(
        "sample.wav",
        instrument_model=FakeInstrumentModel([ModelPrediction("Bass drum", 0.4)]),
    )

    assert [(tag.dimension, tag.value, tag.source, tag.confidence) for tag in tags] == [
        ("Instrument", "bass-drum", "model", 0.4)
    ]


def test_classify_audio_deduplicates_direct_tags_by_highest_confidence() -> None:
    tags = classify_audio(
        "sample.wav",
        instrument_model=FakeInstrumentModel(
            [
                ModelPrediction("Instrument:guitar", 0.61),
                ModelPrediction("Instrument:guitar", 0.74),
            ]
        ),
    )

    assert len(tags) == 1
    assert tags[0].dimension == "Instrument"
    assert tags[0].value == "guitar"
    assert tags[0].confidence == 0.74


def test_classify_audio_maps_loop_detector_output_to_type() -> None:
    tags = classify_audio(
        "loop.wav",
        loop_detector=FakeLoopDetector(LoopPrediction(is_loop=True, confidence=0.91)),
    )

    assert [(tag.dimension, tag.value, tag.source) for tag in tags] == [("Type", "loop", "model")]


def test_classify_audio_maps_non_loop_output_to_one_shot() -> None:
    tags = classify_audio(
        "hit.wav",
        loop_detector=FakeLoopDetector(LoopPrediction(is_loop=False, confidence=0.88)),
    )

    assert [(tag.dimension, tag.value, tag.source) for tag in tags] == [
        ("Type", "one-shot", "model")
    ]


def test_classify_audio_has_no_model_dependency_by_default() -> None:
    assert classify_audio("sample.wav") == []


def test_classify_audio_skips_unavailable_model_backend() -> None:
    assert classify_audio("sample.wav", instrument_model=FailingInstrumentModel()) == []


def test_clap_prompt_tag_model_maps_prompt_scores_to_tags() -> None:
    model = ClapPromptTagModel(
        FakeSimilarityModel(
            {
                "a kick drum sample": 0.66,
                "a punchy bass drum one shot": 0.7,
                "a snare drum sample": 0.11,
            }
        ),
        [
            PromptCandidate(
                dimension="Instrument",
                value="kick",
                prompts=("a kick drum sample", "a punchy bass drum one shot"),
                min_confidence=0.12,
            ),
            PromptCandidate(
                dimension="Instrument",
                value="snare",
                prompts=("a snare drum sample",),
                min_confidence=0.12,
            ),
        ],
    )

    predictions = model.predict("sample.wav")

    assert len(predictions) == 1
    assert predictions[0].dimension == "Instrument"
    assert predictions[0].value == "kick"
    assert predictions[0].confidence == pytest.approx(0.7)


def test_classify_audio_includes_clap_prompt_tags() -> None:
    tag_model = ClapPromptTagModel(
        FakeSimilarityModel({"a kick drum sample": 0.7}),
        [
            PromptCandidate(
                dimension="Instrument",
                value="kick",
                prompts=("a kick drum sample",),
                min_confidence=0.12,
            )
        ],
    )

    tags = classify_audio("sample.wav", tag_model=tag_model)

    assert [(tag.dimension, tag.value, tag.source, tag.confidence) for tag in tags] == [
        ("Instrument", "kick", "model", 0.7)
    ]


def test_clap_prompt_tag_model_scores_paths_in_one_batch() -> None:
    similarity_model = FakeBatchSimilarityModel(
        {
            "kick.wav": {"a kick drum sample": 0.82, "a snare drum sample": 0.2},
            "snare.wav": {"a kick drum sample": 0.12, "a snare drum sample": 0.76},
        }
    )
    tag_model = ClapPromptTagModel(
        similarity_model,
        [
            PromptCandidate(
                dimension="Instrument",
                value="kick",
                prompts=("a kick drum sample",),
                min_confidence=0.3,
            ),
            PromptCandidate(
                dimension="Instrument",
                value="snare",
                prompts=("a snare drum sample",),
                min_confidence=0.3,
            ),
        ],
    )

    predictions = tag_model.predict_batch(["kick.wav", "snare.wav"])

    assert [[prediction.value for prediction in row] for row in predictions] == [
        ["kick"],
        ["snare"],
    ]
    assert similarity_model.batch_calls == [
        (["kick.wav", "snare.wav"], ["a kick drum sample", "a snare drum sample"])
    ]


def test_clap_prompt_tag_model_supports_dimension_specific_top_k() -> None:
    tag_model = ClapPromptTagModel(
        FakeSimilarityModel(
            {
                "a major key music sample": 0.64,
                "a minor key music sample": 0.72,
            }
        ),
        [
            PromptCandidate(
                dimension="Mode",
                value="major",
                prompts=("a major key music sample",),
                min_confidence=0.12,
            ),
            PromptCandidate(
                dimension="Mode",
                value="minor",
                prompts=("a minor key music sample",),
                min_confidence=0.12,
            ),
        ],
        top_k_by_dimension={"Mode": 1},
    )

    predictions = tag_model.predict("sample.wav")

    assert [(prediction.dimension, prediction.value) for prediction in predictions] == [
        ("Mode", "minor")
    ]


def test_loop_role_predictions_map_essentia_roles_to_instruments() -> None:
    predictions = loop_role_predictions([0.12, 0.48, 0.02, 0.7, 0.36], 0.35)

    assert [(prediction.dimension, prediction.value) for prediction in predictions] == [
        ("Instrument", "lead"),
        ("Instrument", "chord"),
    ]
    assert [prediction.confidence for prediction in predictions] == [
        pytest.approx(0.7),
        pytest.approx(0.48),
    ]


def test_essentia_loop_role_tag_model_maps_predictions(monkeypatch, tmp_path) -> None:
    (tmp_path / "msd-musicnn-1.pb").write_text("x", encoding="utf-8")
    (tmp_path / "fs_loop_ds-msd-musicnn-1.pb").write_text("x", encoding="utf-8")
    fake_standard = FakeEssentiaStandard()

    def fake_import_module(name: str) -> object:
        if name == "essentia.standard":
            return fake_standard
        raise ImportError(name)

    monkeypatch.setattr("sonoscope_analyzer.adapters.essentia.import_module", fake_import_module)

    predictions = EssentiaLoopRoleTagModel(tmp_path).predict("loop.wav")

    assert [(prediction.dimension, prediction.value) for prediction in predictions] == [
        ("Instrument", "lead"),
        ("Instrument", "chord"),
    ]
    assert [prediction.confidence for prediction in predictions] == [
        pytest.approx(0.7),
        pytest.approx(0.42),
    ]
    assert fake_standard.loader.kwargs == {
        "filename": "loop.wav",
        "sampleRate": 16000,
        "resampleQuality": 4,
    }


def test_classify_audio_batch_uses_batch_tag_model_once() -> None:
    similarity_model = FakeBatchSimilarityModel(
        {
            "kick.wav": {"a kick drum sample": 0.82},
            "other.wav": {"a kick drum sample": 0.1},
        }
    )
    tag_model = ClapPromptTagModel(
        similarity_model,
        [
            PromptCandidate(
                dimension="Instrument",
                value="kick",
                prompts=("a kick drum sample",),
                min_confidence=0.3,
            )
        ],
    )

    tag_batch = classify_audio_batch(["kick.wav", "other.wav"], tag_model=tag_model)

    assert [[tag.value for tag in tags] for tags in tag_batch] == [["kick"], []]
    assert len(similarity_model.batch_calls) == 1


def test_composite_tag_model_keeps_predictions_when_optional_backend_fails() -> None:
    tag_model = ClapPromptTagModel(
        FakeSimilarityModel({"a kick drum sample": 0.7}),
        [
            PromptCandidate(
                dimension="Instrument",
                value="kick",
                prompts=("a kick drum sample",),
                min_confidence=0.12,
            )
        ],
    )
    composite = CompositeTagModel([FailingTagModel(), tag_model])

    predictions = composite.predict_batch(["sample.wav"])

    assert [[prediction.value for prediction in row] for row in predictions] == [["kick"]]


def test_resolve_clap_device_prefers_cuda(monkeypatch) -> None:
    fake_torch = FakeTorch(cuda_available=True, mps_available=True)

    def fake_import_module(name: str) -> object:
        if name == "torch":
            return fake_torch
        raise ImportError(name)

    monkeypatch.setattr("sonoscope_analyzer.torch_utils.import_module", fake_import_module)

    assert resolve_clap_device("auto") == "cuda"


def test_resolve_clap_device_uses_mps_before_cpu(monkeypatch) -> None:
    fake_torch = FakeTorch(cuda_available=False, mps_available=True)

    def fake_import_module(name: str) -> object:
        if name == "torch":
            return fake_torch
        raise ImportError(name)

    monkeypatch.setattr("sonoscope_analyzer.torch_utils.import_module", fake_import_module)

    assert resolve_clap_device("auto") == "mps"


def test_resolve_clap_device_falls_back_when_requested_device_is_unavailable(monkeypatch) -> None:
    fake_torch = FakeTorch(cuda_available=False, mps_available=True)

    def fake_import_module(name: str) -> object:
        if name == "torch":
            return fake_torch
        raise ImportError(name)

    monkeypatch.setattr("sonoscope_analyzer.torch_utils.import_module", fake_import_module)

    assert resolve_clap_device("cuda") == "mps"


def test_resolve_clap_device_uses_cpu_without_torch(monkeypatch) -> None:
    def fake_import_module(name: str) -> object:
        raise ImportError(name)

    monkeypatch.setattr("sonoscope_analyzer.torch_utils.import_module", fake_import_module)

    assert resolve_clap_device("auto") == "cpu"


def test_clap_similarity_model_retries_on_cpu_when_accelerator_fails(monkeypatch) -> None:
    fake_torch = FakeTorch(cuda_available=True)
    fake_processor = FakeClapProcessor(failing_device="cuda")
    fake_model = FakeClapModel()

    def fake_import_module(name: str) -> object:
        if name == "torch":
            return fake_torch
        raise ImportError(name)

    monkeypatch.setattr("sonoscope_analyzer.adapters.clap.import_module", fake_import_module)
    monkeypatch.setattr("sonoscope_analyzer.torch_utils.import_module", fake_import_module)
    monkeypatch.setattr(
        "sonoscope_analyzer.adapters.clap.read_mono_audio",
        lambda path, target_sample_rate: (np.asarray([0.1, 0.2], dtype=np.float32), 48_000),
    )
    model = ClapSimilarityModel(device="auto", model=fake_model, processor=fake_processor)

    scores = model.score("sample.wav", ["first", "second"])

    assert scores == [pytest.approx(0.25), pytest.approx(0.75)]
    assert [inputs.devices for inputs in fake_processor.inputs] == [["cuda"], ["cpu"]]
    assert fake_model.devices == ["cpu"]
    assert model.device == "cpu"


def test_clap_similarity_model_scores_non_empty_audio_in_one_batch(monkeypatch) -> None:
    fake_torch = FakeTorch(cuda_available=False)
    fake_processor = FakeClapProcessor()
    fake_model = FakeClapModel()

    def fake_import_module(name: str) -> object:
        if name == "torch":
            return fake_torch
        raise ImportError(name)

    def fake_read_mono_audio(path: str, target_sample_rate: int) -> tuple[np.ndarray, int]:
        assert target_sample_rate == 48_000
        if path == "empty.wav":
            return np.asarray([], dtype=np.float32), 48_000
        return np.asarray([0.1, 0.2], dtype=np.float32), 48_000

    monkeypatch.setattr("sonoscope_analyzer.adapters.clap.import_module", fake_import_module)
    monkeypatch.setattr("sonoscope_analyzer.torch_utils.import_module", fake_import_module)
    monkeypatch.setattr("sonoscope_analyzer.adapters.clap.read_mono_audio", fake_read_mono_audio)
    model = ClapSimilarityModel(device="auto", model=fake_model, processor=fake_processor)

    scores = model.score_batch(["kick.wav", "snare.wav", "empty.wav"], ["first", "second"])

    assert scores == [
        [pytest.approx(0.25), pytest.approx(0.75)],
        [pytest.approx(0.8), pytest.approx(0.2)],
        [0.0, 0.0],
    ]
    assert fake_processor.audio_batch_sizes == [2]


def test_clap_similarity_model_splits_large_audio_batch(monkeypatch) -> None:
    fake_torch = FakeTorch(cuda_available=False)
    fake_processor = FakeClapProcessor()
    fake_model = FakeClapModel()

    def fake_import_module(name: str) -> object:
        if name == "torch":
            return fake_torch
        raise ImportError(name)

    monkeypatch.setattr("sonoscope_analyzer.adapters.clap.import_module", fake_import_module)
    monkeypatch.setattr("sonoscope_analyzer.torch_utils.import_module", fake_import_module)
    monkeypatch.setattr(
        "sonoscope_analyzer.adapters.clap.read_mono_audio",
        lambda path, target_sample_rate: (np.asarray([0.1, 0.2], dtype=np.float32), 48_000),
    )
    monkeypatch.setenv("SONOSCOPE_CLAP_BATCH_SIZE", "2")
    model = ClapSimilarityModel(device="auto", model=fake_model, processor=fake_processor)

    scores = model.score_batch(["kick.wav", "snare.wav", "hat.wav"], ["first", "second"])

    assert scores == [
        [pytest.approx(0.25), pytest.approx(0.75)],
        [pytest.approx(0.8), pytest.approx(0.2)],
        [pytest.approx(0.25), pytest.approx(0.75)],
    ]
    assert fake_processor.audio_batch_sizes == [2, 1]


def test_resolve_clap_batch_size_clamps_to_available_audio_count() -> None:
    assert resolve_clap_batch_size("32", 3) == 3
    assert resolve_clap_batch_size("0", 3) == 1
    assert resolve_clap_batch_size("invalid", 20) == 16


def test_load_clap_prompt_candidates(tmp_path) -> None:
    path = tmp_path / "clap_prompts.json"
    path.write_text(
        """
        {
          "candidates": [
            {
              "dimension": "Instrument",
              "value": "kick",
              "prompts": ["a kick drum sample", "  "],
              "min_confidence": 0.2
            }
          ]
        }
        """,
        encoding="utf-8",
    )

    candidates = load_clap_prompt_candidates(path)

    assert len(candidates) == 1
    assert candidates[0].dimension == "Instrument"
    assert candidates[0].value == "kick"
    assert candidates[0].prompts == ("a kick drum sample",)
    assert candidates[0].min_confidence == 0.2


def test_bundled_clap_prompts_cover_system_instruments() -> None:
    candidates = load_clap_prompt_candidates(
        files("sonoscope_analyzer").joinpath("mappings/clap_prompts.json")
    )

    instrument_values = {
        candidate.value for candidate in candidates if candidate.dimension == "Instrument"
    }

    assert {
        "kick",
        "snare",
        "hi-hat",
        "clap",
        "percussion",
        "bass",
        "chord",
        "pad",
        "synth",
        "lead",
        "vocal",
        "fx",
        "foley",
        "cymbal",
        "guitar",
        "piano",
        "brass",
        "woodwind",
        "strings",
        "tops",
    } <= instrument_values


def test_onset_loop_detector_detects_short_one_shot(tmp_path) -> None:
    path = tmp_path / "hit.wav"
    samples = np.zeros(4_410, dtype=np.float32)
    samples[100:300] = 0.9
    soundfile.write(path, samples, 22_050)

    prediction = OnsetLoopDetector().predict(str(path))

    assert prediction == LoopPrediction(is_loop=False, confidence=0.8)


def test_default_model_backends_do_not_use_onset_loop_fallback(monkeypatch) -> None:
    monkeypatch.delenv("SONOSCOPE_DISABLE_ML", raising=False)
    monkeypatch.delenv("SONOSCOPE_ESSENTIA_LOOP_MODEL", raising=False)
    monkeypatch.delenv("SONOSCOPE_ENABLE_ONSET_LOOP_FALLBACK", raising=False)
    load_default_model_backends.cache_clear()

    backends = load_default_model_backends()

    assert backends.loop_detector is None
    load_default_model_backends.cache_clear()


def test_default_model_backends_can_be_disabled(monkeypatch) -> None:
    monkeypatch.setenv("SONOSCOPE_DISABLE_ML", "1")
    load_default_model_backends.cache_clear()

    backends = load_default_model_backends()

    assert backends.tag_model is None
    assert backends.loop_detector is None
    load_default_model_backends.cache_clear()


def test_resolve_clap_model_id_uses_ready_local_path(tmp_path, monkeypatch) -> None:
    for file_name in (
        "config.json",
        "merges.txt",
        "preprocessor_config.json",
        "pytorch_model.bin",
        "special_tokens_map.json",
        "tokenizer.json",
        "tokenizer_config.json",
        "vocab.json",
    ):
        (tmp_path / file_name).write_text("x", encoding="utf-8")
    monkeypatch.setenv("SONOSCOPE_CLAP_MODEL_PATH", str(tmp_path))
    monkeypatch.setenv("SONOSCOPE_CLAP_LOCAL_ONLY", "1")

    assert resolve_clap_model_id() == str(tmp_path)


def test_default_tag_model_skips_missing_local_model_when_required(monkeypatch, tmp_path) -> None:
    monkeypatch.setenv("SONOSCOPE_CLAP_MODEL_PATH", str(tmp_path))
    monkeypatch.setenv("SONOSCOPE_CLAP_LOCAL_ONLY", "1")
    load_default_tag_model.cache_clear()

    assert load_default_tag_model() is None
    load_default_tag_model.cache_clear()
