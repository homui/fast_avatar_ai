Place sherpa-onnx runtime models here before bundling.

Recommended layout:

- `asr/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09/`
  - `model.int8.onnx`
  - `tokens.txt`
- `vad/`
  - `silero_vad.int8.onnx`
- `tts/vits-melo-tts-zh_en/`
  - `model.onnx`
  - `tokens.txt`
  - `lexicon.txt`
  - `dict/`
  - `espeak-ng-data/`
- `tts/kokoro-multi-lang/`
  - `model.onnx`
  - `voices.bin`
  - `tokens.txt`
  - `lexicon.txt`
  - `dict/`
  - `espeak-ng-data/`
