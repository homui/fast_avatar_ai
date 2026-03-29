# TTS 扩展文档

本文说明如何在当前项目中扩展新的 TTS 提供方，包括本地 TTS 和在线流式 TTS。

## 1. 当前 TTS 结构

当前项目已经有两类 TTS：

- 本地 TTS：`sherpa_vits`
- 在线流式 TTS：`qwen_realtime`

相关代码：

- [src-tauri/src/speech.rs](/D:/software/fast_avatar_ai/src-tauri/src/speech.rs)
- [src-tauri/src/server.rs](/D:/software/fast_avatar_ai/src-tauri/src/server.rs)
- [web/main.js](/D:/software/fast_avatar_ai/web/main.js)

## 2. 当前默认模型目录

```text
models/tts/
  sherpa-onnx-vits-zh-ll/
    model.onnx
    tokens.txt
    lexicon.txt
    dict/
```

## 3. 两类扩展方式

### 3.1 扩展本地 sherpa TTS

适合：

- 离线部署
- 一个 exe 内自带本地模型
- 低依赖

落点：

- [src-tauri/src/speech.rs](/D:/software/fast_avatar_ai/src-tauri/src/speech.rs)

### 3.2 扩展在线 TTS API

适合：

- 想用更高质量音色
- 需要云端流式能力
- 想接入商业语音服务

落点：

- [src-tauri/src/server.rs](/D:/software/fast_avatar_ai/src-tauri/src/server.rs)

当前 `qwen_realtime` 就是这种实现方式。

## 4. 新增本地 TTS 引擎

### 4.1 配置层

在 [src-tauri/src/config.rs](/D:/software/fast_avatar_ai/src-tauri/src/config.rs) 中：

- 新增默认模型目录常量
- 视情况调整 `TtsSettings::default()`

### 4.2 运行时层

在 [src-tauri/src/speech.rs](/D:/software/fast_avatar_ai/src-tauri/src/speech.rs) 中：

1. 扩展 `TtsEngine` 枚举
2. 在 `build_tts_engine()` 中增加新的 `settings.engine` 分支
3. 在合成分支里把结果转换为统一的音频输出格式

当前已有参考分支：

- `sherpa_vits`
- `sherpa_kokoro`
- `sherpa_matcha`

## 5. 新增在线 TTS 提供方

推荐方式是仿照 `qwen_realtime` 新增一个在线分支。

入口在：

- [src-tauri/src/server.rs](/D:/software/fast_avatar_ai/src-tauri/src/server.rs) 的 `tts_proxy()`

扩展步骤：

1. 在 `tts_proxy()` 中识别新的 `settings.engine`
2. 单独写一个代理函数，例如：
   - `tts_proxy_<provider>()`
3. 输出统一的响应头：
   - `x-audio-format`
   - `x-sample-rate`
   - `x-channels`
4. 前端继续复用现有播放器

## 6. 前端如何增加新 TTS 选项

需要改两处：

1. [web/main.js](/D:/software/fast_avatar_ai/web/main.js) 的 `TTS_ENGINE_PRESETS`
2. [web/index.html](/D:/software/fast_avatar_ai/web/index.html) 的 TTS 下拉框

如果是在线 TTS，通常还需要在设置页保留：

- `endpoint`
- `apiKey`
- `model`
- `mode`
- `voice`

## 7. 本地 TTS 模型目录建议

建议每个模型独立目录：

```text
models/tts/<model-name>/
  model.onnx
  tokens.txt
  lexicon.txt
  dict/
```

如果是多说话人模型，还要额外考虑：

- `speakerId`
- `voice`

## 8. 打包同步

本地 TTS 模型要同步到：

```text
resources/models/tts/<model-name>/
```

在线 TTS 不需要本地模型目录，但需要配置：

- `endpoint`
- `apiKey`
- `model`
- `voice`

## 9. 流式输入与输出说明

### 本地 `sherpa_vits`

当前是：

- 文本整段提交
- 音频以流式响应方式回前端播放

### `qwen_realtime`

当前是：

- WebSocket 输入
- 服务端流式返回音频块
- 前端边收边播

如果你要扩展新的在线 TTS，建议优先保持这种流式结构，而不是先整段缓存再返回。

## 10. 验证建议

至少验证：

```powershell
cargo check --manifest-path src-tauri/Cargo.toml
node --check web/main.js
```

联调时检查：

- 设置页能否保存新引擎配置
- `/api/tts` 是否正确分流到新引擎
- 前端是否能播出返回音频
- 是否存在首字或尾字截断
