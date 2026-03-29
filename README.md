# Fast Avatar AI

> 一款主打低延迟、快速启动、可本地部署的 Live2D 桌面陪伴助手，也可以把它理解成一套可扩展的 AI 女友 / AI 桌宠软件框架 💗

`Fast Avatar AI` 把 `Live2D`、`LLM`、`ASR`、`TTS` 和桌面常驻交互整合到了一起。  
它不是单纯的网页聊天窗口，而是一个可以常驻桌面、能看、能听、能说、能做动作、还能替换不同模型和不同角色形象的本地桌宠应用。

如果你的目标是做一套：

- 响应快 ⚡
- 启动快 🚀
- 本地可运行 🖥️
- 可切换模型 🧠
- 可更换 Live2D 形象 🎭
- 可继续扩展 ASR / TTS / 记忆系统 🧩

那这个仓库就是为这个方向设计的。

## ✨ 项目亮点

- 低延迟交互：前端、Rust 后端、本地 ASR/TTS 和 Live2D 渲染都在一套桌面应用里完成，减少外部依赖和中间跳转。
- 快速启动：Tauri + 本地静态资源方案，启动链路短，适合常驻桌面。
- 流式体验：支持流式对话、流式语音播放、语音输入与桌宠状态联动。
- 多模型适配：LLM、ASR、TTS、Live2D 都是可替换的，不是写死单一路线。
- 桌宠化交互：支持动作映射、空闲动作、语义动作、聊天窗口避让和口型驱动。
- 本地部署友好：适合打包成单个 Windows 桌面应用长期使用。

## 🖼️ 应用截图

### 初始化与桌宠形态

![初始化界面](./docs/images/init.jpg)

### 聊天与角色互动

![聊天界面](./docs/images/chat.jpg)

### 设置与模型切换

![设置界面](./docs/images/setting.jpg)

## 💡 这是一款怎样的应用

你可以把它理解成一套“本地桌面 AI 女友 / AI 陪伴助手”的基础框架。

它适合这些场景：

- 做一个有 Live2D 形象的桌面聊天助手
- 做一个有语音输入和语音播报的 AI 女友应用
- 做一个可以快速替换角色、提示词、ASR、TTS 的实验平台
- 做一个本地优先、兼顾在线模型能力的桌宠项目

当前仓库默认已经打通：

- Live2D 形象显示与切换
- 文本聊天与流式回复
- 本地 ASR：`zipformer_ctc + silero_vad`
- 本地 TTS：`sherpa_vits`
- 在线流式 TTS：`Qwen TTS Realtime`
- 动作映射、口型驱动、聊天层避让
- 会话级短期记忆
- 本地配置与角色配置管理

## 🧱 项目结构

```text
fast_avatar_ai/
  config/
    settings.json          # 主配置
    characters.json        # 角色配置与动作映射
  docs/
    configuration.md       # 配置说明
    asr-extension.md       # ASR 扩展文档
    tts-extension.md       # TTS 扩展文档
    live2d-integration.md  # Live2D 接入文档
    images/                # README 截图资源
  live2d/                  # Live2D 模型唯一真源
  models/
    asr/                   # 本地 ASR 模型
    tts/                   # 本地 TTS 模型
    vad/                   # VAD 模型
  resources/models/        # 打包时带入的模型资源
  src-tauri/               # Rust + Tauri 后端
  web/                     # 前端页面与 Live2D 运行时
```

当前约定：

- `live2d/` 是唯一 Live2D 资源真源
- `models/` 用于开发运行时加载本地模型
- `resources/models/` 用于打包分发模型
- `config/settings.json` 是主配置文件
- `config/characters.json` 是角色目录和动作映射配置

## 🧠 当前默认模型

当前仓库保留并默认使用这两套本地模型：

- ASR: `models/asr/sherpa-onnx-streaming-zipformer-ctc-zh-int8-2025-06-30`
- TTS: `models/tts/sherpa-onnx-vits-zh-ll`
- VAD: `models/vad/silero_vad.int8.onnx`

在线 TTS 可选：

- `Qwen TTS Realtime`
- 接入qwen-tts和音色选择参考：
  https://help.aliyun.com/zh/model-studio/qwen-tts-realtime?spm=a2c4g.11186623.help-menu-2400256.d_0_3_2_1.2dd36cc51WT90v

当前配置层支持：

- LLM: `ollama`、`openai_compat`
- ASR: `zipformer_ctc`
- TTS: `sherpa_vits`、`qwen_realtime`

## ⚙️ 运行环境

推荐环境：

- Windows 10/11
- 已安装 WebView2 Runtime
- Rust toolchain
- Node.js 18+

当前仓库主要以 Windows 桌面形态开发和验证。

## 🚀 快速开始

### 1. 安装前端依赖

```powershell
npm install
```

`postinstall` 会自动执行 `scripts/sync-assets.mjs`，同步前端需要的静态资源。

### 2. 开发运行

```powershell
cargo run --manifest-path src-tauri/Cargo.toml
```

如果本机上的 `sherpa-rs` 调试态不稳定，优先使用：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\run-release.ps1
```

或者：

```powershell
cargo run --release --manifest-path src-tauri/Cargo.toml
```

## 📦 构建与部署

### 生成可执行文件

```powershell
cargo build --release --manifest-path src-tauri/Cargo.toml
```

生成的可执行文件位于：

- `src-tauri/target/release/fast-avatar-ai.exe`

### 打包资源

当前 `src-tauri/tauri.conf.json` 已将以下资源纳入 bundle：

- `web`
- `live2d`
- `scripts`
- `resources/models`

如果你使用 Tauri 安装器构建链，可在安装 `cargo-tauri` 后执行：

```powershell
cargo tauri build --manifest-path src-tauri/Cargo.toml
```

### 模型资源同步

开发态读取：

- `models/`

打包态分发读取：

- `resources/models/`

如果你替换了本地模型，建议把对应资源同步到 `resources/models/`，这样打包后的应用也能带上同一套模型。

## 🔄 工作方式

整体链路如下：

1. 前端负责界面、Live2D 渲染、聊天框、麦克风采集和音频播放
2. Rust 后端负责配置、静态资源服务、LLM 代理、ASR/TTS 运行时和本地 API
3. ASR 通过 WebSocket 会话把音频块送到后端
4. LLM 生成文本回复
5. TTS 把文本合成为音频并流式返回前端播放
6. Live2D 根据动作映射、说话状态和语义动作更新表现

当前主要接口：

- `/api/health`
- `/api/tts`
- `/api/session/ws`
- `/live2d/*`

## 🛠️ 可扩展能力

这个项目不是只能跑一套固定模型。它本质上是一套可扩展桌宠框架。

你可以沿以下方向继续扩展：

- 扩展 ASR 引擎：见 [docs/asr-extension.md](./docs/asr-extension.md)
- 扩展 TTS 引擎：见 [docs/tts-extension.md](./docs/tts-extension.md)
- 接入新的 Live2D 模型：见 [docs/live2d-integration.md](./docs/live2d-integration.md)
- 查看完整配置结构：见 [docs/configuration.md](./docs/configuration.md)

适合扩展的内容包括：

- 新的本地 ASR 模型
- 新的本地 TTS 模型
- 在线流式 TTS 提供方
- 新的 Live2D 角色
- 角色动作映射与语义动作系统
- 更强的长期记忆或向量记忆

## 🎭 Live2D 与角色说明

当前支持：

- 标准 `model3.json` 模型加载
- 角色配置与默认头像
- `motionMap` 语义动作映射
- 聊天与语音状态驱动动作
- 普通模型统一口型驱动
- 基础 VTS companion 兼容

说明：

- `motionMap` 不是从模型自动推断出来的
- 它是“语义动作 -> 模型真实动作组”的映射
- 如果你想让不同角色有不同表现，重点调整的是 `characters.json`

## ❓常见问题

### 为什么这个项目适合做低延迟陪伴应用

因为它把：

- 界面
- Live2D 渲染
- 本地 ASR/TTS
- 聊天链路

尽量收敛到一套桌面应用里完成，减少中间服务和额外跳转，更适合做“桌面常驻、随时开口”的 AI 陪伴体验。

### 为什么它适合做 AI 女友 / AI 桌宠

因为它不仅有文本聊天，还包括：

- 角色系统
- 语音输入输出
- Live2D 动作
- 口型驱动
- 本地配置
- 多模型替换能力

这比一个普通网页聊天页更接近真正可用的桌面陪伴软件。

### 为什么某些 VTS 模型显示不完整

因为 VTS 模型包经常依赖：

- `*.vtube.json`
- `items_pinned_to_model.json`
- VTube Studio 自己的 item 资源

这些并不一定都属于标准 Cubism Web 运行时资源。当前项目对这类文件做了有限兼容，但不能保证与 VTube Studio 运行时完全一致。

### 为什么开发态和打包态表现不一致

常见原因有两个：

- 读取的配置目录不同
- `resources/models/` 没有同步最新模型

部署前建议确认：

- `config/settings.json`
- `resources/models/`
- `live2d/`

三者都已经同步到最新状态。
