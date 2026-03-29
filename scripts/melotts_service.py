from __future__ import annotations

import os
from contextlib import asynccontextmanager

import numpy as np
import torch
import uvicorn
from fastapi import FastAPI, HTTPException
from fastapi.responses import StreamingResponse
from melo.api import TTS
from pydantic import BaseModel


HOST = os.environ.get("MELOTTS_HOST", "127.0.0.1")
PORT = int(os.environ.get("MELOTTS_PORT", "8000"))
DEVICE = os.environ.get("MELOTTS_DEVICE", "auto")
DEFAULT_LANGUAGE = os.environ.get("MELOTTS_LANGUAGE", "ZH")
WARMUP_ENABLED = os.environ.get("MELOTTS_WARMUP", "1").strip() not in {"0", "false", "False"}
WARMUP_TEXT = os.environ.get("MELOTTS_WARMUP_TEXT", "你好")

model: TTS | None = None
speaker_ids: dict[str, int] = {}
active_device = "unknown"
warmup_ready = False


def resolve_device() -> str:
    configured = DEVICE.strip().lower()
    if configured and configured != "auto":
        return DEVICE
    return "cuda:0" if torch.cuda.is_available() else "cpu"


@asynccontextmanager
async def lifespan(_: FastAPI):
    global model, speaker_ids, active_device, warmup_ready
    active_device = resolve_device()
    model = TTS(language=DEFAULT_LANGUAGE, device=active_device)
    speaker_ids = model.hps.data.spk2id
    if WARMUP_ENABLED:
        try:
            warmup_model()
        except Exception as exc:
            print(f"[melotts] warmup skipped: {exc}")
    warmup_ready = True
    yield


app = FastAPI(title="Fast Avatar AI MeloTTS Service", lifespan=lifespan)


class SpeechRequest(BaseModel):
    input: str
    speed: float = 1.0
    language: str = DEFAULT_LANGUAGE


@app.get("/health")
async def health() -> dict[str, object]:
    return {
        "ok": model is not None,
        "device": active_device,
        "language": DEFAULT_LANGUAGE,
        "speaker_ids": list(speaker_ids.keys()),
        "warmup_ready": warmup_ready,
    }


def warmup_model() -> None:
    if model is None:
        return

    warmup_language = DEFAULT_LANGUAGE if DEFAULT_LANGUAGE in speaker_ids else next(iter(speaker_ids), None)
    if warmup_language is None:
        return

    model.tts_to_file(
        WARMUP_TEXT,
        speaker_ids[warmup_language],
        output_path=None,
        speed=1.0,
        quiet=True,
    )


def synthesize_audio(text: str, language: str, speed: float) -> tuple[bytes, int]:
    if model is None:
        raise HTTPException(status_code=503, detail="MeloTTS model not ready")

    if language not in speaker_ids:
        raise HTTPException(status_code=400, detail="Invalid language or speaker id")

    try:
        audio = model.tts_to_file(
            text,
            speaker_ids[language],
            output_path=None,
            speed=speed,
            quiet=True,
        )
    except HTTPException:
        raise
    except Exception as exc:
        raise HTTPException(status_code=500, detail=f"Speech generation failed: {exc}") from exc

    clipped = np.clip(audio, -1.0, 1.0)
    pcm = (clipped * 32767.0).astype(np.int16).tobytes()
    return pcm, int(model.hps.data.sampling_rate)


def iter_pcm_chunks(audio_bytes: bytes, chunk_size: int = 8192):
    view = memoryview(audio_bytes)
    for start in range(0, len(view), chunk_size):
        yield view[start : start + chunk_size].tobytes()


@app.post("/speech")
async def speech(request: SpeechRequest) -> StreamingResponse:
    audio_binary, sample_rate = synthesize_audio(request.input, request.language, request.speed)
    headers = {
        "X-Audio-Format": "pcm_s16le",
        "X-Sample-Rate": str(sample_rate),
        "X-Channels": "1",
    }
    return StreamingResponse(
        iter_pcm_chunks(audio_binary),
        media_type="application/octet-stream",
        headers=headers,
    )


if __name__ == "__main__":
    uvicorn.run(app, host=HOST, port=PORT, log_level="warning")
