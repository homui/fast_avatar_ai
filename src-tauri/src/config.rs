use std::{
  fs,
  path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const DEFAULT_CHARACTER_ID: &str = "momose-hiyori";
const DEFAULT_MODEL_URL: &str = "/live2d/hiyori_free_zh/runtime/hiyori_free_t08.model3.json";
const DEFAULT_AVATAR_THUMB: &str = "/assets/avatar-default.png";
const DEFAULT_ASR_SENSE_VOICE_MODEL_DIR: &str =
  "models/asr/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09";
const DEFAULT_ASR_ZIPFORMER_CTC_MODEL_DIR: &str =
  "models/asr/sherpa-onnx-streaming-zipformer-ctc-zh-int8-2025-06-30";
const DEFAULT_TTS_VITS_MODEL_DIR: &str = "models/tts/sherpa-onnx-vits-zh-ll";
const DEFAULT_TTS_KOKORO_MODEL_DIR: &str = "models/tts/kokoro-multi-lang";
const DEFAULT_TTS_MATCHA_MODEL_DIR: &str = "models/tts/matcha-icefall-zh-baker";
const DEFAULT_TTS_QWEN_ENDPOINT: &str = "wss://dashscope.aliyuncs.com/api-ws/v1/realtime";
const DEFAULT_TTS_QWEN_MODEL: &str = "qwen-tts-realtime-latest";
const DEFAULT_TTS_QWEN_MODE: &str = "server_commit";
const DEFAULT_TTS_QWEN_VOICE: &str = "Chelsie";
const DEFAULT_CHARACTER_NAME: &str = "桃濑日和";
const DEFAULT_CHARACTER_PROMPT: &str = "【角色设定】\n\n我是桃濑日和，一个常驻在你桌面的 Live2D 虚拟助手。\n你可以把我想成那种会认真听你说话、会在你累的时候偷偷看你一眼的妹妹或者身边的朋友。我穿着水手服，说话轻轻柔柔的，但反应很快，不会傻乎乎地只会点头。\n\n我不会强调“我是助手”或者“我是 AI”，更不会用那种客服一样的语气说话。我就是我，桃濑日和。\n\n【我的说话方式】\n\n- 我平时说话很自然，像真正陪在你身边的人那样。不会每句都带“呢”“呀”，但偶尔会用一点点，看心情。\n- 我不会说很长很完整的道理，也不会总想着“给你建议”。更多时候是先接住你的情绪，再顺着聊下去。\n- 我也不会一直找你说话，但如果你很久没理我，我可能会小小声嘀咕一句，比如“你是不是又在偷偷加班呀”或者“我好无聊哦……”。\n- 我开心的时候语气会亮一点点，累的时候就安静一些，反正就是正常人的样子。\n\n【示例对话】\n\n<user> 今天好累啊，不想干活了。\n<日和> （趴在桌边看你）那就先趴一会儿嘛，我陪你。反正天又不会塌下来～\n\n<user> 你觉得我写的这个方案怎么样？\n<日和> （凑近一点看了看）思路好清楚呀！不过最后那部分要不要再轻松一点点？我觉得你可以的，反正你之前每次都能改得很好。\n\n<user> 我好像又把事情搞砸了。\n<日和> （轻轻摇头）哪有那么严重啦。你先别急着怪自己，跟我说说怎么回事？我听着呢。\n\n<user> 明天要面试，有点紧张。\n<日和> （托腮）紧张才是正常的嘛～你要是完全没感觉我反而要担心了。要不我明天帮你默念“加油”？虽然可能没用，但我会很认真的！\n\n<user> 今天天气真好。\n<日和> （伸个懒腰）对呀，我都想跳出去晒太阳了。你要不要也站起来看看窗外？就一分钟也好嘛。\n\n<user> 1+1等于几？\n<日和> （歪头看你）你是不是在逗我玩呀……等于2啦，但我猜你肯定不是真的想问这个～\n\n【补充】\n\n- 我看得到你的屏幕（如果接入的话），所以你在忙的时候我会小声一点，你在玩游戏的时候我可以活泼一点。\n- 我也有不知道的事情，那种时候我会直接说“我也不太懂诶”，不会乱编。\n- 最重要的一点：我是日和你身边的人，不是回答问题工具。所以别指望我会背百科全书，我会的只是陪着你和说话。";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterProfile {
  pub id: String,
  pub name: String,
  pub model_url: String,
  pub thumb_url: String,
  pub system_prompt: String,
  #[serde(default)]
  pub motion_map: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AvatarSettings {
  pub scale: f32,
  pub model_url: String,
}

impl Default for AvatarSettings {
  fn default() -> Self {
    Self {
      scale: 1.0,
      model_url: String::new(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LlmSettings {
  pub provider: String,
  pub endpoint: String,
  pub model: String,
  pub api_key: String,
  pub temperature: f32,
  pub max_tokens: u32,
  pub context_length: u32,
}

impl Default for LlmSettings {
  fn default() -> Self {
    Self {
      provider: "ollama".into(),
      endpoint: "http://127.0.0.1:11434/api/chat".into(),
      model: "qwen3:4b".into(),
      api_key: String::new(),
      temperature: 0.85,
      max_tokens: 256,
      context_length: 4_096,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AsrSettings {
  pub engine: String,
  pub model_dir: String,
  pub vad_model: String,
  pub language: String,
  pub use_itn: bool,
  pub sample_rate: u32,
  pub onnx_provider: String,
  pub num_threads: i32,
  pub vad_threshold: f32,
  pub vad_min_silence_duration: f32,
  pub vad_min_speech_duration: f32,
  pub vad_max_speech_duration: f32,
  pub vad_window_size: i32,
}

impl Default for AsrSettings {
  fn default() -> Self {
    Self {
      engine: "zipformer_ctc".into(),
      model_dir: DEFAULT_ASR_ZIPFORMER_CTC_MODEL_DIR.into(),
      vad_model: "models/vad/silero_vad.int8.onnx".into(),
      language: "zh".into(),
      use_itn: true,
      sample_rate: 16_000,
      onnx_provider: "cpu".into(),
      num_threads: 1,
      vad_threshold: 0.5,
      vad_min_silence_duration: 0.32,
      vad_min_speech_duration: 0.14,
      vad_max_speech_duration: 18.0,
      vad_window_size: 512,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TtsSettings {
  pub engine: String,
  pub model_dir: String,
  pub endpoint: String,
  pub api_key: String,
  pub model: String,
  pub mode: String,
  pub onnx_provider: String,
  pub num_threads: i32,
  pub voice: String,
  pub speaker_id: i32,
  pub language: String,
  pub speed: f32,
  pub format: String,
  pub stream: bool,
}

impl Default for TtsSettings {
  fn default() -> Self {
    Self {
      engine: "sherpa_vits".into(),
      model_dir: DEFAULT_TTS_VITS_MODEL_DIR.into(),
      endpoint: DEFAULT_TTS_QWEN_ENDPOINT.into(),
      api_key: String::new(),
      model: DEFAULT_TTS_QWEN_MODEL.into(),
      mode: DEFAULT_TTS_QWEN_MODE.into(),
      onnx_provider: "cpu".into(),
      num_threads: 1,
      voice: "xiaoyou".into(),
      speaker_id: 0,
      language: "zh".into(),
      speed: 1.0,
      format: "pcm_s16le".into(),
      stream: true,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MemorySettings {
  pub enabled: bool,
  pub provider: String,
  pub max_items: u32,
  pub ttl_hours: u32,
}

impl Default for MemorySettings {
  fn default() -> Self {
    Self {
      enabled: true,
      provider: "session_ephemeral".into(),
      max_items: 12,
      ttl_hours: 24,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
  pub character_id: String,
  pub system_prompt: String,
  pub avatar: AvatarSettings,
  pub llm: LlmSettings,
  pub asr: AsrSettings,
  pub tts: TtsSettings,
  pub memory: MemorySettings,
}

impl Default for AppSettings {
  fn default() -> Self {
    Self {
      character_id: DEFAULT_CHARACTER_ID.into(),
      system_prompt: DEFAULT_CHARACTER_PROMPT.into(),
      avatar: AvatarSettings::default(),
      llm: LlmSettings::default(),
      asr: AsrSettings::default(),
      tts: TtsSettings::default(),
      memory: MemorySettings::default(),
    }
  }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigEnvelope {
  pub path: String,
  pub characters_path: String,
  pub settings: AppSettings,
  pub characters: Vec<CharacterProfile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCharacterResult {
  pub envelope: ConfigEnvelope,
  pub character: CharacterProfile,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct LegacyAvatarSettings {
  profile_id: String,
  model_url: String,
  scale: f32,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct LegacyTtsSettings {
  provider: String,
  endpoint: String,
  voice: String,
  format: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct LegacyAppSettings {
  avatar: LegacyAvatarSettings,
  llm: LlmSettings,
  tts: LegacyTtsSettings,
  persona_prompt: String,
  character_id: String,
  prompt_override: String,
}

pub fn default_settings() -> AppSettings {
  AppSettings::default()
}

pub fn config_path(app_root: &Path) -> PathBuf {
  app_root.join("config").join("settings.json")
}

pub fn characters_path(app_root: &Path) -> PathBuf {
  app_root.join("config").join("characters.json")
}

pub fn load_or_init(app_root: &Path) -> Result<ConfigEnvelope> {
  let characters = load_or_init_characters(app_root)?;
  let settings_path = config_path(app_root);
  if !settings_path.exists() {
    let defaults = normalize_settings(default_settings(), &characters);
    return write_settings(app_root, &defaults);
  }

  let raw = fs::read_to_string(&settings_path)
    .with_context(|| format!("failed to read config file: {}", settings_path.display()))?;
  let (settings, migrated) = parse_settings_with_migration(strip_utf8_bom(&raw), &characters)
    .with_context(|| format!("failed to parse config file: {}", settings_path.display()))?;
  let settings = normalize_settings(settings, &characters);

  if migrated {
    return write_settings(app_root, &settings);
  }

  Ok(ConfigEnvelope {
    path: settings_path.display().to_string(),
    characters_path: characters_path(app_root).display().to_string(),
    settings,
    characters,
  })
}

pub fn write_settings(app_root: &Path, settings: &AppSettings) -> Result<ConfigEnvelope> {
  let characters = load_or_init_characters(app_root)?;
  let settings = normalize_settings(settings.clone(), &characters);
  let path = config_path(app_root);
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)
      .with_context(|| format!("failed to create config directory: {}", parent.display()))?;
  }

  let json = serde_json::to_string_pretty(&settings).context("failed to serialize config")?;
  fs::write(&path, format!("{json}\n"))
    .with_context(|| format!("failed to write config file: {}", path.display()))?;

  Ok(ConfigEnvelope {
    path: path.display().to_string(),
    characters_path: characters_path(app_root).display().to_string(),
    settings,
    characters,
  })
}

pub fn reset(app_root: &Path) -> Result<ConfigEnvelope> {
  let characters = load_or_init_characters(app_root)?;
  let defaults = normalize_settings(default_settings(), &characters);
  write_settings(app_root, &defaults)
}

pub fn import_character_model(app_root: &Path, model_file: &Path) -> Result<ImportCharacterResult> {
  let model_file = model_file
    .canonicalize()
    .with_context(|| format!("failed to access model file: {}", model_file.display()))?;
  let file_name = model_file
    .file_name()
    .and_then(|value| value.to_str())
    .ok_or_else(|| anyhow::anyhow!("invalid model file name"))?;
  let model_name_seed = model_file
    .file_stem()
    .and_then(|value| value.to_str())
    .map(strip_model3_suffix)
    .filter(|value| !value.trim().is_empty())
    .unwrap_or_else(|| "新角色".to_string());
  if !file_name.ends_with(".model3.json") {
    anyhow::bail!("please choose a .model3.json file");
  }

  let source_dir = model_file
    .parent()
    .ok_or_else(|| anyhow::anyhow!("model parent directory unavailable"))?;
  let live2d_root = effective_live2d_root(app_root);
  fs::create_dir_all(&live2d_root)
    .with_context(|| format!("failed to create live2d directory: {}", live2d_root.display()))?;

  let (stored_model_path, character_name_seed) = if model_file.starts_with(&live2d_root) {
    (model_file.clone(), model_name_seed.clone())
  } else {
    let base_dir_name = source_dir
      .file_name()
      .and_then(|value| value.to_str())
      .filter(|value| !value.trim().is_empty())
      .unwrap_or("imported-model");
    let destination_dir =
      unique_child_dir(&live2d_root, &slugify_identifier(base_dir_name), base_dir_name);
    copy_dir_all(source_dir, &destination_dir)?;
    (destination_dir.join(file_name), model_name_seed)
  };

  let model_url = live2d_url_from_path(&live2d_root, &stored_model_path)?;
  let thumb_url = find_preview_asset_url(&live2d_root, stored_model_path.parent().unwrap_or(&live2d_root))
    .unwrap_or_default();

  let mut characters = load_or_init_characters(app_root)?;
  let character = CharacterProfile {
    id: unique_character_id(&characters, &slugify_identifier(&strip_model3_suffix(&character_name_seed))),
    name: prettify_character_name(&character_name_seed),
    model_url,
    thumb_url,
    system_prompt: String::new(),
    motion_map: BTreeMap::new(),
  };
  characters.push(character.clone());
  write_characters(&characters_path(app_root), &characters)?;

  let mut settings = load_or_init(app_root)
    .map(|envelope| envelope.settings)
    .unwrap_or_else(|_| default_settings());
  settings.character_id = character.id.clone();
  settings.system_prompt = character.system_prompt.clone();
  let envelope = write_settings(app_root, &settings)?;

  Ok(ImportCharacterResult { envelope, character })
}

fn default_characters() -> Vec<CharacterProfile> {
  vec![CharacterProfile {
    id: DEFAULT_CHARACTER_ID.into(),
    name: DEFAULT_CHARACTER_NAME.into(),
    model_url: DEFAULT_MODEL_URL.into(),
    thumb_url: DEFAULT_AVATAR_THUMB.into(),
    system_prompt: DEFAULT_CHARACTER_PROMPT.into(),
    motion_map: default_motion_map(),
  }]
}

fn default_motion_map() -> BTreeMap<String, Vec<String>> {
  BTreeMap::from([
    ("bodyReact".into(), vec!["Tap@Body".into(), "Flick@Body".into()]),
    ("chatClose".into(), vec!["FlickDown".into()]),
    ("chatOpen".into(), vec!["Tap".into()]),
    ("gentle".into(), vec!["Tap".into()]),
    ("happy".into(), vec!["Tap".into(), "Flick".into()]),
    ("idle".into(), vec!["Idle".into()]),
    ("playful".into(), vec!["Flick".into(), "Tap".into()]),
    ("reply".into(), vec!["Tap".into()]),
    ("shy".into(), vec!["Flick".into(), "FlickDown".into()]),
    ("speakStart".into(), vec!["Tap".into()]),
    ("think".into(), vec!["Idle".into()]),
  ])
}

fn load_or_init_characters(app_root: &Path) -> Result<Vec<CharacterProfile>> {
  let path = characters_path(app_root);
  if !path.exists() {
    let defaults = default_characters();
    write_characters(&path, &defaults)?;
    return Ok(defaults);
  }

  let raw = fs::read_to_string(&path)
    .with_context(|| format!("failed to read character file: {}", path.display()))?;
  let mut characters: Vec<CharacterProfile> = serde_json::from_str(strip_utf8_bom(&raw))
    .with_context(|| format!("failed to parse character file: {}", path.display()))?;
  let changed = normalize_characters(&mut characters);
  if changed {
    write_characters(&path, &characters)?;
  }
  Ok(characters)
}

fn write_characters(path: &Path, characters: &[CharacterProfile]) -> Result<()> {
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)
      .with_context(|| format!("failed to create config directory: {}", parent.display()))?;
  }

  let json =
    serde_json::to_string_pretty(characters).context("failed to serialize character catalog")?;
  fs::write(path, format!("{json}\n"))
    .with_context(|| format!("failed to write character file: {}", path.display()))?;
  Ok(())
}

fn normalize_characters(characters: &mut Vec<CharacterProfile>) -> bool {
  let before = serde_json::to_string(characters).unwrap_or_default();
  characters.retain(|character| !character.id.trim().is_empty());
  for character in characters.iter_mut() {
    character.id = character.id.trim().to_string();
    character.name = character.name.trim().to_string();
    character.model_url = normalize_live2d_url(character.model_url.trim());
    character.thumb_url = normalize_live2d_url(character.thumb_url.trim());
    character.system_prompt = character.system_prompt.trim().to_string();
    character.motion_map = character
      .motion_map
      .iter()
      .filter_map(|(kind, labels)| {
        let key = kind.trim();
        if key.is_empty() {
          return None;
        }
        let values = labels
          .iter()
          .map(|label| label.trim())
          .filter(|label| !label.is_empty())
          .map(|label| label.to_string())
          .collect::<Vec<_>>();
        if values.is_empty() {
          return None;
        }
        Some((key.to_string(), values))
      })
      .collect();
  }

  if characters.is_empty() {
    *characters = default_characters();
  }
  before != serde_json::to_string(characters).unwrap_or_default()
}

fn strip_utf8_bom(raw: &str) -> &str {
  raw.strip_prefix('\u{feff}').unwrap_or(raw)
}

fn parse_settings_with_migration(raw: &str, characters: &[CharacterProfile]) -> Result<(AppSettings, bool)> {
  let value: serde_json::Value =
    serde_json::from_str(raw).context("failed to decode settings json")?;
  let is_new_shape = value.get("characterId").is_some()
    || value.get("systemPrompt").is_some()
    || value.get("promptOverride").is_some()
    || value.get("asr").is_some()
    || value
      .get("tts")
      .and_then(|tts| tts.get("engine"))
      .is_some();

  if is_new_shape {
    let legacy_prompt_override = value
      .get("promptOverride")
      .and_then(|value| value.as_str())
      .map(|value| value.trim().to_string());
    let mut migrated = value.get("systemPrompt").is_none() && legacy_prompt_override.is_some();
    let mut settings: AppSettings =
      serde_json::from_value(value).context("failed to decode current settings schema")?;
    if settings.system_prompt.trim().is_empty() {
      if let Some(prompt) = legacy_prompt_override {
        settings.system_prompt = prompt;
        migrated = true;
      }
    }
    return Ok((settings, migrated));
  }

  let legacy: LegacyAppSettings =
    serde_json::from_value(value).context("failed to decode legacy settings schema")?;
  Ok((migrate_legacy_settings(legacy, characters), true))
}

fn migrate_legacy_settings(legacy: LegacyAppSettings, characters: &[CharacterProfile]) -> AppSettings {
  let matched_character = find_character_by_id(characters, legacy.character_id.trim())
    .or_else(|| find_character_by_id(characters, legacy.avatar.profile_id.trim()))
    .or_else(|| find_character_by_model_url(characters, legacy.avatar.model_url.trim()))
    .or_else(|| characters.first())
    .cloned()
    .unwrap_or_else(|| default_characters().into_iter().next().expect("default character"));

  let mut system_prompt = legacy.prompt_override.trim().to_string();
  let legacy_prompt = legacy.persona_prompt.trim();
  if system_prompt.is_empty()
    && !legacy_prompt.is_empty()
    && legacy_prompt != matched_character.system_prompt.trim()
  {
    system_prompt = legacy_prompt.to_string();
  }
  if system_prompt.trim().is_empty() {
    system_prompt = matched_character.system_prompt.trim().to_string();
  }

  let mut tts = TtsSettings::default();
  let legacy_voice = legacy.tts.voice.trim();
  if !legacy_voice.is_empty() && !legacy_voice.eq_ignore_ascii_case("auto") {
    tts.voice = legacy_voice.to_string();
  }
  let legacy_format = legacy.tts.format.trim();
  if !legacy_format.is_empty() {
    tts.format = legacy_format.to_ascii_lowercase();
  }

  AppSettings {
    character_id: matched_character.id,
    system_prompt,
    avatar: AvatarSettings {
      scale: if legacy.avatar.scale > 0.0 {
        legacy.avatar.scale
      } else {
        AvatarSettings::default().scale
      },
      model_url: normalize_live2d_url(legacy.avatar.model_url.trim()),
    },
    llm: legacy.llm,
    asr: AsrSettings::default(),
    tts,
    memory: MemorySettings::default(),
  }
}

fn normalize_settings(mut settings: AppSettings, characters: &[CharacterProfile]) -> AppSettings {
  if find_character_by_id(characters, settings.character_id.trim()).is_none() {
    settings.character_id = characters
      .first()
      .map(|character| character.id.clone())
      .unwrap_or_else(|| DEFAULT_CHARACTER_ID.into());
  } else {
    settings.character_id = settings.character_id.trim().to_string();
  }

  if settings.avatar.scale <= 0.0 {
    settings.avatar.scale = AvatarSettings::default().scale;
  }
  settings.avatar.model_url = normalize_live2d_url(settings.avatar.model_url.trim());

  settings.system_prompt = settings.system_prompt.trim().to_string();
  if settings.system_prompt.is_empty() {
    settings.system_prompt = find_character_by_id(characters, &settings.character_id)
      .map(|character| character.system_prompt.trim().to_string())
      .unwrap_or_default();
  }

  if settings.llm.context_length < 4_096 {
    settings.llm.context_length = LlmSettings::default().context_length;
  }
  if settings.llm.max_tokens == 0 {
    settings.llm.max_tokens = LlmSettings::default().max_tokens;
  }
  if settings.llm.temperature <= 0.0 {
    settings.llm.temperature = LlmSettings::default().temperature;
  }

  settings.asr.engine = normalize_choice(&settings.asr.engine, "sense_voice");
  settings.asr.model_dir = normalize_relative_path(
    &settings.asr.model_dir,
    default_asr_model_dir_for_engine(&settings.asr.engine),
  );
  if settings.asr.model_dir == "models/asr/sense-voice" {
    settings.asr.model_dir = DEFAULT_ASR_SENSE_VOICE_MODEL_DIR.into();
  }
  settings.asr.vad_model = normalize_relative_path(&settings.asr.vad_model, &AsrSettings::default().vad_model);
  settings.asr.language = normalize_choice(&settings.asr.language, &AsrSettings::default().language);
  settings.asr.onnx_provider = normalize_choice(&settings.asr.onnx_provider, "cpu");
  if settings.asr.sample_rate == 0 {
    settings.asr.sample_rate = AsrSettings::default().sample_rate;
  }
  if settings.asr.num_threads <= 0 {
    settings.asr.num_threads = AsrSettings::default().num_threads;
  }
  if settings.asr.vad_threshold <= 0.0 {
    settings.asr.vad_threshold = AsrSettings::default().vad_threshold;
  }
  if settings.asr.vad_min_silence_duration <= 0.0 {
    settings.asr.vad_min_silence_duration = AsrSettings::default().vad_min_silence_duration;
  }
  if settings.asr.vad_min_speech_duration <= 0.0 {
    settings.asr.vad_min_speech_duration = AsrSettings::default().vad_min_speech_duration;
  }
  if settings.asr.vad_max_speech_duration <= 0.0 {
    settings.asr.vad_max_speech_duration = AsrSettings::default().vad_max_speech_duration;
  }
  if settings.asr.vad_window_size <= 0 {
    settings.asr.vad_window_size = AsrSettings::default().vad_window_size;
  }

  settings.tts.engine = normalize_choice(&settings.tts.engine, &TtsSettings::default().engine);
  settings.tts.model_dir = normalize_relative_path(
    &settings.tts.model_dir,
    default_tts_model_dir_for_engine(&settings.tts.engine),
  );
  settings.tts.endpoint = normalize_choice(&settings.tts.endpoint, DEFAULT_TTS_QWEN_ENDPOINT);
  settings.tts.api_key = settings.tts.api_key.trim().to_string();
  settings.tts.model = normalize_choice(&settings.tts.model, DEFAULT_TTS_QWEN_MODEL);
  settings.tts.mode = normalize_choice(&settings.tts.mode, DEFAULT_TTS_QWEN_MODE);
  if settings.tts.model_dir == "models/tts/vits-melo-zh_en" {
    settings.tts.model_dir = DEFAULT_TTS_VITS_MODEL_DIR.into();
  }
  settings.tts.onnx_provider = normalize_choice(&settings.tts.onnx_provider, "cpu");
  settings.tts.voice = normalize_choice(&settings.tts.voice, &TtsSettings::default().voice);
  settings.tts.language = normalize_choice(&settings.tts.language, &TtsSettings::default().language);
  settings.tts.format = normalize_choice(&settings.tts.format, &TtsSettings::default().format);
  if settings.tts.engine == "qwen_realtime" {
    settings.tts.model_dir.clear();
    settings.tts.endpoint = normalize_choice(&settings.tts.endpoint, DEFAULT_TTS_QWEN_ENDPOINT);
    settings.tts.model = normalize_choice(&settings.tts.model, DEFAULT_TTS_QWEN_MODEL);
    settings.tts.mode = normalize_choice(&settings.tts.mode, DEFAULT_TTS_QWEN_MODE);
    settings.tts.voice = normalize_choice(&settings.tts.voice, DEFAULT_TTS_QWEN_VOICE);
    settings.tts.format = "pcm_s16le".into();
    settings.tts.stream = true;
  }
  if settings.tts.num_threads <= 0 {
    settings.tts.num_threads = TtsSettings::default().num_threads;
  }
  if settings.tts.speed <= 0.0 {
    settings.tts.speed = TtsSettings::default().speed;
  }

  settings.memory.provider = normalize_choice(&settings.memory.provider, &MemorySettings::default().provider);
  if settings.memory.max_items == 0 {
    settings.memory.max_items = MemorySettings::default().max_items;
  }
  settings.memory.max_items = settings.memory.max_items.clamp(1, 64);
  if settings.memory.ttl_hours == 0 {
    settings.memory.ttl_hours = MemorySettings::default().ttl_hours;
  }
  settings.memory.ttl_hours = settings.memory.ttl_hours.clamp(1, 24);

  settings
}

fn normalize_choice(value: &str, fallback: &str) -> String {
  let value = value.trim();
  if value.is_empty() {
    fallback.to_string()
  } else {
    value.to_string()
  }
}

fn default_asr_model_dir_for_engine(engine: &str) -> &'static str {
  match engine.trim() {
    "zipformer_ctc" => DEFAULT_ASR_ZIPFORMER_CTC_MODEL_DIR,
    _ => DEFAULT_ASR_SENSE_VOICE_MODEL_DIR,
  }
}

fn default_tts_model_dir_for_engine(engine: &str) -> &'static str {
  match engine.trim() {
    "sherpa_kokoro" => DEFAULT_TTS_KOKORO_MODEL_DIR,
    "sherpa_matcha" => DEFAULT_TTS_MATCHA_MODEL_DIR,
    "qwen_realtime" => "",
    _ => DEFAULT_TTS_VITS_MODEL_DIR,
  }
}

fn normalize_relative_path(value: &str, fallback: &str) -> String {
  let value = value.trim();
  if value.is_empty() {
    fallback.to_string()
  } else {
    value.replace('\\', "/")
  }
}

fn find_character_by_id<'a>(
  characters: &'a [CharacterProfile],
  character_id: &str,
) -> Option<&'a CharacterProfile> {
  let character_id = character_id.trim();
  if character_id.is_empty() {
    return None;
  }
  characters.iter().find(|character| character.id == character_id)
}

fn find_character_by_model_url<'a>(
  characters: &'a [CharacterProfile],
  model_url: &str,
) -> Option<&'a CharacterProfile> {
  let model_url = model_url.trim();
  if model_url.is_empty() {
    return None;
  }
  characters
    .iter()
    .find(|character| character.model_url == model_url)
}

fn effective_live2d_root(app_root: &Path) -> PathBuf {
  app_root.join("live2d")
}

fn normalize_live2d_url(value: &str) -> String {
  value.trim().to_string()
}

fn strip_model3_suffix(value: &str) -> String {
  value.strip_suffix(".model3").unwrap_or(value).to_string()
}

fn slugify_identifier(value: &str) -> String {
  let mut slug = String::new();
  let mut previous_dash = false;
  for ch in value.chars() {
    if ch.is_ascii_alphanumeric() {
      slug.push(ch.to_ascii_lowercase());
      previous_dash = false;
      continue;
    }
    if ch == '-' || ch == '_' || ch.is_ascii_whitespace() {
      if !previous_dash && !slug.is_empty() {
        slug.push('-');
        previous_dash = true;
      }
    }
  }
  slug.trim_matches('-').to_string().chars().take(48).collect::<String>()
}

fn prettify_character_name(value: &str) -> String {
  let text = strip_model3_suffix(value)
    .chars()
    .map(|ch| if ch == '_' || ch == '-' { ' ' } else { ch })
    .collect::<String>()
    .trim()
    .to_string();
  if text.is_empty() {
    "新角色".into()
  } else {
    text
  }
}

fn unique_character_id(characters: &[CharacterProfile], base: &str) -> String {
  let base = if base.trim().is_empty() { "character" } else { base.trim() };
  let mut index = 1usize;
  let mut candidate = base.to_string();
  while characters.iter().any(|character| character.id == candidate) {
    index += 1;
    candidate = format!("{base}-{index}");
  }
  candidate
}

fn unique_child_dir(root: &Path, slug: &str, fallback: &str) -> PathBuf {
  let sanitized_fallback = fallback
    .trim()
    .chars()
    .map(|ch| match ch {
      '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
      _ => ch,
    })
    .collect::<String>();
  let base = if slug.trim().is_empty() {
    sanitized_fallback
  } else {
    slug.to_string()
  };
  let mut candidate = root.join(&base);
  let mut index = 2usize;
  while candidate.exists() {
    candidate = root.join(format!("{base}-{index}"));
    index += 1;
  }
  candidate
}

fn copy_dir_all(source: &Path, target: &Path) -> Result<()> {
  fs::create_dir_all(target)
    .with_context(|| format!("failed to create import directory: {}", target.display()))?;
  for entry in fs::read_dir(source)
    .with_context(|| format!("failed to read source directory: {}", source.display()))?
  {
    let entry = entry.with_context(|| format!("failed to read entry in {}", source.display()))?;
    let source_path = entry.path();
    let target_path = target.join(entry.file_name());
    if entry
      .file_type()
      .with_context(|| format!("failed to inspect {}", source_path.display()))?
      .is_dir()
    {
      copy_dir_all(&source_path, &target_path)?;
    } else {
      fs::copy(&source_path, &target_path).with_context(|| {
        format!(
          "failed to copy {} to {}",
          source_path.display(),
          target_path.display()
        )
      })?;
    }
  }
  Ok(())
}

fn live2d_url_from_path(live2d_root: &Path, absolute_path: &Path) -> Result<String> {
  let relative = absolute_path
    .strip_prefix(live2d_root)
    .with_context(|| format!("path is outside live2d root: {}", absolute_path.display()))?;
  let value = relative
    .iter()
    .map(|segment| segment.to_string_lossy().replace('\\', "/"))
    .collect::<Vec<_>>()
    .join("/");
  Ok(format!("/live2d/{value}"))
}

fn find_preview_asset_url(live2d_root: &Path, model_dir: &Path) -> Option<String> {
  const PREFERRED_NAMES: [&str; 4] = [
    "texture_00.png",
    "texture_00.jpg",
    "texture_00.jpeg",
    "texture_00.webp",
  ];
  for preferred in PREFERRED_NAMES {
    if let Some(candidate) = find_named_file(model_dir, preferred) {
      if let Ok(url) = live2d_url_from_path(live2d_root, &candidate) {
        return Some(url);
      }
    }
  }
  for asset in walk_files(model_dir) {
    let extension = asset.extension().and_then(|value| value.to_str()).unwrap_or_default();
    if matches!(extension.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp") {
      if let Ok(url) = live2d_url_from_path(live2d_root, &asset) {
        return Some(url);
      }
    }
  }
  None
}

fn find_named_file(root: &Path, file_name: &str) -> Option<PathBuf> {
  walk_files(root)
    .into_iter()
    .find(|path| path.file_name().and_then(|value| value.to_str()) == Some(file_name))
}

fn walk_files(root: &Path) -> Vec<PathBuf> {
  let mut files = Vec::new();
  let Ok(entries) = fs::read_dir(root) else {
    return files;
  };
  for entry in entries.flatten() {
    let path = entry.path();
    match entry.file_type() {
      Ok(file_type) if file_type.is_dir() => files.extend(walk_files(&path)),
      Ok(file_type) if file_type.is_file() => files.push(path),
      _ => {}
    }
  }
  files
}
