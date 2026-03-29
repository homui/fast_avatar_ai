# 配置说明

本项目的主配置文件位于：

- [config/settings.json](/D:/software/fast_avatar_ai/config/settings.json)

角色目录位于：

- [config/characters.json](/D:/software/fast_avatar_ai/config/characters.json)

## 1. `settings.json` 结构

```json
{
  "characterId": "momose-hiyori",
  "systemPrompt": "",
  "avatar": {},
  "llm": {},
  "asr": {},
  "tts": {},
  "memory": {}
}
```

## 2. 顶层字段

### `characterId`

当前使用的角色 ID，对应 `characters.json` 中的 `id`。

### `systemPrompt`

系统提示词。优先级高于角色默认提示词。

如果为空，运行时会回退到当前角色在 `characters.json` 中的 `systemPrompt`。

## 3. `avatar`

```json
"avatar": {
  "scale": 1.0,
  "modelUrl": ""
}
```

字段说明：

- `scale`
  - Live2D 缩放系数。
- `modelUrl`
  - 可选覆盖项。
  - 为空时使用当前角色配置里的 `modelUrl`。

## 4. `llm`

```json
"llm": {
  "provider": "openai_compat",
  "endpoint": "https://api.deepseek.com/chat/completions",
  "model": "deepseek-chat",
  "apiKey": "",
  "temperature": 0.85,
  "maxTokens": 256,
  "contextLength": 4096
}
```

字段说明：

- `provider`
  - `ollama`
  - `openai_compat`
- `endpoint`
  - 完整请求地址，不是 base URL。
- `model`
  - 模型名。
- `apiKey`
  - OpenAI 兼容接口使用的 API Key。
- `temperature`
  - 采样温度。
- `maxTokens`
  - 单次回复最大 token。
- `contextLength`
  - 前端用于上下文长度控制的配置值。

## 5. `asr`

```json
"asr": {
  "engine": "zipformer_ctc",
  "modelDir": "models/asr/sherpa-onnx-streaming-zipformer-ctc-zh-int8-2025-06-30",
  "vadModel": "models/vad/silero_vad.int8.onnx",
  "language": "zh",
  "useItn": true,
  "sampleRate": 16000,
  "onnxProvider": "cpu",
  "numThreads": 1,
  "vadThreshold": 0.5,
  "vadMinSilenceDuration": 0.32,
  "vadMinSpeechDuration": 0.14,
  "vadMaxSpeechDuration": 18.0,
  "vadWindowSize": 512
}
```

字段说明：

- `engine`
  - 当前默认：`zipformer_ctc`
- `modelDir`
  - ASR 模型目录
- `vadModel`
  - VAD 模型文件路径
- `language`
  - 识别语言
- `useItn`
  - 是否启用逆文本规范化
- `sampleRate`
  - 输入采样率，当前固定推荐 `16000`
- `onnxProvider`
  - ONNX 执行提供方，默认 `cpu`
- `numThreads`
  - 推理线程数
- `vadThreshold`
  - VAD 阈值
- `vadMinSilenceDuration`
  - 静音时长阈值
- `vadMinSpeechDuration`
  - 最短语音时长
- `vadMaxSpeechDuration`
  - 最长语音时长
- `vadWindowSize`
  - VAD 窗口大小

## 6. `tts`

```json
"tts": {
  "engine": "sherpa_vits",
  "modelDir": "models/tts/sherpa-onnx-vits-zh-ll",
  "endpoint": "wss://dashscope.aliyuncs.com/api-ws/v1/realtime",
  "apiKey": "",
  "model": "qwen-tts-realtime-latest",
  "mode": "server_commit",
  "onnxProvider": "cpu",
  "numThreads": 1,
  "voice": "xiaoyou",
  "speakerId": 0,
  "language": "zh",
  "speed": 1.0,
  "format": "pcm_s16le",
  "stream": true
}
```

字段说明：

- `engine`
  - `sherpa_vits`
  - `qwen_realtime`
- `modelDir`
  - 本地 TTS 模型目录
  - `qwen_realtime` 下可为空
- `endpoint`
  - 在线 TTS 的 WebSocket 地址
- `apiKey`
  - 在线 TTS 的 API Key
- `model`
  - 在线 TTS 模型名
- `mode`
  - 在线 TTS 的提交模式
- `onnxProvider`
  - 本地 ONNX 执行提供方
- `numThreads`
  - 本地推理线程数
- `voice`
  - 音色名
- `speakerId`
  - 本地多说话人模型的 speaker id
- `language`
  - 语言标识
- `speed`
  - 语速
- `format`
  - 当前主要使用 `pcm_s16le`
- `stream`
  - 是否使用流式输出

## 7. `memory`

```json
"memory": {
  "enabled": true,
  "provider": "session_ephemeral",
  "maxItems": 12,
  "ttlHours": 24
}
```

字段说明：

- `enabled`
  - 是否启用记忆
- `provider`
  - 当前默认 `session_ephemeral`
- `maxItems`
  - 最多保留多少条记忆
- `ttlHours`
  - 单次启动内最长保留时长

## 8. `characters.json`

示例：

```json
{
  "id": "momose-hiyori",
  "name": "桃濑日和",
  "modelUrl": "/live2d/hiyori/hiyori_pro_t11.model3.json",
  "thumbUrl": "/assets/avatar-default.png",
  "systemPrompt": "",
  "motionMap": {}
}
```

字段说明：

- `id`
  - 角色唯一标识
- `name`
  - 设置页显示名称
- `modelUrl`
  - Live2D 模型入口
- `thumbUrl`
  - 默认头像
- `systemPrompt`
  - 角色默认系统提示词
- `motionMap`
  - 语义动作到实际 motion group 的映射

## 9. `motionMap`

示例：

```json
"motionMap": {
  "idle": ["Idle"],
  "chatOpen": ["Tap"],
  "chatClose": ["FlickDown"],
  "speakStart": ["Tap"],
  "happy": ["Tap", "Flick"],
  "shy": ["Flick", "FlickDown"]
}
```

说明：

- 键是语义动作名
- 值是 Live2D 模型里真实存在的 motion group 名称列表
- 运行时会按当前模型实际存在的动作组再做一次过滤

## 10. 修改配置后的生效方式

- 文本、LLM、ASR、TTS 配置在保存后即可参与后续请求
- Live2D 角色与模型地址变更后，建议执行一次头像重载
- 打包版和开发版可能读取不同配置根目录，部署时需确认实际使用的是哪份 `settings.json`
