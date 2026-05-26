use crate::commands::models::switch_active_model;
use crate::managers::model::{EngineType, ModelManager};
use crate::managers::transcription::{
    LongFormConfig, TranscriptionManager, ANTI_HALLUC_ENTROPY_THOLD, ANTI_HALLUC_N_MAX_TEXT_CTX,
};
use crate::settings::{get_settings, write_settings, AppSettings, ModelUnloadTimeout};
use anyhow::{anyhow, Result};
use chrono::Local;
use hound::{SampleFormat, WavReader};
use log::{debug, info, warn};
use rubato::{FftFixedIn, Resampler};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Manager, State};

const TARGET_SAMPLE_RATE: u32 = 16_000;
const RESAMPLER_CHUNK_SIZE: usize = 1024;

struct RunSpec {
    model_id: &'static str,
    engine_label: &'static str,
    use_prompt: bool,
    use_anti_halluc: bool,
    sot_lang_tokens: Option<&'static [&'static str]>,
}

// Superset RUN_MATRIX for the 2026-04-23 v17 night run + v20 live_v1 R1/R2 (47 rows). Each row
// is the unique (model_id, use_prompt, use_anti_halluc, sot_lang_tokens)
// signature — the per-invocation `prompt` override and `language` arg cover
// the remaining axes (V1/V2/V3/V4 and ru/auto). Resume-key now includes
// effective_initial_prompt + language, so the same row run with different
// prompts / languages is distinct.
//
// Layout: for each Whisper model, the 4 (use_prompt × ah) × sot=None combos
// cover all "Angle C" style conditions. Extra rows for LID variants where
// Block G, the original Phase A/1 experiments, and v17 P0/P1/P2 need them.
const RUN_MATRIX: &[RunSpec] = &[
    // ==== breeze-asr (Block A / A2 / B / F / J) ====
    RunSpec {
        model_id: "breeze-asr",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "breeze-asr",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: true,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "breeze-asr",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "breeze-asr",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: true,
        sot_lang_tokens: None,
    },
    // v17 LID variants (P0 noprompt+lid_ru+ah; P1/P2 V2/V3/V4 × lid_ru / lid_en+ru / lid_ru+en × ah).
    RunSpec {
        model_id: "breeze-asr",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: true,
        sot_lang_tokens: Some(&["ru"]),
    },
    RunSpec {
        model_id: "breeze-asr",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: true,
        sot_lang_tokens: Some(&["ru"]),
    },
    RunSpec {
        model_id: "breeze-asr",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: true,
        sot_lang_tokens: Some(&["en", "ru"]),
    },
    RunSpec {
        model_id: "breeze-asr",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: true,
        sot_lang_tokens: Some(&["ru", "en"]),
    },
    // ==== breeze-asr-26 (MediaTek 2nd-gen Breeze; mirrors breeze-asr matrix for cross-version comparison) ====
    RunSpec {
        model_id: "breeze-asr-26",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "breeze-asr-26",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: true,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "breeze-asr-26",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "breeze-asr-26",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: true,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "breeze-asr-26",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: true,
        sot_lang_tokens: Some(&["ru"]),
    },
    RunSpec {
        model_id: "breeze-asr-26",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: true,
        sot_lang_tokens: Some(&["ru"]),
    },
    RunSpec {
        model_id: "breeze-asr-26",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: true,
        sot_lang_tokens: Some(&["en", "ru"]),
    },
    RunSpec {
        model_id: "breeze-asr-26",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: true,
        sot_lang_tokens: Some(&["ru", "en"]),
    },
    // ==== ggml-medium (Block A / A2 / B / G) ====
    RunSpec {
        model_id: "ggml-medium",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "ggml-medium",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: true,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "ggml-medium",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "ggml-medium",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: true,
        sot_lang_tokens: None,
    },
    // Block G: ggml-medium noprompt+ah × LID en+ru.
    RunSpec {
        model_id: "ggml-medium",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: true,
        sot_lang_tokens: Some(&["en", "ru"]),
    },
    // v20 H22 — ggml-medium noprompt × LID ru+en + AH (live_v1 Tier 5).
    RunSpec {
        model_id: "ggml-medium",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: true,
        sot_lang_tokens: Some(&["ru", "en"]),
    },
    // ==== medium (Block B) ====
    RunSpec {
        model_id: "medium",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "medium",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: true,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "medium",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "medium",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: true,
        sot_lang_tokens: None,
    },
    // ==== turbo (Block A / B) ====
    RunSpec {
        model_id: "turbo",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "turbo",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: true,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "turbo",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "turbo",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: true,
        sot_lang_tokens: None,
    },
    // v17 P2 LID variants (turbo × lid_ru).
    RunSpec {
        model_id: "turbo",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: true,
        sot_lang_tokens: Some(&["ru"]),
    },
    RunSpec {
        model_id: "turbo",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: true,
        sot_lang_tokens: Some(&["ru"]),
    },
    // ==== large (Block C / C2) ====
    RunSpec {
        model_id: "large",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "large",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: true,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "large",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "large",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: true,
        sot_lang_tokens: None,
    },
    // ==== ggml-large-v3 (Block C) ====
    RunSpec {
        model_id: "ggml-large-v3",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "ggml-large-v3",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: true,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "ggml-large-v3",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "ggml-large-v3",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: true,
        sot_lang_tokens: None,
    },
    // ==== whisper-large-v3-russian (Block C / C2) ====
    RunSpec {
        model_id: "whisper-large-v3-russian",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "whisper-large-v3-russian",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: true,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "whisper-large-v3-russian",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "whisper-large-v3-russian",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: true,
        sot_lang_tokens: None,
    },
    // v20 Round 2 H37 — wlv3-russian noprompt × LID ru + AH.
    RunSpec {
        model_id: "whisper-large-v3-russian",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: true,
        sot_lang_tokens: Some(&["ru"]),
    },
    // ==== whisper-podlodka-turbo (Block B / D / G) ====
    RunSpec {
        model_id: "whisper-podlodka-turbo",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "whisper-podlodka-turbo",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: true,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "whisper-podlodka-turbo",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "whisper-podlodka-turbo",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: true,
        sot_lang_tokens: None,
    },
    // Block G: podlodka noprompt+ah × LID variants.
    RunSpec {
        model_id: "whisper-podlodka-turbo",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: true,
        sot_lang_tokens: Some(&["en", "ru"]),
    },
    RunSpec {
        model_id: "whisper-podlodka-turbo",
        engine_label: "whisper",
        use_prompt: false,
        use_anti_halluc: true,
        sot_lang_tokens: Some(&["ru", "en"]),
    },
    // v20 H21 — podlodka prompt × LID ru+en + AH (live_v1 Tier 5).
    RunSpec {
        model_id: "whisper-podlodka-turbo",
        engine_label: "whisper",
        use_prompt: true,
        use_anti_halluc: true,
        sot_lang_tokens: Some(&["ru", "en"]),
    },
    // ==== Non-Whisper (kept for completeness; not invoked by any block below) ====
    RunSpec {
        model_id: "parakeet-tdt-0.6b-v3",
        engine_label: "parakeet",
        use_prompt: false,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "canary-1b-v2",
        engine_label: "canary",
        use_prompt: false,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
    RunSpec {
        model_id: "gigaam-v3-e2e-ctc",
        engine_label: "gigaam",
        use_prompt: false,
        use_anti_halluc: false,
        sot_lang_tokens: None,
    },
];

/// Per-model maximum chunk duration for VAD-based long-form transcription
/// (non-Whisper engines only). Values chosen conservatively — models have no
/// documented hard max; they fail with ONNX shape errors beyond certain sizes.
fn default_max_chunk_secs(model_id: &str) -> f32 {
    match model_id {
        "parakeet-tdt-0.6b-v3" => 20.0,
        "canary-1b-v2" => 30.0,
        "gigaam-v3-e2e-ctc" => 20.0,
        _ => 20.0,
    }
}

#[derive(Serialize, Deserialize, Type, Clone)]
pub struct BenchmarkRunRecord {
    model_id: String,
    model_name: String,
    engine: String,
    use_prompt: bool,
    use_anti_halluc: bool,
    run_idx: u32,
    language: String,
    translate: bool,
    transcription_prompt: Option<String>,
    custom_words: Vec<String>,
    transcribe_time_ms: u64,
    rtf: f64,
    text: String,
    error: Option<String>,
    chunk_count: u32,
    max_chunk_secs: Option<f32>,
    sot_lang_tokens: Option<Vec<String>>,
    /// The `initial_prompt` string actually fed to whisper.cpp — the same
    /// `custom_words + "\n\n" + transcription_prompt` concatenation assembled
    /// in `managers/transcription.rs`. Lets the JSON record stand on its own
    /// when UI-state leaks into a run (e.g. the 2026-04-22 V2 surprise).
    effective_initial_prompt: Option<String>,
    /// `n_max_text_ctx` value passed to whisper.cpp's `FullParams` for this
    /// run. `Some(ANTI_HALLUC_N_MAX_TEXT_CTX)` when anti-halluc is on, None
    /// otherwise. Survives future tuning of the const.
    effective_n_max_text_ctx: Option<i32>,
    /// `entropy_thold` value passed to whisper.cpp's `FullParams` for this
    /// run. `Some(ANTI_HALLUC_ENTROPY_THOLD)` when anti-halluc is on, None
    /// otherwise.
    effective_entropy_thold: Option<f32>,
    /// SOT `prompt_init` token IDs the decoder actually received, read back
    /// from whisper.cpp state via `whisper_get_last_prompt_init`. For LID-hack
    /// rows the sequence is
    /// `[<|startoftranscript|>, <|lang_a|>, <|lang_b|>, ..., <|transcribe|>, <|notimestamps|>]`;
    /// for baseline rows it's `[<|sot|>, <|auto-lang|>, <|transcribe|>, <|notimestamps|>]`.
    /// None when the engine is non-whisper, transcribe crashed before
    /// whisper.cpp assembled prompt_init, or the getter returned empty.
    decoder_prompt_init_tokens: Option<Vec<i32>>,
    /// Bare filename of the audio file this run was transcribed from (e.g.
    /// "live-01-3s.wav"). None in single-file mode (legacy `file_path` path);
    /// Some(filename) in audio_batch mode. Lets the eval driver look up
    /// per-file metadata (speaker, duration, env) without parsing run order.
    audio_file: Option<String>,
}

#[derive(Serialize, Deserialize, Type)]
pub struct BenchmarkAggregate {
    model_id: String,
    model_name: String,
    engine: String,
    use_prompt: bool,
    use_anti_halluc: bool,
    successful_runs: u32,
    time_min_ms: Option<u64>,
    time_median_ms: Option<u64>,
    time_mean_ms: Option<f64>,
    time_stdev_ms: Option<f64>,
    rtf_median: Option<f64>,
    texts_identical: bool,
    first_error: Option<String>,
}

/// Bundle of optional overrides threaded into a single benchmark invocation.
/// These apply on top of (or instead of) the per-spec values in RUN_MATRIX.
///
/// Grouped into a struct because tauri/specta caps command signatures at
/// ten parameters; collecting overrides here keeps the door open for more
/// toggles without another refactor.
#[derive(Deserialize, Serialize, Type, Debug, Default, Clone)]
pub struct BenchmarkOverrides {
    /// When Some, overrides settings.transcription_prompt for runs where
    /// RunSpec::use_prompt is true. Custom words are cleared so the test
    /// isolates the overridden prompt.
    pub prompt: Option<String>,
    /// Skip RUN_MATRIX entries with use_prompt == false. Useful for a quick
    /// "prompt only" sweep.
    pub skip_no_prompt: Option<bool>,
    /// When Some, overrides RunSpec::sot_lang_tokens for every row. Lets a
    /// DevTools caller flip the Peng-style LID hack on/off for a full matrix
    /// pass without editing the constant.
    pub sot_lang_tokens: Option<Vec<String>>,
    /// Resume-from-checkpoint. When Some(path) AND the file exists and parses,
    /// seed `runs` with the prior (non-errored) records and skip any matching
    /// (model_id, use_prompt, use_anti_halluc, sot_lang_tokens, run_idx) tuples.
    /// Missing or unreadable file → fresh start with a warning, not an error:
    /// callers can pass the expected checkpoint path unconditionally.
    pub resume_from: Option<String>,
    /// Tight filter: when Some(list), only RUN_MATRIX rows whose
    /// (model_id, use_prompt, use_anti_halluc, sot_lang_tokens) tuple appears
    /// in the list are executed. Every other row is skipped silently. Used by
    /// the night-run queue runner to pin each invocation to exactly the
    /// conditions it's supposed to measure, independent of
    /// skip_models / skip_no_prompt. `model_id_null_as_wildcard` is NOT a
    /// feature — explicit tuples only.
    pub only_conditions: Option<Vec<ConditionFilter>>,
    /// When Some(non-empty list), switches benchmark to multi-file batch mode:
    /// model loads ONCE per condition, then iterates over each file × N runs.
    /// Each record gets `audio_file` populated with the bare filename. The
    /// positional `file_path` arg is treated as a label only (used for
    /// checkpoint naming and the report's `input_file` field) and its
    /// existence is NOT checked. None or empty list → legacy single-file
    /// mode using `file_path` as the actual audio source.
    pub audio_batch: Option<Vec<String>>,
    /// When Some(n), shifts run_idx numbering for this invocation: runs carry
    /// run_idx ∈ [n, n + runs_per_condition). Used by Round 2 T3 to append 8
    /// new runs (run_idx 12..19) to Round 1 baseline (run_idx 0..11) without
    /// collision when the eval driver merges by (cid, audio_file, run_idx).
    /// Default: 0. Resume-key includes run_idx as-is, so offset values survive
    /// resume-from-checkpoint cleanly.
    pub run_idx_offset: Option<u32>,
}

/// Single condition signature the night-run queue uses to pin an invocation
/// to a specific matrix-row subset. See `BenchmarkOverrides::only_conditions`.
#[derive(Deserialize, Serialize, Type, Debug, Clone)]
pub struct ConditionFilter {
    pub model_id: String,
    pub use_prompt: bool,
    pub use_anti_halluc: bool,
    /// None = sot_lang_tokens must equal None (i.e. auto-LID / no Peng-hack).
    /// Some(list) = sot_lang_tokens must equal that exact list.
    pub sot_lang_tokens: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Type)]
pub struct BenchmarkReport {
    timestamp: String,
    input_file: String,
    warmup_file: Option<String>,
    audio_duration_s: f64,
    runs_per_condition: u32,
    runs: Vec<BenchmarkRunRecord>,
    aggregates: Vec<BenchmarkAggregate>,
}

/// Mirror of the `initial_prompt` assembly in `managers/transcription.rs`
/// (`custom_words.join(", ") + "\n\n" + transcription_prompt`), applied to the
/// post-override `AppSettings` snapshot. Used to stamp every benchmark record
/// with the exact prompt string whisper.cpp saw. Returns None when both
/// `custom_words` and `transcription_prompt` are empty or whitespace-only.
fn compute_effective_initial_prompt(s: &AppSettings) -> Option<String> {
    let mut parts = Vec::new();
    if !s.custom_words.is_empty() {
        parts.push(s.custom_words.join(", "));
    }
    if let Some(ref prompt) = s.transcription_prompt {
        if !prompt.trim().is_empty() {
            parts.push(prompt.clone());
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

struct SettingsGuard {
    app: AppHandle,
    original: AppSettings,
}

impl Drop for SettingsGuard {
    fn drop(&mut self) {
        write_settings(&self.app, self.original.clone());
        info!("benchmark: restored original settings");
    }
}

fn load_wav_mono_16k(path: &Path) -> Result<Vec<f32>> {
    let reader = WavReader::open(path)
        .map_err(|e| anyhow!("Failed to open WAV {}: {}", path.display(), e))?;
    let spec = reader.spec();
    let channels = spec.channels as usize;
    let sample_rate = spec.sample_rate;

    if channels == 0 {
        return Err(anyhow!("WAV has zero channels: {}", path.display()));
    }

    let interleaved: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Float, _) => reader
            .into_samples::<f32>()
            .collect::<Result<Vec<f32>, _>>()
            .map_err(|e| anyhow!("Failed to read float samples: {}", e))?,
        (SampleFormat::Int, 16) => reader
            .into_samples::<i16>()
            .map(|s| s.map(|v| v as f32 / i16::MAX as f32))
            .collect::<Result<Vec<f32>, _>>()
            .map_err(|e| anyhow!("Failed to read i16 samples: {}", e))?,
        (SampleFormat::Int, bits) => {
            let max = 2f32.powi(bits as i32 - 1);
            reader
                .into_samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max))
                .collect::<Result<Vec<f32>, _>>()
                .map_err(|e| anyhow!("Failed to read {}-bit int samples: {}", bits, e))?
        }
    };

    let mono: Vec<f32> = if channels == 1 {
        interleaved
    } else {
        interleaved
            .chunks(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    };

    info!(
        "benchmark: loaded WAV {} (sr={}, ch={}, bits={}, fmt={:?}, samples_mono={})",
        path.display(),
        sample_rate,
        channels,
        spec.bits_per_sample,
        spec.sample_format,
        mono.len()
    );

    if sample_rate == TARGET_SAMPLE_RATE {
        return Ok(mono);
    }

    let mut resampler = FftFixedIn::<f32>::new(
        sample_rate as usize,
        TARGET_SAMPLE_RATE as usize,
        RESAMPLER_CHUNK_SIZE,
        1,
        1,
    )
    .map_err(|e| anyhow!("Failed to create resampler: {}", e))?;

    let expected_out_len =
        (mono.len() as f64 * TARGET_SAMPLE_RATE as f64 / sample_rate as f64) as usize + 2048;
    let mut out = Vec::with_capacity(expected_out_len);

    let mut i = 0;
    while i + RESAMPLER_CHUNK_SIZE <= mono.len() {
        let input = &mono[i..i + RESAMPLER_CHUNK_SIZE];
        let chunk_out = resampler
            .process(&[input], None)
            .map_err(|e| anyhow!("Resampler failed on chunk: {}", e))?;
        out.extend_from_slice(&chunk_out[0]);
        i += RESAMPLER_CHUNK_SIZE;
    }
    if i < mono.len() {
        let mut tail = vec![0.0f32; RESAMPLER_CHUNK_SIZE];
        tail[..mono.len() - i].copy_from_slice(&mono[i..]);
        let chunk_out = resampler
            .process(&[&tail], None)
            .map_err(|e| anyhow!("Resampler failed on tail: {}", e))?;
        out.extend_from_slice(&chunk_out[0]);
    }

    info!(
        "benchmark: resampled {} → {} Hz ({} → {} samples)",
        sample_rate,
        TARGET_SAMPLE_RATE,
        mono.len(),
        out.len()
    );

    Ok(out)
}

fn median(values: &[u64]) -> u64 {
    let mut sorted: Vec<u64> = values.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    if n == 0 {
        return 0;
    }
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2
    }
}

fn mean_stdev(values: &[u64]) -> (f64, f64) {
    let n = values.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let mean = values.iter().map(|&v| v as f64).sum::<f64>() / n as f64;
    if n < 2 {
        return (mean, 0.0);
    }
    let variance = values
        .iter()
        .map(|&v| {
            let d = v as f64 - mean;
            d * d
        })
        .sum::<f64>()
        / (n - 1) as f64;
    (mean, variance.sqrt())
}

fn compute_aggregates(runs: &[BenchmarkRunRecord]) -> Vec<BenchmarkAggregate> {
    let mut aggregates = Vec::new();
    let mut seen: Vec<(String, bool, bool)> = Vec::new();
    for run in runs {
        let key = (run.model_id.clone(), run.use_prompt, run.use_anti_halluc);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);

        let condition_runs: Vec<&BenchmarkRunRecord> = runs
            .iter()
            .filter(|r| {
                r.model_id == run.model_id
                    && r.use_prompt == run.use_prompt
                    && r.use_anti_halluc == run.use_anti_halluc
            })
            .collect();

        let successful: Vec<&BenchmarkRunRecord> = condition_runs
            .iter()
            .filter(|r| r.error.is_none())
            .copied()
            .collect();

        let times_ms: Vec<u64> = successful.iter().map(|r| r.transcribe_time_ms).collect();
        let rtfs: Vec<f64> = successful.iter().map(|r| r.rtf).collect();

        let (time_mean, time_stdev) = if times_ms.is_empty() {
            (None, None)
        } else {
            let (m, s) = mean_stdev(&times_ms);
            (Some(m), Some(s))
        };

        let time_min = times_ms.iter().min().copied();
        let time_median = if times_ms.is_empty() {
            None
        } else {
            Some(median(&times_ms))
        };
        let rtf_median = if rtfs.is_empty() {
            None
        } else {
            let mut sorted = rtfs.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let n = sorted.len();
            Some(if n % 2 == 1 {
                sorted[n / 2]
            } else {
                (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
            })
        };

        let first_text = successful.first().map(|r| r.text.clone());
        let texts_identical = match first_text {
            Some(ref t) => successful.iter().all(|r| &r.text == t),
            None => true,
        };

        let first_error = condition_runs.iter().find_map(|r| r.error.clone());

        aggregates.push(BenchmarkAggregate {
            model_id: run.model_id.clone(),
            model_name: run.model_name.clone(),
            engine: run.engine.clone(),
            use_prompt: run.use_prompt,
            use_anti_halluc: run.use_anti_halluc,
            successful_runs: successful.len() as u32,
            time_min_ms: time_min,
            time_median_ms: time_median,
            time_mean_ms: time_mean,
            time_stdev_ms: time_stdev,
            rtf_median,
            texts_identical,
            first_error,
        });
    }
    aggregates
}

/// Compute the checkpoint file paths for a given input file. Per-input-file
/// naming prevents concurrent (sp1/sp2) invocations from clobbering each
/// other's checkpoints, and makes resume deterministic from the caller side.
fn checkpoint_paths(output_dir: &Path, input_file: &str) -> (PathBuf, PathBuf) {
    let stem = Path::new(input_file)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    let safe: String = stem
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let json = output_dir.join(format!("benchmark-checkpoint-{}.json", safe));
    let md = output_dir.join(format!("benchmark-checkpoint-{}.md", safe));
    (json, md)
}

/// Writes a partial JSON + MD snapshot to per-input-file checkpoint paths.
/// Called after every run so a crash at any point loses at most one run.
/// Caller provides pre-computed paths to amortize the stem-sanitize cost.
fn write_checkpoint(
    json_path: &Path,
    md_path: &Path,
    output_dir: &Path,
    file_path: &str,
    warmup_path: &Option<String>,
    audio_duration_s: f64,
    runs_per_condition: u32,
    runs: &[BenchmarkRunRecord],
) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    let aggregates = compute_aggregates(runs);
    let report = BenchmarkReport {
        timestamp: Local::now().to_rfc3339(),
        input_file: file_path.to_string(),
        warmup_file: warmup_path.clone(),
        audio_duration_s,
        runs_per_condition,
        runs: runs.to_vec(),
        aggregates,
    };
    std::fs::write(json_path, serde_json::to_string_pretty(&report)?)?;
    std::fs::write(md_path, build_markdown(&report))?;
    Ok(())
}

fn build_markdown(report: &BenchmarkReport) -> String {
    let mut md = String::new();
    md.push_str("# Transcription Benchmark\n\n");
    md.push_str(&format!("- **Timestamp:** {}\n", report.timestamp));
    md.push_str(&format!("- **Input:** `{}`\n", report.input_file));
    if let Some(ref w) = report.warmup_file {
        md.push_str(&format!("- **Warmup:** `{}`\n", w));
    }
    md.push_str(&format!(
        "- **Audio duration:** {:.2} s\n",
        report.audio_duration_s
    ));
    md.push_str(&format!(
        "- **Runs per condition:** {}\n\n",
        report.runs_per_condition
    ));

    md.push_str("## Aggregates\n\n");
    md.push_str("| Model | Engine | Prompt | AntiHalluc | OK/Total | Min (ms) | Median (ms) | Mean (ms) | Stdev (ms) | RTF (median) | Texts identical |\n");
    md.push_str("|---|---|---|---|---|---|---|---|---|---|---|\n");
    for agg in &report.aggregates {
        let prompt_label = if agg.use_prompt { "on" } else { "off" };
        let anti_label = if agg.use_anti_halluc { "on" } else { "off" };
        md.push_str(&format!(
            "| {} (`{}`) | {} | {} | {} | {}/{} | {} | {} | {} | {} | {} | {} |\n",
            agg.model_name,
            agg.model_id,
            agg.engine,
            prompt_label,
            anti_label,
            agg.successful_runs,
            report.runs_per_condition,
            agg.time_min_ms
                .map(|v| v.to_string())
                .unwrap_or_else(|| "—".to_string()),
            agg.time_median_ms
                .map(|v| v.to_string())
                .unwrap_or_else(|| "—".to_string()),
            agg.time_mean_ms
                .map(|v| format!("{:.0}", v))
                .unwrap_or_else(|| "—".to_string()),
            agg.time_stdev_ms
                .map(|v| format!("{:.0}", v))
                .unwrap_or_else(|| "—".to_string()),
            agg.rtf_median
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "—".to_string()),
            if agg.texts_identical { "yes" } else { "no" },
        ));
    }

    md.push_str("\n## Runs\n\n");
    md.push_str("| # | Model | Prompt | AntiHalluc | Run | Chunks | MaxChunk (s) | Time (ms) | RTF | Result |\n");
    md.push_str("|---|---|---|---|---|---|---|---|---|---|\n");
    for (idx, run) in report.runs.iter().enumerate() {
        let prompt_label = if run.use_prompt { "on" } else { "off" };
        let anti_label = if run.use_anti_halluc { "on" } else { "off" };
        let result = match &run.error {
            Some(e) => format!("ERROR: {}", e),
            None => {
                let preview: String = run.text.chars().take(120).collect();
                if run.text.chars().count() > 120 {
                    format!("{}…", preview)
                } else {
                    preview
                }
            }
        };
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {:.3} | {} |\n",
            idx + 1,
            run.model_id,
            prompt_label,
            anti_label,
            run.run_idx,
            run.chunk_count,
            run.max_chunk_secs
                .map(|v| format!("{:.1}", v))
                .unwrap_or_else(|| "—".to_string()),
            run.transcribe_time_ms,
            run.rtf,
            result.replace('|', "\\|").replace('\n', " "),
        ));
    }

    md.push_str("\n## Transcripts (per condition)\n\n");
    let mut seen: Vec<(String, bool, bool)> = Vec::new();
    for run in &report.runs {
        let key = (run.model_id.clone(), run.use_prompt, run.use_anti_halluc);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key.clone());

        let prompt_label = if run.use_prompt { "on" } else { "off" };
        let anti_label = if run.use_anti_halluc { "on" } else { "off" };
        md.push_str(&format!(
            "### {} (`{}`) — prompt: {} · anti-halluc: {}\n\n",
            run.model_name, run.model_id, prompt_label, anti_label
        ));
        if run.use_prompt {
            if let Some(ref p) = run.transcription_prompt {
                md.push_str(&format!("**Prompt:** {}\n\n", p));
            }
            if !run.custom_words.is_empty() {
                md.push_str(&format!(
                    "**Custom words:** {}\n\n",
                    run.custom_words.join(", ")
                ));
            }
        }

        for r in report
            .runs
            .iter()
            .filter(|r| r.model_id == key.0 && r.use_prompt == key.1 && r.use_anti_halluc == key.2)
        {
            md.push_str(&format!("**Run {}:**\n\n", r.run_idx));
            if let Some(ref e) = r.error {
                md.push_str(&format!("> ERROR: {}\n\n", e));
            } else if r.text.is_empty() {
                md.push_str("> _(empty)_\n\n");
            } else {
                for line in r.text.lines() {
                    md.push_str(&format!("> {}\n", line));
                }
                md.push('\n');
            }
        }
    }

    md
}

#[tauri::command]
#[specta::specta]
pub async fn benchmark_transcription_file(
    app: AppHandle,
    model_manager: State<'_, Arc<ModelManager>>,
    file_path: String,
    warmup_path: Option<String>,
    runs_per_condition: Option<u32>,
    skip_models: Option<Vec<String>>,
    max_chunk_secs_override: Option<f32>,
    language: Option<String>,
    overrides: Option<BenchmarkOverrides>,
) -> Result<String, String> {
    let overrides = overrides.unwrap_or_default();
    let prompt_override = overrides.prompt.clone();
    let skip_no_prompt = overrides.skip_no_prompt.unwrap_or(false);
    let sot_lang_tokens_override = overrides.sot_lang_tokens.clone();
    let runs_per_condition = runs_per_condition.unwrap_or(3).max(1);
    let run_idx_offset = overrides.run_idx_offset.unwrap_or(0);
    let skip_set: std::collections::HashSet<String> =
        skip_models.unwrap_or_default().into_iter().collect();
    let language = language.unwrap_or_else(|| "ru".to_string());

    // Batch mode: audio_batch is Some(non-empty). file_path is a label only
    // (checkpoint naming + report input_file) — its existence is not checked.
    // Single-file mode: file_path must exist and is loaded as the sole audio.
    let batch_mode = overrides
        .audio_batch
        .as_ref()
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    let input_path = PathBuf::from(&file_path);
    if !batch_mode && !input_path.exists() {
        return Err(format!("Input file not found: {}", file_path));
    }

    let output_dir_path = if batch_mode {
        // In batch mode the label may not be a real path. Anchor checkpoints
        // and final report next to the first batch audio file's parent (or
        // CWD if that fails) so eval scripts can find them deterministically.
        overrides
            .audio_batch
            .as_ref()
            .and_then(|b| b.first())
            .and_then(|p| Path::new(p).parent().map(|x| x.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        input_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    };

    // Per-input-file checkpoint paths — computed once, used per-run below.
    let (ckpt_json_path, ckpt_md_path) = checkpoint_paths(&output_dir_path, &file_path);

    info!(
        "benchmark: starting — input={} warmup={:?} runs={} skip={:?} max_chunk_override={:?} language={} prompt_override={} skip_no_prompt={} sot_lang_tokens_override={:?}",
        file_path,
        warmup_path,
        runs_per_condition,
        skip_set,
        max_chunk_secs_override,
        language,
        prompt_override
            .as_ref()
            .map(|p| format!("{}…{} chars", &p.chars().take(40).collect::<String>(), p.chars().count()))
            .unwrap_or_else(|| "none".to_string()),
        skip_no_prompt,
        sot_lang_tokens_override,
    );

    // Unified audio inventory: in single-file mode = 1 entry; in batch mode =
    // N entries from overrides.audio_batch. Each entry: (audio_file label,
    // mono 16k samples, duration_s). audio_file is None for single-file (so
    // legacy records keep `audio_file: null`); Some(filename) for batch.
    let audio_files: Vec<(Option<String>, Vec<f32>, f64)> = if batch_mode {
        let batch = overrides.audio_batch.as_ref().expect("batch_mode set");
        let mut out = Vec::with_capacity(batch.len());
        for path in batch {
            let p = Path::new(path);
            let samples = load_wav_mono_16k(p)
                .map_err(|e| format!("audio_batch file {} load failed: {}", path, e))?;
            let dur = samples.len() as f64 / TARGET_SAMPLE_RATE as f64;
            let fname = p
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| path.clone());
            out.push((Some(fname), samples, dur));
        }
        out
    } else {
        let samples = load_wav_mono_16k(&input_path).map_err(|e| e.to_string())?;
        let dur = samples.len() as f64 / TARGET_SAMPLE_RATE as f64;
        vec![(None, samples, dur)]
    };
    // Total duration across the batch (for the report's audio_duration_s
    // header). Per-record RTF uses each file's own duration below.
    let audio_duration_s: f64 = audio_files.iter().map(|(_, _, d)| d).sum();

    let audio_warmup = match &warmup_path {
        Some(p) => {
            Some(load_wav_mono_16k(Path::new(p)).map_err(|e| format!("warmup WAV failed: {}", e))?)
        }
        None => None,
    };

    let original_settings = get_settings(&app);
    let _settings_guard = SettingsGuard {
        app: app.clone(),
        original: original_settings.clone(),
    };

    {
        let mut s = original_settings.clone();
        s.selected_language = language.clone();
        s.translate_to_english = false;
        if s.model_unload_timeout == ModelUnloadTimeout::Immediately {
            s.model_unload_timeout = ModelUnloadTimeout::Min15;
        }
        write_settings(&app, s);
    }

    let transcription_manager = app.state::<Arc<TranscriptionManager>>();

    // Resume-from-checkpoint: seed `runs` from a prior partial report (if
    // overrides.resume_from was provided and points to a readable+parseable
    // file). `completed_keys` holds the per-run tuples we should skip below.
    // Errored runs from the prior report are dropped entirely so they retry.
    // Missing/corrupt file → fresh start with a warning (caller-friendly).
    //
    // Key tuple must capture everything that could change between otherwise-
    // equivalent RUN_MATRIX row invocations: the prompt string actually used
    // (V1/V2/V3/V4 differ but same use_prompt=true) and the top-level
    // `language` arg (ru vs auto are different conditions per eval.py
    // naming scheme).
    type RunKey = (
        String,              // model_id
        bool,                // use_prompt
        Option<String>,      // effective_initial_prompt (distinguishes V1/V2/V3/V4 & custom_words)
        bool,                // use_anti_halluc
        Option<Vec<String>>, // sot_lang_tokens
        String,              // language (ru / auto / en / ...)
        Option<String>,      // audio_file (None in single-file mode; per-file in batch)
        u32,                 // run_idx
    );
    fn run_key(r: &BenchmarkRunRecord) -> RunKey {
        (
            r.model_id.clone(),
            r.use_prompt,
            r.effective_initial_prompt.clone(),
            r.use_anti_halluc,
            r.sot_lang_tokens.clone(),
            r.language.clone(),
            r.audio_file.clone(),
            r.run_idx,
        )
    }
    let (completed_keys, mut runs): (std::collections::HashSet<RunKey>, Vec<BenchmarkRunRecord>) =
        match overrides.resume_from.as_deref() {
            Some(path) if Path::new(path).exists() => {
                match std::fs::read_to_string(path)
                    .map_err(|e| e.to_string())
                    .and_then(|s| {
                        serde_json::from_str::<BenchmarkReport>(&s).map_err(|e| e.to_string())
                    }) {
                    Ok(prev) => {
                        let total = prev.runs.len();
                        let ok_runs: Vec<BenchmarkRunRecord> = prev
                            .runs
                            .into_iter()
                            .filter(|r| r.error.is_none())
                            .collect();
                        let errored = total - ok_runs.len();
                        let keys: std::collections::HashSet<RunKey> =
                            ok_runs.iter().map(run_key).collect();
                        info!(
                            "benchmark: resuming from {} ({} completed will skip, {} errored will retry)",
                            path,
                            keys.len(),
                            errored
                        );
                        (keys, ok_runs)
                    }
                    Err(e) => {
                        warn!(
                            "benchmark: resume_from {} exists but failed to parse ({}) — starting fresh",
                            path, e
                        );
                        (std::collections::HashSet::new(), Vec::new())
                    }
                }
            }
            Some(path) => {
                info!(
                    "benchmark: resume_from file not found ({}) — starting fresh",
                    path
                );
                (std::collections::HashSet::new(), Vec::new())
            }
            None => (std::collections::HashSet::new(), Vec::new()),
        };
    let mut previous_model: Option<String> = None;

    // Precompute only_conditions matching set once for O(1) per-row check.
    let only_conditions_ref = overrides.only_conditions.as_ref();
    for spec in RUN_MATRIX {
        if skip_set.contains(spec.model_id) {
            info!("benchmark: skipping {} (in skip_models)", spec.model_id);
            continue;
        }
        if skip_no_prompt && !spec.use_prompt {
            info!(
                "benchmark: skipping {} (use_prompt=false and skip_no_prompt is on)",
                spec.model_id
            );
            continue;
        }
        // only_conditions pin: spec must match one of the listed tuples.
        if let Some(list) = only_conditions_ref {
            let spec_sot: Option<Vec<String>> = spec
                .sot_lang_tokens
                .map(|arr| arr.iter().map(|s| s.to_string()).collect());
            let matches = list.iter().any(|c| {
                c.model_id == spec.model_id
                    && c.use_prompt == spec.use_prompt
                    && c.use_anti_halluc == spec.use_anti_halluc
                    && c.sot_lang_tokens == spec_sot
            });
            if !matches {
                debug!(
                    "benchmark: skipping {} use_prompt={} ah={} sot={:?} (not in only_conditions)",
                    spec.model_id, spec.use_prompt, spec.use_anti_halluc, spec_sot
                );
                continue;
            }
        }

        let model_info_opt = model_manager.get_model_info(spec.model_id);
        let model_name = model_info_opt
            .as_ref()
            .map(|m| m.name.clone())
            .unwrap_or_else(|| spec.model_id.to_string());

        let is_whisper = model_info_opt
            .as_ref()
            .map(|m| matches!(m.engine_type, EngineType::Whisper))
            .unwrap_or(false);

        // Non-Whisper → VAD-based chunked long-form. Whisper uses single-shot.
        let max_chunk_secs = if is_whisper {
            None
        } else {
            Some(max_chunk_secs_override.unwrap_or_else(|| default_max_chunk_secs(spec.model_id)))
        };
        let long_form_cfg = max_chunk_secs.map(|m| LongFormConfig {
            max_chunk_secs: m,
            ..LongFormConfig::default()
        });

        let is_downloaded = model_info_opt
            .as_ref()
            .map(|m| m.is_downloaded)
            .unwrap_or(false);
        if !is_downloaded {
            // Stamp one error record per (audio_file, run_idx) so the eval
            // driver can still attribute the failure per file in batch mode.
            for (audio_filename_opt, _, _) in &audio_files {
                for run_idx in run_idx_offset..(run_idx_offset + runs_per_condition) {
                    runs.push(BenchmarkRunRecord {
                        model_id: spec.model_id.to_string(),
                        model_name: model_name.clone(),
                        engine: spec.engine_label.to_string(),
                        use_prompt: spec.use_prompt,
                        use_anti_halluc: spec.use_anti_halluc,
                        run_idx,
                        language: language.clone(),
                        translate: false,
                        transcription_prompt: None,
                        custom_words: vec![],
                        transcribe_time_ms: 0,
                        rtf: 0.0,
                        text: String::new(),
                        error: Some("model not downloaded".to_string()),
                        chunk_count: 0,
                        max_chunk_secs,
                        sot_lang_tokens: None,
                        effective_initial_prompt: None,
                        effective_n_max_text_ctx: None,
                        effective_entropy_thold: None,
                        decoder_prompt_init_tokens: None,
                        audio_file: audio_filename_opt.clone(),
                    });
                }
            }
            continue;
        }

        if previous_model.as_deref() != Some(spec.model_id) {
            info!("benchmark: switching to model {}", spec.model_id);
            if let Err(e) = switch_active_model(&app, spec.model_id) {
                for (audio_filename_opt, _, _) in &audio_files {
                    for run_idx in run_idx_offset..(run_idx_offset + runs_per_condition) {
                        runs.push(BenchmarkRunRecord {
                            model_id: spec.model_id.to_string(),
                            model_name: model_name.clone(),
                            engine: spec.engine_label.to_string(),
                            use_prompt: spec.use_prompt,
                            use_anti_halluc: spec.use_anti_halluc,
                            run_idx,
                            language: language.clone(),
                            translate: false,
                            transcription_prompt: None,
                            custom_words: vec![],
                            transcribe_time_ms: 0,
                            rtf: 0.0,
                            text: String::new(),
                            error: Some(format!("switch_active_model failed: {}", e)),
                            chunk_count: 0,
                            max_chunk_secs,
                            sot_lang_tokens: None,
                            effective_initial_prompt: None,
                            effective_n_max_text_ctx: None,
                            effective_entropy_thold: None,
                            decoder_prompt_init_tokens: None,
                            audio_file: audio_filename_opt.clone(),
                        });
                    }
                }
                continue;
            }

            // Warmup. For non-Whisper engines we MUST use long-form path so
            // the ONNX engine sees an input size it can handle; warmup-file may
            // still be short enough for a single call but long-form handles both.
            let warm_audio = audio_warmup.as_ref().cloned().unwrap_or_else(|| {
                let first = &audio_files[0].1;
                first
                    .iter()
                    .take((TARGET_SAMPLE_RATE as usize * 3).min(first.len()))
                    .copied()
                    .collect::<Vec<f32>>()
            });
            if let Some(ref cfg) = long_form_cfg {
                info!(
                    "benchmark: warmup (long-form, max_chunk={:.1}s) for {}",
                    cfg.max_chunk_secs, spec.model_id
                );
                let _ = transcription_manager.transcribe_long_form(warm_audio, cfg.clone());
            } else {
                info!("benchmark: warmup (single-shot) for {}", spec.model_id);
                let _ = transcription_manager.transcribe(warm_audio);
            }

            previous_model = Some(spec.model_id.to_string());
        }

        let mut s = get_settings(&app);
        if is_whisper && spec.use_prompt {
            // If prompt_override is provided, use it verbatim and clear
            // custom_words so the test isolates the punctuation prompt.
            // Otherwise fall back to whatever was in Handy's UI settings.
            if let Some(ref p) = prompt_override {
                s.transcription_prompt = Some(p.clone());
                s.custom_words = vec![];
            } else {
                s.transcription_prompt = original_settings.transcription_prompt.clone();
                s.custom_words = original_settings.custom_words.clone();
            }
        } else {
            s.transcription_prompt = None;
            s.custom_words = vec![];
        }
        // Anti-hallucination: enabled only when spec requests AND it's a
        // Whisper-based engine (for non-Whisper the flag is ignored anyway,
        // but we keep settings clean).
        s.whisper_anti_hallucination = is_whisper && spec.use_anti_halluc;
        // LID-hack: per-run override takes precedence over the spec, so a
        // DevTools invocation can flip sot_lang_tokens on/off for an entire
        // RUN_MATRIX pass without editing the constant. When both are None
        // the existing SettingsGuard Drop still restores the pre-benchmark
        // value, so there's no permanent setting mutation.
        s.whisper_sot_lang_tokens = sot_lang_tokens_override.clone().or_else(|| {
            spec.sot_lang_tokens
                .map(|a| a.iter().map(|s| s.to_string()).collect())
        });
        write_settings(&app, s.clone());

        let applied_prompt = s.transcription_prompt.clone();
        let applied_custom_words = s.custom_words.clone();
        let applied_sot_lang_tokens = s.whisper_sot_lang_tokens.clone();
        // Provenance: stamp every record with what actually reached whisper.cpp,
        // not just the RunSpec flags. `compute_effective_initial_prompt` mirrors
        // the concat in `managers/transcription.rs`, so record + decoder stay
        // in lock-step without an extra round-trip through the engine.
        let effective_initial_prompt = compute_effective_initial_prompt(&s);
        let (effective_n_max_text_ctx, effective_entropy_thold) =
            if is_whisper && spec.use_anti_halluc {
                (
                    Some(ANTI_HALLUC_N_MAX_TEXT_CTX),
                    Some(ANTI_HALLUC_ENTROPY_THOLD),
                )
            } else {
                (None, None)
            };

        // Audio batch loop: each (audio_file, samples, dur) is transcribed
        // N times before moving to the next file. Model loads ONCE per spec
        // (above), so per-file warmup is unnecessary — model state is hot.
        for (audio_filename_opt, audio_samples, audio_dur) in &audio_files {
            for run_idx in run_idx_offset..(run_idx_offset + runs_per_condition) {
                // Resume short-circuit: skip if this full tuple is already in the
                // seeded completed-set. Audio file is part of the key so the
                // same (spec, run_idx) on different audio files counts as
                // distinct work units.
                let key: RunKey = (
                    spec.model_id.to_string(),
                    spec.use_prompt,
                    effective_initial_prompt.clone(),
                    spec.use_anti_halluc,
                    applied_sot_lang_tokens.clone(),
                    language.clone(),
                    audio_filename_opt.clone(),
                    run_idx,
                );
                if completed_keys.contains(&key) {
                    debug!(
                        "benchmark: resume skip (already done) run {}/{} model={} audio={:?} prompt={:?} sot={:?} lang={}",
                        run_idx + 1,
                        runs_per_condition,
                        spec.model_id,
                        audio_filename_opt,
                        effective_initial_prompt.as_ref().map(|s| s.chars().take(40).collect::<String>()),
                        applied_sot_lang_tokens,
                        language
                    );
                    continue;
                }

                info!(
                    "benchmark: measure run {}/{} model={} audio={:?} prompt={} anti_halluc={} path={}",
                    run_idx + 1,
                    runs_per_condition,
                    spec.model_id,
                    audio_filename_opt,
                    spec.use_prompt,
                    spec.use_anti_halluc,
                    if long_form_cfg.is_some() { "long-form" } else { "single-shot" }
                );

                let start = Instant::now();
                let (text_result, chunks): (Result<String>, u32) = match &long_form_cfg {
                    Some(cfg) => match transcription_manager
                        .transcribe_long_form(audio_samples.clone(), cfg.clone())
                    {
                        Ok(r) => (Ok(r.text), r.chunk_count),
                        Err(e) => (Err(e), 0),
                    },
                    None => match transcription_manager.transcribe(audio_samples.clone()) {
                        Ok(t) => (Ok(t), 1),
                        Err(e) => (Err(e), 0),
                    },
                };
                let elapsed_ms = start.elapsed().as_millis() as u64;

                // Pull the SOT prompt_init tokens captured by TranscriptionManager
                // from the decoder's state (LID-hack provenance). Consuming take:
                // each run owns its snapshot; next iteration re-captures or clears.
                let decoder_prompt_init_tokens =
                    transcription_manager.take_last_whisper_prompt_init();

                let record = match text_result {
                    Ok(text) => BenchmarkRunRecord {
                        model_id: spec.model_id.to_string(),
                        model_name: model_name.clone(),
                        engine: spec.engine_label.to_string(),
                        use_prompt: spec.use_prompt,
                        use_anti_halluc: spec.use_anti_halluc,
                        run_idx,
                        language: language.clone(),
                        translate: false,
                        transcription_prompt: applied_prompt.clone(),
                        custom_words: applied_custom_words.clone(),
                        transcribe_time_ms: elapsed_ms,
                        rtf: if *audio_dur > 0.0 {
                            elapsed_ms as f64 / (audio_dur * 1000.0)
                        } else {
                            0.0
                        },
                        text,
                        error: None,
                        chunk_count: chunks,
                        max_chunk_secs,
                        sot_lang_tokens: applied_sot_lang_tokens.clone(),
                        effective_initial_prompt: effective_initial_prompt.clone(),
                        effective_n_max_text_ctx,
                        effective_entropy_thold,
                        decoder_prompt_init_tokens: decoder_prompt_init_tokens.clone(),
                        audio_file: audio_filename_opt.clone(),
                    },
                    Err(e) => BenchmarkRunRecord {
                        model_id: spec.model_id.to_string(),
                        model_name: model_name.clone(),
                        engine: spec.engine_label.to_string(),
                        use_prompt: spec.use_prompt,
                        use_anti_halluc: spec.use_anti_halluc,
                        run_idx,
                        language: language.clone(),
                        translate: false,
                        transcription_prompt: applied_prompt.clone(),
                        custom_words: applied_custom_words.clone(),
                        transcribe_time_ms: elapsed_ms,
                        rtf: 0.0,
                        text: String::new(),
                        error: Some(e.to_string()),
                        chunk_count: chunks,
                        max_chunk_secs,
                        sot_lang_tokens: applied_sot_lang_tokens.clone(),
                        effective_initial_prompt: effective_initial_prompt.clone(),
                        effective_n_max_text_ctx,
                        effective_entropy_thold,
                        decoder_prompt_init_tokens,
                        audio_file: audio_filename_opt.clone(),
                    },
                };
                runs.push(record);

                // Per-run checkpoint. Keeps the crash-loss bounded to the single
                // in-flight transcribe (the new record is already in `runs` by
                // this point, so it survives too). Overwrites the same per-input-
                // file path each time. ~50ms write overhead vs ~50s transcribe —
                // negligible.
                if let Err(e) = write_checkpoint(
                    &ckpt_json_path,
                    &ckpt_md_path,
                    &output_dir_path,
                    &file_path,
                    &warmup_path,
                    audio_duration_s,
                    runs_per_condition,
                    &runs,
                ) {
                    log::warn!(
                        "benchmark: failed to write checkpoint after run {}/{} model {} audio {:?}: {}",
                        run_idx + 1,
                        runs_per_condition,
                        spec.model_id,
                        audio_filename_opt,
                        e
                    );
                } else {
                    debug!(
                        "benchmark: checkpoint written after run {}/{} model {} audio {:?}",
                        run_idx + 1,
                        runs_per_condition,
                        spec.model_id,
                        audio_filename_opt
                    );
                }
            }
        }
    }

    let aggregates = compute_aggregates(&runs);

    let timestamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let report = BenchmarkReport {
        timestamp: Local::now().to_rfc3339(),
        input_file: file_path.clone(),
        warmup_file: warmup_path.clone(),
        audio_duration_s,
        runs_per_condition,
        runs,
        aggregates,
    };

    std::fs::create_dir_all(&output_dir_path)
        .map_err(|e| format!("Failed to create output dir: {}", e))?;

    let json_path = output_dir_path.join(format!("benchmark-results-{}.json", timestamp));
    let md_path = output_dir_path.join(format!("benchmark-results-{}.md", timestamp));

    let json_text = serde_json::to_string_pretty(&report)
        .map_err(|e| format!("Failed to serialize report: {}", e))?;
    std::fs::write(&json_path, json_text).map_err(|e| format!("Failed to write JSON: {}", e))?;

    let md_text = build_markdown(&report);
    std::fs::write(&md_path, md_text).map_err(|e| format!("Failed to write MD: {}", e))?;

    // Success → remove the checkpoint files so the next invocation for this
    // input_file starts fresh (not resume-from-complete). Missing-file errors
    // are benign: user may have aborted mid-checkpoint or never resumed.
    if let Err(e) = std::fs::remove_file(&ckpt_json_path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            warn!(
                "benchmark: failed to remove checkpoint JSON {}: {}",
                ckpt_json_path.display(),
                e
            );
        }
    }
    if let Err(e) = std::fs::remove_file(&ckpt_md_path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            warn!(
                "benchmark: failed to remove checkpoint MD {}: {}",
                ckpt_md_path.display(),
                e
            );
        }
    }

    info!(
        "benchmark: done — {} runs, report written to {} and {}",
        report.runs.len(),
        json_path.display(),
        md_path.display()
    );

    Ok(json_path.to_string_lossy().to_string())
}
