use std::{
  ffi::{CStr, CString},
  fs,
  mem,
  path::{Path, PathBuf},
  ptr::null,
  sync::Mutex,
};

use anyhow::{anyhow, bail, Context, Result};
use sherpa_rs::{
  sense_voice::{SenseVoiceConfig, SenseVoiceRecognizer},
  silero_vad::{SileroVad, SileroVadConfig},
  tts::{
    CommonTtsConfig, KokoroTts, KokoroTtsConfig, MatchaTts, MatchaTtsConfig, TtsAudio, VitsTts,
    VitsTtsConfig,
  },
  OnnxConfig,
};

use crate::config::{AsrSettings, TtsSettings};

pub struct SpeechRuntime {
  data_root: PathBuf,
  asr_cache: Mutex<Option<CachedAsrEngine>>,
  tts_cache: Mutex<Option<CachedTtsEngine>>,
}

struct CachedAsrEngine {
  signature: String,
  engine: AsrEngine,
}

struct CachedTtsEngine {
  signature: String,
  engine: TtsEngine,
}

enum AsrEngine {
  SenseVoice(SenseVoiceRecognizer),
  ZipformerCtc(ZipformerCtcRecognizer),
}

enum TtsEngine {
  Vits(VitsTts),
  Kokoro(KokoroTts),
  Matcha(MatchaTts),
}

pub struct SpeechRender {
  pub bytes: Vec<u8>,
  pub sample_rate: u32,
  pub channels: u32,
  pub format: String,
}

pub enum AsrEvent {
  SpeechStart,
  SpeechEnd,
  TranscriptFinal(String),
}

pub struct AsrSession<'a> {
  runtime: &'a SpeechRuntime,
  settings: AsrSettings,
  vad: SileroVad,
  is_speaking: bool,
}

impl SpeechRuntime {
  pub fn new(data_root: PathBuf) -> Self {
    Self {
      data_root,
      asr_cache: Mutex::new(None),
      tts_cache: Mutex::new(None),
    }
  }

  pub fn create_asr_session(&self, settings: &AsrSettings) -> Result<AsrSession<'_>> {
    let vad_model = self.resolve_input_path(&settings.vad_model);
    let vad = SileroVad::new(
      SileroVadConfig {
        model: path_text(&vad_model),
        min_silence_duration: settings.vad_min_silence_duration,
        min_speech_duration: settings.vad_min_speech_duration,
        max_speech_duration: settings.vad_max_speech_duration,
        threshold: settings.vad_threshold,
        sample_rate: settings.sample_rate,
        window_size: settings.vad_window_size,
        provider: Some(settings.onnx_provider.clone()),
        num_threads: Some(settings.num_threads),
        debug: false,
      },
      24.0,
    )
    .map_err(|error| anyhow!("failed to initialize VAD model at {}: {error}", vad_model.display()))?;

    Ok(AsrSession {
      runtime: self,
      settings: settings.clone(),
      vad,
      is_speaking: false,
    })
  }

  pub fn synthesize(&self, settings: &TtsSettings, text: &str) -> Result<SpeechRender> {
    let signature = format!(
      "{}|{}|{}|{}|{}|{}|{}|{}",
      settings.engine,
      settings.model_dir,
      settings.onnx_provider,
      settings.num_threads,
      settings.speaker_id,
      settings.language,
      settings.voice,
      settings.speed
    );

    let mut cache = self.tts_cache.lock().map_err(|_| anyhow!("tts cache poisoned"))?;
    if cache
      .as_ref()
      .map(|cached| cached.signature.as_str() != signature.as_str())
      .unwrap_or(true)
    {
      *cache = Some(CachedTtsEngine {
        signature,
        engine: self.build_tts_engine(settings)?,
      });
    }

    let audio = match cache.as_mut().expect("tts cache just initialized").engine {
      TtsEngine::Vits(ref mut engine) => engine
        .create(text, settings.speaker_id, settings.speed)
        .map_err(|error| anyhow!("failed to synthesize VITS audio: {error}"))?,
      TtsEngine::Kokoro(ref mut engine) => engine
        .create(text, settings.speaker_id, settings.speed)
        .map_err(|error| anyhow!("failed to synthesize Kokoro audio: {error}"))?,
      TtsEngine::Matcha(ref mut engine) => engine
        .create(text, settings.speaker_id, settings.speed)
        .map_err(|error| anyhow!("failed to synthesize Matcha audio: {error}"))?,
    };

    Ok(render_tts_audio(audio))
  }

  pub fn warmup(&self, asr_settings: &AsrSettings, tts_settings: &TtsSettings) -> Result<()> {
    // Initialize a VAD instance once so the first live session does not pay model startup cost.
    let _ = self.create_asr_session(asr_settings)?;

    // Prime the ASR execution path with a short silence buffer.
    let warmup_samples = vec![0.0_f32; (asr_settings.sample_rate / 5).max(512) as usize];
    let _ = self.transcribe(asr_settings, &warmup_samples)?;

    // Prime TTS model loading and the first synthesis graph execution.
    let warmup_text = if tts_settings.language.trim().eq_ignore_ascii_case("en") {
      "hello"
    } else {
      "你好"
    };
    let _ = self.synthesize(tts_settings, warmup_text)?;

    Ok(())
  }

  fn transcribe(&self, settings: &AsrSettings, samples: &[f32]) -> Result<String> {
    let signature = format!(
      "{}|{}|{}|{}|{}|{}",
      settings.engine,
      settings.model_dir,
      settings.onnx_provider,
      settings.num_threads,
      settings.sample_rate,
      settings.language
    );

    let mut cache = self.asr_cache.lock().map_err(|_| anyhow!("asr cache poisoned"))?;
    if cache
      .as_ref()
      .map(|cached| cached.signature.as_str() != signature.as_str())
      .unwrap_or(true)
    {
      *cache = Some(CachedAsrEngine {
        signature,
        engine: self.build_asr_engine(settings)?,
      });
    }

    let text = match cache.as_mut().expect("asr cache just initialized").engine {
      AsrEngine::SenseVoice(ref mut recognizer) => recognizer
        .transcribe(settings.sample_rate, samples)
        .text,
      AsrEngine::ZipformerCtc(ref recognizer) => recognizer.transcribe(settings.sample_rate, samples)?,
    };

    Ok(text.trim().to_string())
  }

  fn build_asr_engine(&self, settings: &AsrSettings) -> Result<AsrEngine> {
    match settings.engine.trim() {
      "sense_voice" => {
        let model_dir = self.resolve_input_path(&settings.model_dir);
        let model = find_first_file(&model_dir, &["model.int8.onnx", "model.onnx"])?;
        let tokens = find_first_file(&model_dir, &["tokens.txt"])?;
        let recognizer = SenseVoiceRecognizer::new(SenseVoiceConfig {
          model: path_text(&model),
          language: settings.language.clone(),
          use_itn: settings.use_itn,
          provider: Some(settings.onnx_provider.clone()),
          num_threads: Some(settings.num_threads),
          debug: false,
          tokens: path_text(&tokens),
        })
        .map_err(|error| anyhow!("failed to initialize SenseVoice model at {}: {error}", model_dir.display()))?;
        Ok(AsrEngine::SenseVoice(recognizer))
      }
      "zipformer_ctc" => {
        let model_dir = self.resolve_input_path(&settings.model_dir);
        let model = find_first_file(&model_dir, &["model.int8.onnx", "model.onnx"])?;
        let tokens = find_first_file(&model_dir, &["tokens.txt"])?;
        let recognizer = ZipformerCtcRecognizer::new(
          &model,
          &tokens,
          &settings.onnx_provider,
          settings.num_threads,
          settings.sample_rate,
        )
        .map_err(|error| {
          anyhow!(
            "failed to initialize streaming Zipformer CTC model at {}: {error}",
            model_dir.display()
          )
        })?;
        Ok(AsrEngine::ZipformerCtc(recognizer))
      }
      engine => bail!("unsupported ASR engine: {engine}"),
    }
  }

  fn build_tts_engine(&self, settings: &TtsSettings) -> Result<TtsEngine> {
    let model_dir = self.resolve_input_path(&settings.model_dir);
    match settings.engine.trim() {
      "sherpa_vits" => {
        let model = find_first_file(&model_dir, &["model.onnx", "model.int8.onnx"])?;
        let tokens = find_first_file(&model_dir, &["tokens.txt"])?;
        let lexicon = find_first_file(&model_dir, &["lexicon.txt"])?;
        let dict_dir = find_first_dir(&model_dir, &["dict", "dict_dir"])?;
        let data_dir = find_first_dir_optional(&model_dir, &["espeak-ng-data", "data"])
          .unwrap_or_else(String::new);
        let engine = VitsTts::new(VitsTtsConfig {
          model: path_text(&model),
          lexicon: path_text(&lexicon),
          dict_dir: path_text(&dict_dir),
          tokens: path_text(&tokens),
          data_dir,
          length_scale: 1.0,
          noise_scale: 0.667,
          noise_scale_w: 0.8,
          silence_scale: 0.2,
          onnx_config: build_onnx_config(settings.onnx_provider.clone(), settings.num_threads),
          tts_config: Default::default(),
        });
        Ok(TtsEngine::Vits(engine))
      }
      "sherpa_kokoro" => {
        let model = find_first_file(&model_dir, &["model.onnx", "model.int8.onnx"])?;
        let voices = find_first_file(&model_dir, &["voices.bin", "voices-v1.0.bin"])?;
        let tokens = find_first_file(&model_dir, &["tokens.txt"])?;
        let lexicon = find_first_file(&model_dir, &["lexicon.txt"])?;
        let dict_dir = find_first_dir(&model_dir, &["dict", "dict_dir"])?;
        let data_dir = find_first_dir(&model_dir, &["espeak-ng-data", "data"])?;
        let engine = KokoroTts::new(KokoroTtsConfig {
          model: path_text(&model),
          voices: path_text(&voices),
          tokens: path_text(&tokens),
          data_dir: path_text(&data_dir),
          dict_dir: path_text(&dict_dir),
          lexicon: path_text(&lexicon),
          length_scale: 1.0,
          onnx_config: build_onnx_config(settings.onnx_provider.clone(), settings.num_threads),
          common_config: Default::default(),
          lang: settings.language.clone(),
        });
        Ok(TtsEngine::Kokoro(engine))
      }
      "sherpa_matcha" => {
        let acoustic_model = find_first_file(
          &model_dir,
          &[
            "model-steps-3.onnx",
            "model-steps-2.onnx",
            "model-steps-4.onnx",
            "model-steps-5.onnx",
            "model-steps-6.onnx",
            "model.onnx",
            "model.int8.onnx",
          ],
        )?;
        let tokens = find_first_file(&model_dir, &["tokens.txt"])?;
        let lexicon = find_first_file(&model_dir, &["lexicon.txt"])?;
        let dict_dir = find_first_dir_optional(&model_dir, &["dict", "dict_dir"]).unwrap_or_else(String::new);
        let data_dir = find_first_dir_optional(&model_dir, &["espeak-ng-data", "data"]).unwrap_or_else(String::new);
        let vocoder = find_matcha_vocoder(&model_dir)?;
        let rule_fsts = collect_existing_files(
          &model_dir,
          &[
            "phone.fst",
            "date.fst",
            "number.fst",
            "phone-zh.fst",
            "date-zh.fst",
            "number-zh.fst",
          ],
        )
          .into_iter()
          .map(|path| path_text(&path))
          .collect::<Vec<_>>()
          .join(",");
        let engine = MatchaTts::new(MatchaTtsConfig {
          model: String::new(),
          lexicon: path_text(&lexicon),
          dict_dir,
          tokens: path_text(&tokens),
          data_dir,
          acoustic_model: path_text(&acoustic_model),
          vocoder: path_text(&vocoder),
          length_scale: 1.0,
          noise_scale: 0.667,
          noise_scale_w: 0.8,
          silence_scale: 0.2,
          common_config: CommonTtsConfig {
            rule_fars: String::new(),
            rule_fsts,
            max_num_sentences: 1,
            silence_scale: 0.2,
          },
          onnx_config: build_onnx_config(settings.onnx_provider.clone(), settings.num_threads),
        });
        Ok(TtsEngine::Matcha(engine))
      }
      engine => bail!("unsupported TTS engine: {engine}"),
    }
  }

  fn resolve_input_path(&self, value: &str) -> PathBuf {
    let normalized = value.replace('\\', "/");
    let path = PathBuf::from(&normalized);
    if path.is_absolute() {
      path
    } else {
      self.data_root.join(path)
    }
  }

}

impl AsrSession<'_> {
  pub fn accept_samples(&mut self, samples: Vec<f32>) -> Result<Vec<AsrEvent>> {
    self.vad.accept_waveform(samples);
    let mut events = Vec::new();
    if self.vad.is_speech() && !self.is_speaking {
      self.is_speaking = true;
      events.push(AsrEvent::SpeechStart);
    }
    self.collect_completed_segments(&mut events)?;
    Ok(events)
  }

  pub fn flush(&mut self) -> Result<Vec<AsrEvent>> {
    self.vad.flush();
    let mut events = Vec::new();
    self.collect_completed_segments(&mut events)?;
    if self.is_speaking {
      self.is_speaking = false;
      events.push(AsrEvent::SpeechEnd);
    }
    Ok(events)
  }

  fn collect_completed_segments(&mut self, events: &mut Vec<AsrEvent>) -> Result<()> {
    while !self.vad.is_empty() {
      let segment = self.vad.front();
      self.vad.pop();
      if self.is_speaking {
        self.is_speaking = false;
        events.push(AsrEvent::SpeechEnd);
      }
      let transcript = self.runtime.transcribe(&self.settings, &segment.samples)?;
      if !transcript.is_empty() {
        events.push(AsrEvent::TranscriptFinal(transcript));
      }
    }
    Ok(())
  }
}

struct ZipformerCtcRecognizer {
  recognizer: *const sherpa_rs_sys::SherpaOnnxOnlineRecognizer,
}

impl ZipformerCtcRecognizer {
  fn new(
    model: &Path,
    tokens: &Path,
    provider: &str,
    num_threads: i32,
    sample_rate: u32,
  ) -> Result<Self> {
    let model = cstring_from_path(model)?;
    let tokens = cstring_from_path(tokens)?;
    let provider = CString::new(provider).context("invalid ONNX provider")?;
    let decoding_method = CString::new("greedy_search").expect("static string");
    let modeling_unit = CString::new("cjkchar").expect("static string");

    let config = sherpa_rs_sys::SherpaOnnxOnlineRecognizerConfig {
      feat_config: sherpa_rs_sys::SherpaOnnxFeatureConfig {
        sample_rate: sample_rate as i32,
        feature_dim: 80,
      },
      model_config: sherpa_rs_sys::SherpaOnnxOnlineModelConfig {
        transducer: unsafe { mem::zeroed() },
        paraformer: unsafe { mem::zeroed() },
        zipformer2_ctc: sherpa_rs_sys::SherpaOnnxOnlineZipformer2CtcModelConfig {
          model: model.as_ptr(),
        },
        tokens: tokens.as_ptr(),
        num_threads,
        provider: provider.as_ptr(),
        debug: 0,
        model_type: null(),
        modeling_unit: modeling_unit.as_ptr(),
        bpe_vocab: null(),
        tokens_buf: null(),
        tokens_buf_size: 0,
        nemo_ctc: unsafe { mem::zeroed() },
      },
      decoding_method: decoding_method.as_ptr(),
      max_active_paths: 4,
      enable_endpoint: 0,
      rule1_min_trailing_silence: 0.0,
      rule2_min_trailing_silence: 0.0,
      rule3_min_utterance_length: 0.0,
      hotwords_file: null(),
      hotwords_score: 1.5,
      ctc_fst_decoder_config: sherpa_rs_sys::SherpaOnnxOnlineCtcFstDecoderConfig {
        graph: null(),
        max_active: 0,
      },
      rule_fsts: null(),
      rule_fars: null(),
      blank_penalty: 0.0,
      hotwords_buf: null(),
      hotwords_buf_size: 0,
      hr: unsafe { mem::zeroed() },
    };

    let recognizer = unsafe { sherpa_rs_sys::SherpaOnnxCreateOnlineRecognizer(&config) };
    if recognizer.is_null() {
      bail!("sherpa-onnx failed to create online recognizer");
    }

    Ok(Self { recognizer })
  }

  fn transcribe(&self, input_sample_rate: u32, samples: &[f32]) -> Result<String> {
    if samples.is_empty() {
      return Ok(String::new());
    }

    let stream = unsafe { sherpa_rs_sys::SherpaOnnxCreateOnlineStream(self.recognizer) };
    if stream.is_null() {
      bail!("sherpa-onnx failed to create online stream");
    }

    unsafe {
      sherpa_rs_sys::SherpaOnnxOnlineStreamAcceptWaveform(
        stream,
        input_sample_rate as i32,
        samples.as_ptr(),
        samples.len() as i32,
      );
      let tail_samples = vec![0.0_f32; ((input_sample_rate as usize) * 3 / 10).max(1)];
      sherpa_rs_sys::SherpaOnnxOnlineStreamAcceptWaveform(
        stream,
        input_sample_rate as i32,
        tail_samples.as_ptr(),
        tail_samples.len() as i32,
      );
      sherpa_rs_sys::SherpaOnnxOnlineStreamInputFinished(stream);
      while sherpa_rs_sys::SherpaOnnxIsOnlineStreamReady(self.recognizer, stream) != 0 {
        sherpa_rs_sys::SherpaOnnxDecodeOnlineStream(self.recognizer, stream);
      }

      let result = sherpa_rs_sys::SherpaOnnxGetOnlineStreamResult(self.recognizer, stream);
      if result.is_null() {
        sherpa_rs_sys::SherpaOnnxDestroyOnlineStream(stream);
        bail!("sherpa-onnx returned a null online ASR result");
      }

      let text = if (*result).text.is_null() {
        String::new()
      } else {
        CStr::from_ptr((*result).text).to_string_lossy().into_owned()
      };

      sherpa_rs_sys::SherpaOnnxDestroyOnlineRecognizerResult(result);
      sherpa_rs_sys::SherpaOnnxDestroyOnlineStream(stream);
      Ok(text.trim().to_string())
    }
  }
}

unsafe impl Send for ZipformerCtcRecognizer {}
unsafe impl Sync for ZipformerCtcRecognizer {}

impl Drop for ZipformerCtcRecognizer {
  fn drop(&mut self) {
    unsafe {
      sherpa_rs_sys::SherpaOnnxDestroyOnlineRecognizer(self.recognizer);
    }
  }
}

fn build_onnx_config(provider: String, num_threads: i32) -> OnnxConfig {
  OnnxConfig {
    provider,
    debug: false,
    num_threads,
  }
}

fn render_tts_audio(audio: TtsAudio) -> SpeechRender {
  let bytes = audio
    .samples
    .into_iter()
    .flat_map(|sample| {
      let value = (sample * i16::MAX as f32)
        .clamp(i16::MIN as f32, i16::MAX as f32) as i16;
      value.to_le_bytes()
    })
    .collect::<Vec<_>>();

  SpeechRender {
    bytes,
    sample_rate: audio.sample_rate,
    channels: 1,
    format: "pcm_s16le".into(),
  }
}

fn cstring_from_path(path: &Path) -> Result<CString> {
  CString::new(path_text(path)).map_err(|_| anyhow!("path contains an unexpected NUL byte: {}", path.display()))
}

fn path_text(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

fn find_first_file(root: &Path, names: &[&str]) -> Result<PathBuf> {
  for name in names {
    let candidate = root.join(name);
    if candidate.is_file() {
      return Ok(candidate);
    }
  }

  let entries = fs::read_dir(root)
    .with_context(|| format!("failed to inspect directory {}", root.display()))?;
  for entry in entries.flatten() {
    let path = entry.path();
    if !path.is_file() {
      continue;
    }
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
      continue;
    };
    if names.iter().any(|name| file_name.eq_ignore_ascii_case(name)) {
      return Ok(path);
    }
  }

  bail!(
    "required file missing in {} (expected one of: {})",
    root.display(),
    names.join(", ")
  )
}

fn find_first_dir(root: &Path, names: &[&str]) -> Result<PathBuf> {
  for name in names {
    let candidate = root.join(name);
    if candidate.is_dir() {
      return Ok(candidate);
    }
  }

  let entries = fs::read_dir(root)
    .with_context(|| format!("failed to inspect directory {}", root.display()))?;
  for entry in entries.flatten() {
    let path = entry.path();
    if !path.is_dir() {
      continue;
    }
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
      continue;
    };
    if names.iter().any(|name| file_name.eq_ignore_ascii_case(name)) {
      return Ok(path);
    }
  }

  bail!(
    "required directory missing in {} (expected one of: {})",
    root.display(),
    names.join(", ")
  )
}

fn find_first_file_optional(root: &Path, names: &[&str]) -> Option<PathBuf> {
  for name in names {
    let candidate = root.join(name);
    if candidate.is_file() {
      return Some(candidate);
    }
  }

  let entries = fs::read_dir(root).ok()?;
  for entry in entries.flatten() {
    let path = entry.path();
    if !path.is_file() {
      continue;
    }
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
      continue;
    };
    if names.iter().any(|name| file_name.eq_ignore_ascii_case(name)) {
      return Some(path);
    }
  }

  None
}

fn collect_existing_files(root: &Path, names: &[&str]) -> Vec<PathBuf> {
  names
    .iter()
    .filter_map(|name| find_first_file_optional(root, &[*name]))
    .collect()
}

fn find_matcha_vocoder(model_dir: &Path) -> Result<PathBuf> {
  const VOCODER_NAMES: &[&str] = &[
    "vocos-22khz-univ.onnx",
    "vocos-16khz-univ.onnx",
    "hifigan_v1.onnx",
    "hifigan_v2.onnx",
    "hifigan_v3.onnx",
    "vocoder.onnx",
  ];

  if let Some(path) = find_first_file_optional(model_dir, VOCODER_NAMES) {
    return Ok(path);
  }

  if let Some(parent) = model_dir.parent() {
    if let Some(path) = find_first_file_optional(parent, VOCODER_NAMES) {
      return Ok(path);
    }
  }

  bail!(
    "matcha TTS requires a separate vocoder model. Download vocos-22khz-univ.onnx (or hifigan_v1/v2/v3.onnx) and place it in {} or its parent directory",
    model_dir.display()
  )
}

fn find_first_dir_optional(root: &Path, names: &[&str]) -> Option<String> {
  for name in names {
    let candidate = root.join(name);
    if candidate.is_dir() {
      return Some(path_text(&candidate));
    }
  }

  let entries = fs::read_dir(root).ok()?;
  for entry in entries.flatten() {
    let path = entry.path();
    if !path.is_dir() {
      continue;
    }
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
      continue;
    };
    if names.iter().any(|name| file_name.eq_ignore_ascii_case(name)) {
      return Some(path_text(&path));
    }
  }

  None
}
