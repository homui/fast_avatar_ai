Place sherpa-onnx runtime models here before bundling.

Recommended layout:

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
  - `espeak-ng-data/` optional

Notes:

- `zipformer_ctc` is loaded as a streaming ASR recognizer, but the current chat flow still emits final text after each VAD-completed utterance.
