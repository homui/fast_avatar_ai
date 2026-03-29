# Live2D 接入文档

本文说明如何把新的 Live2D 模型接入当前项目，以及当前项目对动作系统和 VTS companion 的支持范围。

模型可从以下获取：
https://www.live2d.com/zh-CHS/learn/sample/

## 1. 当前接入约定

当前项目只保留一份 Live2D 资源目录：

- [live2d](/D:/software/fast_avatar_ai/live2d)

不再维护 `web/live2d` 副本。

运行时静态资源路由：

- `/live2d/*`

## 2. 标准模型接入方式

推荐接入的是标准 `model3.json` 模型包。

典型结构：

```text
live2d/
  <角色目录>/
    xxx.model3.json
    xxx.moc3
    physics3.json
    cdi3.json
    texture_00.png
    ...
```

然后在 [config/characters.json](/D:/software/fast_avatar_ai/config/characters.json) 中增加一项：

```json
{
  "id": "new-character",
  "name": "新角色",
  "modelUrl": "/live2d/new-character/xxx.model3.json",
  "thumbUrl": "/live2d/new-character/texture_00.png",
  "systemPrompt": "",
  "motionMap": {}
}
```

## 3. 通过导入按钮接入

当前项目支持导入一个 `.model3.json` 文件。

导入时后端会：

1. 复制模型所在目录到 `live2d/`
2. 生成标准的 `modelUrl`
3. 尝试推断默认缩略图
4. 自动把角色写入 `characters.json`

相关入口：

- [src-tauri/src/config.rs](/D:/software/fast_avatar_ai/src-tauri/src/config.rs) 的 `import_character_model()`

## 4. 动作系统怎么工作

当前动作系统不是直接在业务代码里到处写 `Tap`、`Idle`，而是走语义动作层。

### 4.1 `motionMap`

每个角色都可以配置：

```json
"motionMap": {
  "idle": ["Idle"],
  "chatOpen": ["Tap"],
  "chatClose": ["FlickDown"],
  "speakStart": ["Tap"],
  "happy": ["Tap", "Flick"]
}
```

### 4.2 语义动作

当前语义动作包括：

- `idle`
- `chatOpen`
- `chatClose`
- `think`
- `speakStart`
- `reply`
- `happy`
- `gentle`
- `playful`
- `shy`
- `bodyReact`

### 4.3 实际触发场景

当前项目会在这些场景触发动作：

- 打开聊天
- 关闭聊天
- 思考中
- 开始播报
- 空闲随机动作
- 点击模型
- 回复文本结束后的语义动作推断
- 聊天框中的动作命令，例如 `/motion Tap`

## 5. `motionMap` 不是自动推断出来的

模型文件只会告诉运行时：

- 当前模型有哪些 motion group

语义到动作的映射仍然需要你自己配置在 `characters.json` 里。

运行时只会：

1. 读取角色 `motionMap`
2. 用默认映射补全缺失项
3. 再按当前模型真正存在的动作组做过滤

## 6. 口型驱动

当前普通模型已经接入统一的 `ParamMouthOpenY` 驱动。

工作方式：

- TTS 播放时，前端通过 WebAudio analyser 采样音量包络
- 再把音量映射到普通模型的 `ParamMouthOpenY`
- 对 VTS companion 模型则同时兼容 `ParameterSettings` 里的 `MouthOpen`

## 7. VTS 模型兼容范围

当前项目对 VTS 模型做了有限兼容，主要包括：

- 识别 `*.vtube.json`
- 识别 `items_pinned_to_model.json`
- 读取 `ParameterSettings`
- 对缺失的 pinned item 做降级忽略

但要注意：

- 这不是 VTube Studio 运行时本体
- 不能保证与 VTS 内效果完全一致
- 如果 VTS item 资源本身不在模型包里，当前项目无法凭空还原

## 8. 为什么 Live2D 官方样例通常没问题

因为 Live2D 官方样例通常是标准 Cubism 模型包，本来就是给 Web SDK 用的。

而 VTS 模型往往依赖：

- `.vtube.json`
- scene item
- pinned item
- VTS 自己的参数输入

所以后者的兼容成本更高。

## 9. 接入新模型的推荐流程

推荐顺序：

1. 先确认模型能独立访问 `xxx.model3.json`
2. 把模型目录放进 `live2d/`
3. 在 `characters.json` 增加角色项
4. 先不写 `motionMap`，确认基础显示正常
5. 再根据模型真实动作组补 `motionMap`
6. 最后再调默认缩放、弹层位置和口型表现

## 10. 调试建议

如果某个模型显示异常，优先检查：

1. `modelUrl` 是否能正常访问
2. 贴图、`moc3`、`physics3.json` 是否都在同目录
3. `model3.json` 中的 motion group 名称
4. 是否带有 `*.vtube.json`
5. 是否引用了缺失的 pinned item

如果是动作问题，可直接在聊天框中测试：

- `/motions`
- `/motion Tap`
- `/motion FlickDown`

用来确认当前模型到底加载到了哪些动作组。
