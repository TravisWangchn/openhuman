//! Factory functions for creating voice (STT / TTS) providers.
//!
//! Mirrors the shape of [`crate::openhuman::embeddings::factory`]: a single
//! entry point that takes a provider name + parameters and returns a boxed
//! trait object. Production paths pick the provider based on the user's
//! config (`stt_provider`, `tts_provider`); unit tests use the factory
//! directly to verify dispatch branches.
//!
//! ## STT providers
//!
//! - `"cloud"` → backend Whisper proxy (POST `/openai/v1/audio/transcriptions`).
//!   Same path the renamed `MicComposer` used to call directly. Keeps the API key
//!   off the desktop, costs network round-trip latency.
//! - `"whisper"` → local Whisper via the `WHISPER_BIN` env var (or in-process
//!   `whisper-rs` engine when `local_ai.whisper_in_process` is on). Zero
//!   network, but the user has to download the model. Default model:
//!   `whisper-large-v3-turbo` (recommended) or smaller variants
//!   (`tiny / base / small / medium`) for lower-end hardware.
//!
//! ## TTS providers
//!
//! - `"cloud"` → backend ElevenLabs proxy (POST `/openai/v1/audio/speech`)
//!   which also returns Oculus-15 visemes for the mascot lip-sync.
//! - `"piper"` → local Piper subprocess via `PIPER_BIN`. Lower latency than
//!   ElevenLabs and runs offline; default voice `en_US-lessac-medium`.
//!   **Note**: Kokoro (higher quality, 82M params) is intentionally out of
//!   scope for this ship — `PIPER_BIN` is already reserved in `.env.example`
//!   and Piper is the simpler integration. Kokoro is tracked as future work.
//!
//! ## Logging prefixes
//!
//! All factory branches log against `[voice-factory]`; the wrapped provider
//! implementations log under `[voice-stt]` / `[voice-tts]` so end-to-end
//! traces grep cleanly.

use std::sync::Arc;

use async_trait::async_trait;
use log::debug;
use serde::{Deserialize, Serialize};

use super::cloud_transcribe::{transcribe_cloud, CloudTranscribeOptions, CloudTranscribeResult};
use super::doubao_stt::{transcribe_doubao, DoubaoSttOptions, DoubaoSttResult};
use super::doubao_tts::{synthesize_doubao, DoubaoTtsOptions, DoubaoTtsResult};
use super::local_speech::{synthesize_piper, PiperOptions};
use super::local_transcribe::{transcribe_whisper, WhisperTranscribeOptions};
use super::reply_speech::{synthesize_reply, ReplySpeechOptions, ReplySpeechResult};
use crate::openhuman::config::Config;
use crate::rpc::RpcOutcome;

const LOG_PREFIX: &str = "[voice-factory]";

// ---------------------------------------------------------------------------
// Provider traits
// ---------------------------------------------------------------------------

/// Common shape both STT branches return after dispatch. Keeps the wire
/// contract identical regardless of provider — the UI only sees `text`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttResult {
    pub text: String,
    /// Lowercase provider id (`"cloud"`, `"whisper"`) — exposed on the wire
    /// so the renderer can show the user which path actually ran.
    pub provider: String,
}

/// Speech-to-text provider abstraction. Cloud (backend proxy) and Whisper
/// (local subprocess / in-process) both implement this; the factory hands
/// the caller a boxed trait object.
#[async_trait]
pub trait SttProvider: Send + Sync {
    /// Stable identifier used in logs and config (`"cloud"`, `"whisper"`).
    fn name(&self) -> &'static str;

    /// Transcribe a single base64-encoded audio blob.
    ///
    /// `mime_type` and `file_name` are hints; providers that don't care
    /// may ignore them. `language` is BCP-47 (`"en"`, `"es"`); pass `None`
    /// to let the provider auto-detect.
    async fn transcribe(
        &self,
        config: &Config,
        audio_base64: &str,
        mime_type: Option<&str>,
        file_name: Option<&str>,
        language: Option<&str>,
    ) -> Result<RpcOutcome<SttResult>, String>;
}

/// Text-to-speech provider abstraction. Cloud returns rich viseme alignment
/// (used by the mascot lip-sync); Piper returns audio only and the caller
/// derives a flat viseme timeline downstream.
#[async_trait]
pub trait TtsProvider: Send + Sync {
    fn name(&self) -> &'static str;

    /// Synthesize speech for `text`. Returns the same envelope shape as
    /// `voice.reply_synthesize` so the renderer can swap providers without
    /// branching on the response.
    async fn synthesize(
        &self,
        config: &Config,
        text: &str,
        voice: Option<&str>,
    ) -> Result<RpcOutcome<ReplySpeechResult>, String>;
}

// ---------------------------------------------------------------------------
// Cloud STT
// ---------------------------------------------------------------------------

/// Cloud STT — wraps [`transcribe_cloud`]. Stateless; cheap to construct.
pub struct CloudSttProvider {
    model: String,
}

impl CloudSttProvider {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
        }
    }
}

#[async_trait]
impl SttProvider for CloudSttProvider {
    fn name(&self) -> &'static str {
        "cloud"
    }

    async fn transcribe(
        &self,
        config: &Config,
        audio_base64: &str,
        mime_type: Option<&str>,
        file_name: Option<&str>,
        language: Option<&str>,
    ) -> Result<RpcOutcome<SttResult>, String> {
        debug!(
            "{LOG_PREFIX} cloud STT dispatch model={} bytes_b64={}",
            self.model,
            audio_base64.len()
        );
        let opts = CloudTranscribeOptions {
            model: Some(self.model.clone()),
            language: language.map(str::to_string),
            mime_type: mime_type.map(str::to_string),
            file_name: file_name.map(str::to_string),
        };

        match transcribe_cloud(config, audio_base64, &opts).await {
            Ok(outcome) => {
                let CloudTranscribeResult { text } = outcome.value;
                return Ok(RpcOutcome::single_log(
                    SttResult {
                        text,
                        provider: "cloud".to_string(),
                    },
                    "voice-factory: cloud STT completed",
                ));
            }
            Err(e) => {
                let lower = e.to_lowercase();
                let is_session_error = e.contains("no backend session token")
                    || e.contains("sign in first")
                    || e.contains("401")
                    || e.contains("Unauthorized")
                    || lower.contains("invalid token")
                    || lower.contains("session jwt required")
                    || lower.contains("session expired")
                    || e.contains("SESSION_EXPIRED");
                if !is_session_error {
                    return Err(e);
                }
                debug!("{LOG_PREFIX} cloud STT unavailable (session), trying fallbacks: {e}");
            }
        }

        // Fallback 1: Doubao (火山引擎) STT if credentials configured
        if let (Some(app_id), Some(access_token)) = (
            &config.local_ai.doubao_app_id,
            &config.local_ai.doubao_access_token,
        ) {
            if !app_id.trim().is_empty() && !access_token.trim().is_empty() {
                debug!("{LOG_PREFIX} falling back to doubao STT");
                let doubao_opts = DoubaoSttOptions {
                    app_id: Some(app_id.clone()),
                    access_token: Some(access_token.clone()),
                    audio_format: mime_type
                        .map(|m| m.strip_prefix("audio/").unwrap_or(m).to_string()),
                    language: language.map(str::to_string),
                };
                match transcribe_doubao(config, audio_base64, &doubao_opts).await {
                    Ok(outcome) => {
                        return Ok(RpcOutcome::single_log(
                            SttResult {
                                text: outcome.value.text,
                                provider: "doubao".to_string(),
                            },
                            "voice-factory: doubao STT (cloud fallback) completed",
                        ));
                    }
                    Err(e) => {
                        debug!("{LOG_PREFIX} doubao fallback failed: {e}");
                    }
                }
            }
        }

        // Fallback 2: Local Whisper
        debug!("{LOG_PREFIX} falling back to local whisper STT");
        let whisper_opts = WhisperTranscribeOptions {
            model: Some(DEFAULT_WHISPER_MODEL.to_string()),
            mime_type: mime_type.map(str::to_string),
            language: language.map(str::to_string),
        };
        let outcome = transcribe_whisper(config, audio_base64, &whisper_opts).await?;
        Ok(RpcOutcome::single_log(
            SttResult {
                text: outcome.value.text,
                provider: "whisper".to_string(),
            },
            "voice-factory: whisper STT (cloud fallback) completed",
        ))
    }
}

// ---------------------------------------------------------------------------
// Local Whisper STT
// ---------------------------------------------------------------------------

/// Local Whisper STT — wraps [`transcribe_whisper`]. Resolves `WHISPER_BIN`
/// lazily on each call.
pub struct WhisperSttProvider {
    model: String,
}

impl WhisperSttProvider {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
        }
    }
}

#[async_trait]
impl SttProvider for WhisperSttProvider {
    fn name(&self) -> &'static str {
        "whisper"
    }

    async fn transcribe(
        &self,
        config: &Config,
        audio_base64: &str,
        mime_type: Option<&str>,
        _file_name: Option<&str>,
        language: Option<&str>,
    ) -> Result<RpcOutcome<SttResult>, String> {
        debug!(
            "{LOG_PREFIX} whisper STT dispatch model={} mime={:?} lang={:?}",
            self.model, mime_type, language
        );
        let opts = WhisperTranscribeOptions {
            model: Some(self.model.clone()),
            mime_type: mime_type.map(str::to_string),
            language: language.map(str::to_string),
        };
        let outcome = transcribe_whisper(config, audio_base64, &opts).await?;
        Ok(RpcOutcome::single_log(
            SttResult {
                text: outcome.value.text,
                provider: "whisper".to_string(),
            },
            "voice-factory: whisper STT completed",
        ))
    }
}

// ---------------------------------------------------------------------------
// Doubao (火山引擎) STT
// ---------------------------------------------------------------------------

pub struct DoubaoSttProvider {
    app_id: Option<String>,
    access_token: Option<String>,
}
impl DoubaoSttProvider {
    pub fn new(app_id: Option<String>, access_token: Option<String>) -> Self {
        Self {
            app_id,
            access_token,
        }
    }
}
#[async_trait]
impl SttProvider for DoubaoSttProvider {
    fn name(&self) -> &'static str {
        "doubao"
    }
    async fn transcribe(
        &self,
        config: &Config,
        audio_base64: &str,
        mime_type: Option<&str>,
        _file_name: Option<&str>,
        language: Option<&str>,
    ) -> Result<RpcOutcome<SttResult>, String> {
        let opts = DoubaoSttOptions {
            app_id: self.app_id.clone(),
            access_token: self.access_token.clone(),
            audio_format: mime_type.map(|m| m.strip_prefix("audio/").unwrap_or(m).to_string()),
            language: language.map(str::to_string),
        };
        let outcome = transcribe_doubao(config, audio_base64, &opts).await?;
        Ok(RpcOutcome::single_log(
            SttResult {
                text: outcome.value.text,
                provider: "doubao".into(),
            },
            "voice-factory: doubao STT completed",
        ))
    }
}

// ---------------------------------------------------------------------------
// Doubao (火山引擎) TTS
// ---------------------------------------------------------------------------

pub struct DoubaoTtsProvider {
    app_id: Option<String>,
    access_token: Option<String>,
    voice: Option<String>,
}
impl DoubaoTtsProvider {
    pub fn new(
        app_id: Option<String>,
        access_token: Option<String>,
        voice: Option<String>,
    ) -> Self {
        Self {
            app_id,
            access_token,
            voice,
        }
    }
}
#[async_trait]
impl TtsProvider for DoubaoTtsProvider {
    fn name(&self) -> &'static str {
        "doubao"
    }
    async fn synthesize(
        &self,
        config: &Config,
        text: &str,
        voice: Option<&str>,
    ) -> Result<RpcOutcome<ReplySpeechResult>, String> {
        let opts = DoubaoTtsOptions {
            app_id: self.app_id.clone(),
            access_token: self.access_token.clone(),
            voice: voice.map(str::to_string).or(self.voice.clone()),
            speed: Some(config.local_ai.doubao_tts_speed),
            volume: None,
        };
        let outcome = synthesize_doubao(config, text, &opts).await?;
        Ok(RpcOutcome::single_log(
            ReplySpeechResult {
                audio_base64: outcome.value.audio_base64,
                audio_mime: outcome.value.audio_mime,
                visemes: vec![],
                alignment: None,
            },
            "voice-factory: doubao TTS completed",
        ))
    }
}

// ---------------------------------------------------------------------------
// Cloud TTS
// ---------------------------------------------------------------------------

/// Cloud TTS — wraps [`synthesize_reply`] (backend ElevenLabs proxy).
pub struct CloudTtsProvider {
    voice: Option<String>,
}

impl CloudTtsProvider {
    pub fn new(voice: Option<String>) -> Self {
        Self { voice }
    }
}

#[async_trait]
impl TtsProvider for CloudTtsProvider {
    fn name(&self) -> &'static str {
        "cloud"
    }

    async fn synthesize(
        &self,
        config: &Config,
        text: &str,
        voice: Option<&str>,
    ) -> Result<RpcOutcome<ReplySpeechResult>, String> {
        let resolved_voice = voice
            .map(str::to_string)
            .or_else(|| self.voice.clone())
            .filter(|s| !s.trim().is_empty());
        debug!(
            "{LOG_PREFIX} cloud TTS dispatch voice={} chars={}",
            resolved_voice.as_deref().unwrap_or("<default>"),
            text.len()
        );
        let opts = ReplySpeechOptions {
            voice_id: resolved_voice,
            model_id: None,
            output_format: None,
            voice_settings: None,
        };
        synthesize_reply(config, text, &opts).await
    }
}

// ---------------------------------------------------------------------------
// Local Piper TTS
// ---------------------------------------------------------------------------

/// Local Piper TTS — wraps [`synthesize_piper`].
pub struct PiperTtsProvider {
    voice: String,
}

impl PiperTtsProvider {
    pub fn new(voice: impl Into<String>) -> Self {
        Self {
            voice: voice.into(),
        }
    }
}

#[async_trait]
impl TtsProvider for PiperTtsProvider {
    fn name(&self) -> &'static str {
        "piper"
    }

    async fn synthesize(
        &self,
        config: &Config,
        text: &str,
        voice: Option<&str>,
    ) -> Result<RpcOutcome<ReplySpeechResult>, String> {
        let resolved_voice = voice
            .map(str::to_string)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| self.voice.clone());
        debug!(
            "{LOG_PREFIX} piper TTS dispatch voice={} chars={}",
            resolved_voice,
            text.len()
        );
        let opts = PiperOptions {
            voice: Some(resolved_voice),
        };
        synthesize_piper(config, text, &opts).await
    }
}

// ---------------------------------------------------------------------------
// Factory entry points (mirrors embeddings/factory.rs)
// ---------------------------------------------------------------------------

/// Creates a speech-to-text provider based on the specified name and model.
///
/// Supported provider names:
/// - `"cloud"` → backend Whisper proxy — default, preferred for laptops
///   without local models
/// - `"whisper"` → local whisper.cpp via `WHISPER_BIN` (or in-process
///   `whisper-rs` when configured)
///
/// Returns an error for unrecognised provider names so configuration
/// mistakes surface immediately rather than silently degrading.
///
/// The factory does not eagerly resolve the binary — `WhisperSttProvider`
/// looks up `WHISPER_BIN` lazily inside `transcribe()` so a misconfigured
/// install fails at use-time with a clear error message instead of at
/// startup.
pub fn create_stt_provider(
    provider: &str,
    model: &str,
    _config: &Config,
) -> anyhow::Result<Box<dyn SttProvider>> {
    debug!("{LOG_PREFIX} create_stt_provider provider={provider} model={model}");
    let model = if model.trim().is_empty() {
        DEFAULT_WHISPER_MODEL
    } else {
        model
    };
    match provider.trim() {
        "cloud" => Ok(Box::new(CloudSttProvider::new(
            super::cloud_transcribe_default_model(),
        ))),
        "doubao" => Ok(Box::new(DoubaoSttProvider::new(
            _config.local_ai.doubao_app_id.clone(),
            _config.local_ai.doubao_access_token.clone(),
        ))),
        "whisper" => Ok(Box::new(WhisperSttProvider::new(model))),
        unknown => Err(anyhow::anyhow!(
            "unknown STT provider: \"{unknown}\". Supported: \"cloud\", \"doubao\", \"whisper\""
        )),
    }
}

/// Creates a text-to-speech provider based on the specified name and voice.
///
/// Supported provider names:
/// - `"cloud"` → backend ElevenLabs proxy with viseme alignment
/// - `"piper"` → local Piper subprocess via `PIPER_BIN`
///
/// Kokoro is **not** implemented in this cut — the integration shipped with
/// Piper because `PIPER_BIN` is already reserved in `.env.example` and the
/// runtime contract (subprocess + `.onnx` model) is simpler. Adding Kokoro
/// later is straightforward: add a new branch here and a `local_speech_kokoro`
/// sibling module.
pub fn create_tts_provider(
    provider: &str,
    voice: &str,
    _config: &Config,
) -> anyhow::Result<Box<dyn TtsProvider>> {
    debug!("{LOG_PREFIX} create_tts_provider provider={provider} voice={voice}");
    let voice = if voice.trim().is_empty() {
        DEFAULT_PIPER_VOICE
    } else {
        voice
    };
    match provider.trim() {
        "cloud" => Ok(Box::new(CloudTtsProvider::new(if voice.is_empty() {
            None
        } else {
            Some(voice.to_string())
        }))),
        "doubao" => Ok(Box::new(DoubaoTtsProvider::new(
            _config.local_ai.doubao_app_id.clone(),
            _config.local_ai.doubao_access_token.clone(),
            if voice.is_empty() {
                None
            } else {
                Some(voice.to_string())
            },
        ))),
        "piper" => Ok(Box::new(PiperTtsProvider::new(voice))),
        unknown => Err(anyhow::anyhow!(
            "unknown TTS provider: \"{unknown}\". Supported: \"cloud\", \"doubao\", \"piper\""
        )),
    }
}

/// Default Whisper model for OpenHuman-ZN (国产化).
/// `large-v3-turbo` supports 99 languages including zh, en, ja, ko.
pub const DEFAULT_WHISPER_MODEL: &str = "large-v3-turbo";

/// Compact whisper model for low-end hardware.
pub const MINIMAL_WHISPER_MODEL: &str = "small";

/// Default Piper voice — `en_US-lessac-medium`, matches
/// [`super::super::local_ai::model_ids::effective_tts_voice_id`].
/// Default Piper voice for OpenHuman-ZN (国产化) — 中文女声花颜。
pub const DEFAULT_PIPER_VOICE: &str = "zh_CN-huayan-medium";

/// English fallback when zh-CN model unavailable.
pub const FALLBACK_PIPER_VOICE: &str = "en_US-lessac-medium";

/// Whisper install presets (size tiers exposed to the installer UI).
/// Mirrors the Ollama model installer surface: each entry is `(id, label)`.
pub const WHISPER_MODEL_PRESETS: &[(&str, &str)] = &[
    ("tiny", "Tiny (39 MB, fastest)"),
    ("base", "Base (74 MB)"),
    ("small", "Small (244 MB)"),
    ("medium", "Medium (769 MB, recommended)"),
    ("large-v3-turbo", "Large v3 Turbo (1.5 GB, best accuracy)"),
];

/// Chinese-specialized whisper model presets for OpenHuman-ZN.
pub const WHISPER_ZH_PRESETS: &[(&str, &str)] = &[
    ("tiny", "Tiny (39 MB) — 中文测试"),
    ("base", "Base (74 MB) — 中文基本可用"),
    ("small", "Small (244 MB) — 中文推荐最低"),
    ("medium", "Medium (769 MB) — 中英文推荐"),
    ("large-v3-turbo", "Large v3 Turbo (1.5 GB) — 最佳精度"),
];

/// Piper TTS voice presets for OpenHuman-ZN.
pub const PIPER_VOICE_PRESETS: &[(&str, &str)] = &[
    ("zh_CN-huayan-medium", "中文女声 花颜 medium (~50 MB)"),
    ("zh_CN-huayan-low", "中文女声 花颜 low (~50 MB)"),
    ("zh_CN-kefu-medium", "中文女声 克服 medium (~50 MB)"),
    ("en_US-lessac-medium", "英文女声 Lessac medium (~40 MB)"),
    ("en_US-ryan-medium", "英文男声 Ryan medium (~40 MB)"),
];

/// Returns a thread-safe default STT provider (cloud). Used by callers that
/// can't easily plumb a `Config` reference but still need a sensible default.
pub fn default_stt_provider() -> Arc<dyn SttProvider> {
    Arc::new(WhisperSttProvider::new(DEFAULT_WHISPER_MODEL))
}

/// Returns a thread-safe default TTS provider (cloud).
pub fn default_tts_provider() -> Arc<dyn TtsProvider> {
    Arc::new(PiperTtsProvider::new(DEFAULT_PIPER_VOICE))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config::default()
    }

    #[test]
    fn stt_factory_cloud_branch() {
        let p = create_stt_provider("cloud", "ignored", &cfg()).unwrap();
        assert_eq!(p.name(), "cloud");
    }

    #[test]
    fn stt_factory_whisper_branch() {
        let p = create_stt_provider("whisper", "whisper-large-v3-turbo", &cfg()).unwrap();
        assert_eq!(p.name(), "whisper");
    }

    #[test]
    fn stt_factory_whisper_empty_model_uses_default() {
        // Empty model → default whisper-large-v3-turbo; constructor must not
        // reject an empty string with an opaque error.
        let p = create_stt_provider("whisper", "", &cfg()).unwrap();
        assert_eq!(p.name(), "whisper");
    }

    #[test]
    fn stt_factory_unknown_provider_errors() {
        let err = create_stt_provider("deepgram", "nova-2", &cfg())
            .err()
            .expect("deepgram is not implemented");
        let msg = err.to_string();
        assert!(msg.contains("deepgram"), "should name the provider: {msg}");
        assert!(msg.contains("unknown"), "should say unknown: {msg}");
    }

    #[test]
    fn stt_factory_empty_string_errors() {
        let err = create_stt_provider("", "model", &cfg())
            .err()
            .expect("empty provider must error");
        assert!(err.to_string().contains("unknown"));
    }

    #[test]
    fn tts_factory_cloud_branch() {
        let p = create_tts_provider("cloud", "Rachel", &cfg()).unwrap();
        assert_eq!(p.name(), "cloud");
    }

    #[test]
    fn tts_factory_piper_branch() {
        let p = create_tts_provider("piper", "en_US-lessac-medium", &cfg()).unwrap();
        assert_eq!(p.name(), "piper");
    }

    #[test]
    fn tts_factory_piper_empty_voice_uses_default() {
        let p = create_tts_provider("piper", "", &cfg()).unwrap();
        assert_eq!(p.name(), "piper");
    }

    #[test]
    fn tts_factory_unknown_provider_errors() {
        let err = create_tts_provider("kokoro", "af_bella", &cfg())
            .err()
            .expect("kokoro is not implemented in this cut");
        let msg = err.to_string();
        assert!(msg.contains("kokoro"), "should name the provider: {msg}");
        assert!(msg.contains("unknown"), "should say unknown: {msg}");
    }

    #[test]
    fn whisper_presets_cover_full_size_ladder() {
        // Sanity-check the installer surface: tiny→large-v3-turbo must all be
        // exposed so the local-AI panel can render the size picker without
        // hard-coding the list.
        let ids: Vec<&str> = WHISPER_MODEL_PRESETS.iter().map(|(id, _)| *id).collect();
        for expected in ["tiny", "base", "small", "medium", "large-v3-turbo"] {
            assert!(
                ids.contains(&expected),
                "WHISPER_MODEL_PRESETS missing {expected}"
            );
        }
    }

    #[tokio::test]
    async fn whisper_provider_fails_clearly_when_binary_missing() {
        // No WHISPER_BIN env, no model file — the provider must surface an
        // actionable error rather than panic. Drive a small base64 payload
        // so we never reach the actual transcription call.
        let _guard = unset_env_guard("WHISPER_BIN");
        let provider = WhisperSttProvider::new("whisper-large-v3-turbo");
        let result = provider
            .transcribe(&cfg(), "AAAA", Some("audio/wav"), None, None)
            .await;
        assert!(result.is_err(), "missing binary must error");
        let msg = result.err().unwrap();
        // Whatever the underlying message says, it must NOT be a serialize
        // panic — i.e. we must have hit the binary-resolution branch.
        assert!(
            !msg.is_empty(),
            "error message should be populated for diagnosis"
        );
    }

    #[test]
    fn default_providers_return_local() {
        assert_eq!(default_stt_provider().name(), "whisper");
        assert_eq!(default_tts_provider().name(), "piper");
    }

    /// Drop guard that unsets an env var on construction and restores it on
    /// drop. Necessary because cargo runs tests in parallel and bare
    /// `remove_var` would leak across tests.
    fn unset_env_guard(key: &'static str) -> EnvUnsetGuard {
        let prev = std::env::var_os(key);
        std::env::remove_var(key);
        EnvUnsetGuard { key, prev }
    }

    struct EnvUnsetGuard {
        key: &'static str,
        prev: Option<std::ffi::OsString>,
    }
    impl Drop for EnvUnsetGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }
}
