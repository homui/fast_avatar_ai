Place local runtime models here for development.

Current layout used by this repo:

- `asr/sherpa-onnx-streaming-zipformer-ctc-zh-int8-2025-06-30/`
  - `model.int8.onnx`
  - `tokens.txt`
- `vad/`
  - `silero_vad.int8.onnx`
- `tts/sherpa-onnx-vits-zh-ll/`
  - `model.onnx`
  - `tokens.txt`
  - `lexicon.txt`
  - `dict/`
  - `phone.fst`
  - `number.fst`
  - `date.fst`

Notes:

- `models/` is used during development.
- `resources/models/` is used for packaged builds.
- Keep both directories in sync if you want the packaged MSI build to ship with the same local models.
- The current codebase only wires up:
  - local ASR: `zipformer_ctc`
  - local VAD: `silero_vad`
  - local TTS: `sherpa_vits`
