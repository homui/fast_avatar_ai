# ASR 扩展文档

本文说明如何在当前项目中扩展新的 ASR 引擎。

## 1. 当前 ASR 链路

当前默认链路是：

- VAD: `silero_vad.int8.onnx`
- ASR: `zipformer_ctc`
- 前端通过 `/api/session/ws` 持续上传音频块
- 后端基于 VAD 分句后返回最终识别结果

相关代码：

- [src-tauri/src/speech.rs](/D:/software/fast_avatar_ai/src-tauri/src/speech.rs)
- [src-tauri/src/server.rs](/D:/software/fast_avatar_ai/src-tauri/src/server.rs)
- [web/main.js](/D:/software/fast_avatar_ai/web/main.js)

## 2. 当前默认模型目录

```text
models/
  asr/
    sherpa-onnx-streaming-zipformer-ctc-zh-int8-2025-06-30/
      model.int8.onnx
      tokens.txt
  vad/
    silero_vad.int8.onnx
```

## 3. 新增一个 ASR 引擎需要改哪些地方

最少要改 4 处：

1. `src-tauri/src/config.rs`
2. `src-tauri/src/speech.rs`
3. `web/main.js`
4. `web/index.html`

## 4. 后端扩展步骤

### 4.1 增加配置默认值

在 [src-tauri/src/config.rs](/D:/software/fast_avatar_ai/src-tauri/src/config.rs) 中：

- 增加新的默认模型目录常量
- 如有需要，调整 `AsrSettings::default()`

### 4.2 在 `speech.rs` 里增加引擎分支

当前构建入口在：

- `build_asr_engine()`

你需要：

1. 扩展 `AsrEngine` 枚举
2. 在 `build_asr_engine()` 中匹配新的 `settings.engine`
3. 在识别分支里实现 `transcribe()`

当前代码里已存在的例子：

- `sense_voice`
- `zipformer_ctc`

## 5. 前端扩展步骤

### 5.1 增加预设

在 [web/main.js](/D:/software/fast_avatar_ai/web/main.js) 的 `ASR_ENGINE_PRESETS` 中增加新项。

### 5.2 增加设置选项

在 [web/index.html](/D:/software/fast_avatar_ai/web/index.html) 中给 ASR 下拉框增加新的 `<option>`。

### 5.3 设置读取与保存

当前设置读取逻辑已经是通用结构，通常只要：

- 新引擎名称和预设一致
- `modelDir`、`language` 等字段保留

就不需要额外修改提交流程。

## 6. 推荐的目录规范

建议一个 ASR 引擎一个目录：

```text
models/asr/<engine-model-name>/
  model.int8.onnx
  tokens.txt
```

如果模型还依赖其它资源，也一并放进同目录。

## 7. 打包同步

如果新引擎要进入打包版，请同步放到：

```text
resources/models/asr/<engine-model-name>/
```

否则打包后的 exe 可能能启动，但找不到模型文件。

## 8. 推荐扩展顺序

推荐顺序：

1. 先在 `speech.rs` 跑通本地识别
2. 再补设置页下拉和预设
3. 最后同步到 `resources/models/`

## 9. 验证方式

至少验证：

```powershell
cargo check --manifest-path src-tauri/Cargo.toml
node --check web/main.js
```

联调时检查：

- 模型是否加载成功
- 麦克风能否采集
- VAD 是否能正确分句
- 最终文本是否能回填到输入框
