Place runtime models here before bundling the app.

Current default layout:

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

Notes:

- `models/` is used during development.
- `resources/models/` is used for packaged builds.
- Keep both directories in sync if you want the MSI build to ship with the same local models.
- See the repository README for download commands and official sherpa-onnx links.
