"""PyTorch device and tensor helpers, kept import-light.

Torch is only imported lazily via ``import_module`` so unit tests can run
without model dependencies installed.
"""

from __future__ import annotations

import os
from importlib import import_module
from typing import Any

import numpy as np

from sonoscope_analyzer.interfaces import clamp_confidence


def env_flag(name: str) -> bool:
    return os.environ.get(name, "").strip().lower() in {"1", "true", "yes", "on"}


def is_torch_device_available(device: str) -> bool:
    base_device = device.split(":", maxsplit=1)[0]
    if base_device == "cpu":
        return True
    try:
        torch = import_module("torch")
    except (ImportError, OSError, RuntimeError):
        return False
    if base_device == "cuda":
        cuda = getattr(torch, "cuda", None)
        is_available = getattr(cuda, "is_available", None)
        device_count = getattr(cuda, "device_count", None)
        if not callable(is_available) or not bool(is_available()):
            return False
        if callable(device_count) and int(device_count()) <= 0:
            return False
        return True
    if base_device == "mps":
        backends = getattr(torch, "backends", None)
        mps = getattr(backends, "mps", None)
        is_available = getattr(mps, "is_available", None)
        return callable(is_available) and bool(is_available())
    return False


def torch_inference_context(torch: Any) -> Any:
    inference_mode = getattr(torch, "inference_mode", None)
    if callable(inference_mode):
        return inference_mode()
    return torch.no_grad()


def configure_torch_inference(torch: Any, device: str) -> None:
    if device.split(":", maxsplit=1)[0] != "cuda" or env_flag("SONOSCOPE_DISABLE_CUDA_TF32"):
        return

    set_precision = getattr(torch, "set_float32_matmul_precision", None)
    if callable(set_precision):
        try:
            set_precision("high")
        except (AssertionError, RuntimeError, ValueError):
            pass

    backends = getattr(torch, "backends", None)
    cuda_backend = getattr(backends, "cuda", None)
    matmul = getattr(cuda_backend, "matmul", None)
    if matmul is not None and hasattr(matmul, "allow_tf32"):
        matmul.allow_tf32 = True

    cudnn = getattr(backends, "cudnn", None)
    if cudnn is not None and hasattr(cudnn, "allow_tf32"):
        cudnn.allow_tf32 = True


def tensor_to_float_list(values: object) -> list[float]:
    detach = getattr(values, "detach", None)
    if callable(detach):
        values = detach()
    cpu = getattr(values, "cpu", None)
    if callable(cpu):
        values = cpu()
    numpy = getattr(values, "numpy", None)
    if callable(numpy):
        values = numpy()
    return [clamp_confidence(value) for value in np.asarray(values, dtype=np.float32).reshape(-1)]


def tensor_to_nested_float_list(values: object) -> list[list[float]]:
    detach = getattr(values, "detach", None)
    if callable(detach):
        values = detach()
    cpu = getattr(values, "cpu", None)
    if callable(cpu):
        values = cpu()
    numpy = getattr(values, "numpy", None)
    if callable(numpy):
        values = numpy()
    array = np.asarray(values, dtype=np.float32)
    if array.ndim == 1:
        return [[clamp_confidence(value) for value in array.reshape(-1)]]
    return [
        [clamp_confidence(value) for value in row.reshape(-1)]
        for row in array.reshape(array.shape[0], -1)
    ]
