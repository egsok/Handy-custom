# Benchmark Harness — Handoff

**Branch:** `bench/whisper-matrix+lid-hack` (от `bench/whisper-matrix`)
**Last handoff:** 2026-04-22 (LID-hack landed + validated)

## What this is

Tauri команда `benchmark_transcription_file` внутри Handy прогоняет один WAV
через матрицу (model × condition) для сравнения качества (WER/CER/пунктуация)
и скорости транскрипции. Вызов — из DevTools запущенного `tauri dev`. Нет UI.

## Current state

### Branch & commits

- Активная ветка `bench/whisper-matrix+lid-hack` (от `bench/whisper-matrix`).
- Ветка `bench/whisper-matrix` закоммичена и чистая — LID-hack идёт как
  feature-branch сверху.
- 5 коммитов LID-hack (от `d941569`):
  - `90e2fec` chore(deps): pin whisper-rs and whisper-rs-sys to local LID-hack forks
  - `b7c73c6` feat(settings): add whisper_sot_lang_tokens field to AppSettings
  - `29a92f2` feat(transcription): resolve language codes and pass sot_lang_tokens into WhisperInferenceParams
  - `9e245ba` feat(benchmark): plumb sot_lang_tokens into RunSpec, BenchmarkRunRecord, and command signature
  - `af90f01` feat(benchmark): add LID-hack RUN_MATRIX rows for whisper-podlodka-turbo

### Fork chain (new, required)

LID-hack требует патчей в 3 upstream-крейтах. Все они локально:

| Крейт | Путь | Ветки | Роль |
|---|---|---|---|
| whisper-rs-sys | `D:/dev/whisper-rs-sys-fork` | `main` vanilla / `feature/sot-lang-tokens` | Вендорит whisper.cpp 1.8.3 (де-сабмодулено). `feature` ветка добавляет FFI-поля `sot_lang_tokens`/`sot_n_lang_tokens` в `whisper_full_params` + патчит оба SOT-пути (`whisper_full_with_state` + DTW) чтобы пушить concatenated lang-токены вместо одиночного. Pre-release версия `0.15.1-lid-hack.1`. |
| whisper-rs | `D:/dev/whisper-rs-fork` | `master` / `feature/sot-lang-tokens` | Клонирован с https://codeberg.org/tazz4843/whisper-rs (оригинальный GitHub репо stale). `feature` добавляет `FullParams::set_sot_lang_tokens(&[i32])` (mirror of `set_tokens`) и `WhisperContext::lang_token_id(&self, code: &str) -> Option<i32>`. Version остаётся `0.16.0` — bump до pre-release сломал бы cargo `[patch.crates-io]` resolution. |
| transcribe-rs | `D:/dev/transcribe-rs-fork` | `main` / `feature/sot-lang-tokens` | Был tarball extraction без git — сейчас `git init` + import baseline commit включающий pre-existing in-place `n_max_text_ctx`/`entropy_thold` работу. `feature` добавляет `WhisperInferenceParams::sot_lang_tokens: Option<Vec<i32>>` + wiring + `WhisperEngine::ctx_lang_token_id` helper. |

Все три fork'а должны быть на `feature/sot-lang-tokens` одновременно чтобы
Handy-бинарь имел LID-hack. На `main`/`master` все три fork'а — vanilla
baseline (build-infrastructure changes только), чтобы можно было откатить
в любой момент.

Handy's `src-tauri/Cargo.toml` patches-в:
```toml
transcribe-rs = { path = "../../transcribe-rs-fork" }
whisper-rs = { path = "../../whisper-rs-fork" }            # LID-hack
whisper-rs-sys = { path = "../../whisper-rs-sys-fork" }    # LID-hack
```
**КРИТИЧНО:** cargo `[patch.crates-io]` не transitive между workspace'ами.
transcribe-rs-fork declarates свои patches внутри, но Handy должен повторить
их у себя, иначе при resolving cargo возьмёт registry versions и патчи
проигнорируются.

### Installed models in Handy's app data

`C:\Users\Egor Sokolov\AppData\Roaming\com.pais.handy\models\`:

| Файл | model_id | Источник | Формат | Размер |
|---|---|---|---|---|
| `whisper-medium-q4_1.bin` | `medium` | стоковый | q4_1 | 470 MB |
| `ggml-large-v3-q5_0.bin` | `large` | стоковый | q5_0 | 1.1 GB |
| `ggml-large-v3-turbo.bin` | `turbo` | стоковый | f16 | 1.6 GB |
| `breeze-asr-q5_k.bin` | `breeze-asr` | стоковый | q5_k | 1.1 GB |
| `whisper-podlodka-turbo.bin` | `whisper-podlodka-turbo` | наша конверсия bond005/whisper-podlodka-turbo | f16 | 1.6 GB |
| `whisper-large-v3-russian.bin` | `whisper-large-v3-russian` | наша конверсия antony66/whisper-large-v3-russian | f16 | 2.9 GB |
| `ggml-large-v3.bin` | `ggml-large-v3` | ggerganov/whisper.cpp (скачан как есть) | f16 | 2.9 GB |
| `ggml-medium.bin` | `ggml-medium` | ggerganov/whisper.cpp (скачан как есть) | f16 | 1.5 GB |
| `parakeet-tdt-0.6b-v3-int8/` | `parakeet-tdt-0.6b-v3` | стоковый | int8 | 451 MB |
| `canary-1b-v2/` | `canary-1b-v2` | стоковый | int8 | 691 MB |
| `giga-am-v3-int8/` | `gigaam-v3-e2e-ctc` | стоковый | int8 | 151 MB |
| `cohere-int8/` | `cohere-int8` | стоковый (не в матрице) | int8 | 1.7 GB |

**Важно:** стоковые `large`/`medium`/`breeze-asr` квантизированы (q5_0/q4_1/q5_k);
новые custom — f16. Это **не apples-to-apples**. Пользователь это знает.
4 новые модели зарегистрированы через `discover_custom_whisper_models()` в
`src-tauri/src/managers/model.rs:818` (`is_custom: true`, `supported_languages: []`).

### Known runtime issues

1. **`ggml-medium.bin` крашится во время транскрипции** на Vulkan этого GPU.
   `STATUS_STACK_BUFFER_OVERRUN` (0xc0000409), "Rust cannot catch foreign
   exceptions". Модель ЗАГРУЖАЕТСЯ успешно (логи показывают Vulkan0 backend
   init, 1533 MB), но падает на первом `whisper_full()`. Известно по двум
   прогонам на разных аудио. Офиц. `ggml-medium.bin` от ggerganov — не
   наша конверсия. Не OOM: 2.9 GB `ggml-large-v3` и `whisper-large-v3-russian`
   работают без проблем на той же конфигурации.
2. **Limtech's pre-converted `whisper-large-v3-russian-ggml`** тоже крашил
   на аналогичном C++ exception. Заменён на нашу конверсию antony66
   safetensors через `convert-h5-to-ggml.py` — работает.
3. **Конверсия HF safetensors → GGML требует `torch_dtype=torch.float16`
   в `from_pretrained(...)`** — иначе segfault в transformers 4.55
   на meta-tensor loading (`_load_state_dict_into_meta_model`).
   Патч уже применён к `D:\dev\whisper-podlodka-convert\whisper.cpp\models\convert-h5-to-ggml.py`.

### Benchmark command signature (current, after LID-hack landing)

**BREAKING CHANGE** from prior handoff: `promptOverride` и `skipNoPrompt`
больше не top-level аргументы — они внутри `overrides: BenchmarkOverrides`
struct. Пришлось из-за `tauri-specta`'s `SpectaFn` trait arity cap = 10
(добавление `sot_lang_tokens_override` сделало бы 11-й параметр).

```rust
pub async fn benchmark_transcription_file(
    app: AppHandle,
    model_manager: State<'_, Arc<ModelManager>>,
    file_path: String,
    warmup_path: Option<String>,
    runs_per_condition: Option<u32>,       // default 3
    skip_models: Option<Vec<String>>,
    max_chunk_secs_override: Option<f32>,
    language: Option<String>,              // default "ru"; pass "auto" for auto-detect
    overrides: Option<BenchmarkOverrides>, // bundled — was 3 separate top-level params
) -> Result<String, String>                // returns path to final JSON report

pub struct BenchmarkOverrides {
    pub prompt: Option<String>,            // replaces settings.transcription_prompt
                                           // for use_prompt=true rows, clears custom_words
    pub skip_no_prompt: Option<bool>,      // skips RunSpec entries with use_prompt=false
    pub sot_lang_tokens: Option<Vec<String>>,  // NEW — LID-hack: concatenated lang
                                           // tokens like ["ru","en"]. When Some, overrides
                                           // RunSpec.sot_lang_tokens for ALL rows.
}
```

Invocation args (JS camelCase): `filePath`, `warmupPath`, `runsPerCondition`,
`skipModels`, `maxChunkSecsOverride`, `language`, `overrides`. Внутри
`overrides` — camelCase: `prompt`, `skipNoPrompt`, `sotLangTokens`.

### LID-hack semantics

- `RunSpec` теперь имеет поле `sot_lang_tokens: Option<&'static [&'static str]>`.
- В RUN_MATRIX 2 новых строки для `whisper-podlodka-turbo`:
  `Some(&["ru","en"])` и `Some(&["en","ru"])` — остальные поля baseline.
- `BenchmarkRunRecord.sot_lang_tokens: Option<Vec<String>>` — попадает в JSON
  отчёт как `"sot_lang_tokens": null` или `["ru","en"]` etc.
- Per-run mutation (в `benchmark.rs:699-719`):
  ```rust
  s.whisper_sot_lang_tokens = overrides
      .as_ref()
      .and_then(|o| o.sot_lang_tokens.clone())
      .or_else(|| spec.sot_lang_tokens.map(|a| a.iter().map(|s| s.to_string()).collect()));
  ```
  Override > RunSpec > None. `SettingsGuard` восстанавливает на drop.
- Код-resolver (`managers/transcription.rs:554-588`):
  ```rust
  sot_lang_tokens: settings.whisper_sot_lang_tokens.as_ref().and_then(|codes| {
      let resolved: Vec<i32> = codes.iter()
          .filter_map(|c| whisper_engine.ctx_lang_token_id(c))
          .collect();
      (!resolved.is_empty()).then_some(resolved)
  })
  ```
  Строковые ISO коды (`"ru"`, `"en"`) → whisper token IDs через FFI. Если ни
  один код не резолвится — `None`, zero-regression.

### LID-hack validation (2026-04-22, Stage 1)

Отчёт: `benchmark-results-20260422-171442.json/.md`.
Входной WAV: `01-260420_2148-01.wav` (warmup, ~30s русский).
6 прогонов × `whisper-podlodka-turbo`:

| Row | sot_lang_tokens | len | transcribe_ms | Код-свитч сигнал |
|---:|---|---:|---:|---|
| 0 | null | 1893 | 1330 | `Телеграм` (Cyrillic), `Google Calendar` |
| 1 | null (+prompt) | 1885 | 1331 | `Telegram`, `Google календар` |
| 2 | null (+anti-halluc) | 1881 | 1353 | `Telegram`, `Google календар` |
| 3 | null (+both) | 1873 | 1259 | `Telegram`, `Google календар` |
| 4 | `["ru","en"]` | 1875 | **2309** (1.7×) | `Telegram` (Latin!), `Google Calendar` |
| 5 | `["en","ru"]` | 1863 | **4245** (3.2×) | `Telegram`, `Google Calendar` |

Три независимых сигнала подтверждают патчи доходят до декодера:
1. Latency spike 1.7× / 3.2× на hack-rows — невозможен без реального прохода
   concatenated токенов через декодер.
2. Token-choice divergence: без prompt (Row 0) podlodka выдавал `Телеграм`
   Cyrillic; с hack (Row 4) — `Telegram` Latin. LID hack induces code-
   switching на token level без initial_prompt.
3. JSON serialization поля `sot_lang_tokens` корректна per-row.

Zero-regression: Rows 0-3 (sot_lang_tokens=None) совпадают со стилем prior
benchmark'ов.

Длина transcript'а почти одинаковая (1-2%), потому что warmup аудио короткое
и monolingual. Stage 2 (sp1 ~850s) воспроизвёл бы +37% сигнал из Python pilot
на podlodka + `["en","ru"]` — не запущено в этой сессии.

### Conversion toolchain state

Рабочая директория: `D:\dev\whisper-podlodka-convert\`

- `whisper.cpp/` — склонирован, converter script `models/convert-h5-to-ggml.py`
  пропатчен (`torch_dtype=torch.float16`)
- `openai-whisper/` — склонирован, нужен для `whisper/assets/mel_filters.npz`
  (содержит `mel_80` и `mel_128`)
- `whisper-podlodka-turbo-hf/` — скачанный HF source (3.1 GB safetensors)
- `whisper-large-v3-russian-hf/` — скачанный HF source (3.1 GB safetensors)
- `ggml-standard-large-v3/` и `ggml-standard-medium/` — папки, из которых
  файлы уже перемещены в Handy's models dir; пустые кроме .cache

Python venv с torch 2.6 + transformers 4.55 + huggingface_hub 0.34 уже есть:
`C:\ASR\.venv\Scripts\python.exe` (и `hf.exe`, `ct2-transformers-converter.exe` там же).

**Conversion procedure** (для добавления новой HF-модели):
```bash
# 1. Download
cd /d/dev/whisper-podlodka-convert
PYTHONIOENCODING=utf-8 PYTHONUTF8=1 "C:/ASR/.venv/Scripts/hf.exe" download \
  <hf-org>/<model-name> --local-dir ./<model-name>-hf

# 2. Convert
PYTHONIOENCODING=utf-8 PYTHONUTF8=1 "C:/ASR/.venv/Scripts/python.exe" \
  ./whisper.cpp/models/convert-h5-to-ggml.py \
  ./<model-name>-hf ./openai-whisper ./out

# 3. Install
mv ./out/ggml-model.bin "C:/Users/Egor Sokolov/AppData/Roaming/com.pais.handy/models/<model-id>.bin"
# → auto-discovered as custom Whisper with model_id=<model-id>

# 4. Add RUN_MATRIX entries (4 per Whisper model: prompt × anti_halluc combos)
#    in src-tauri/src/commands/benchmark.rs
```

## Build environment gotcha (unchanged, critical)

Windows MSBuild hits `MAX_PATH` (260 chars) в whisper-rs-sys cmake nested
TryCompile dirs. **Always** `export CARGO_TARGET_DIR=D:/h` before cargo /
`bun run tauri dev`.

## Fork chain details

См. таблицу выше. Краткие notes per fork:

**whisper-rs-sys-fork** (`D:/dev/whisper-rs-sys-fork`):
- `main` = `526a5ee` — vanilla baseline: imported sys subcrate из
  tazz4843/whisper-rs (Codeberg @ master 129b9826), vendored whisper.cpp 1.8.3
  как regular directory (submodule deinit'ed), standalone Cargo.toml
  (rust-version inline, version = 0.15.0).
- `feature/sot-lang-tokens` = `e25fb7f` — 5 atomic commits:
  1. `82f7ae1` whisper.h add sot_lang_tokens/sot_n_lang_tokens fields
  2. `261351c` whisper.cpp default-init nullptr/0
  3. `ca18d44` whisper.cpp primary SOT path (whisper_full_with_state)
  4. `c3f7adb` whisper.cpp DTW path (whisper_exp_compute_token_level_timestamps_dtw)
  5. `e25fb7f` version bump to 0.15.1-lid-hack.1

**whisper-rs-fork** (`D:/dev/whisper-rs-fork`):
- `master` = `93ed595` — vanilla baseline: repoint root Cargo.toml
  `whisper-rs-sys` dep к path `../whisper-rs-sys-fork` (no version constraint,
  path-only, чтобы работало и с main vanilla и с feature/sot-lang-tokens
  sys-fork). Removed `[workspace] members = ["sys"]` — sys subcrate больше
  не активный.
- `feature/sot-lang-tokens` = `93421f9` — 2 commits:
  1. `6fb2a79` `FullParams::set_sot_lang_tokens(&[i32])` на `whisper_params.rs`
     (зеркалит `set_tokens` pattern)
  2. `93421f9` `WhisperContext::lang_token_id(&self, code: &str) -> Option<i32>`
     (на `whisper_ctx.rs` + re-export через `whisper_ctx_wrapper.rs`)
- Version остаётся `0.16.0` — bump до pre-release сломал бы cargo patch
  resolution для transcribe-rs's dep `whisper-rs = "0.16.0"`.

**transcribe-rs-fork** (`D:/dev/transcribe-rs-fork`):
- Раньше был cargo-package tarball extraction без git. Сейчас `git init`'d.
- `main` = `cd1e227` — 2 commits:
  1. `8ce8f09` import baseline (vanilla 0.3.8 + pre-existing in-place
     `n_max_text_ctx`/`entropy_thold` customizations сфолжены в baseline)
  2. `cd1e227` vanilla [patch.crates-io] pin к локальным fork'ам (both
     на vanilla branches — this is all-vanilla gate)
- `feature/sot-lang-tokens` = `b154503` — 3 commits:
  1. `ee110c0` add `WhisperInferenceParams::sot_lang_tokens` field + Default
  2. `cb0fddd` wire `set_sot_lang_tokens` call в `infer()`
  3. `b154503` expose `WhisperEngine::ctx_lang_token_id` helper

**Verification matrix** (запускается из whisper-rs-fork):
- sys=main + wr=master → build green (all-vanilla gate)
- sys=main + wr=feature → fails as expected (wr-feature references not-yet-
  existent sys FFI fields — это diagnostic)
- sys=feature + wr=master → green (wr-master doesn't reference new fields)
- sys=feature + wr=feature → green (production combo)

**Upstream update procedure** (когда upstream `whisper-rs-sys` релизит новую
версию):
1. На `main` ветке sys-fork: cherry-pick upstream changes (или `git pull`
   если есть remote). Re-run vanilla build → commit.
2. `git checkout feature/sot-lang-tokens && git rebase main` — патчи должны
   apply-нуться cleanly т.к. они в hot areas whisper.cpp 6954-6965 и 8830-
   8835 + whisper.h struct и default initializer. Если upstream меняет SOT
   assembly — rebase conflict; resolve manually по той же логике (push
   concatenated array когда sot_lang_tokens non-NULL).
3. Rebuild gate matrix. Bump sys-fork version (e.g. `0.15.2-lid-hack.1`).
4. Тот же pattern на whisper-rs-fork и transcribe-rs-fork.

## Completed benchmark runs this session

### Speaker 1 — `Voice to text benchmark.wav` (~850s, русский)

Несколько отчётов в `C:\Users\Egor Sokolov\Documents\REAPER Media\`:
- `benchmark-results-20260421-*.md` (10 файлов) — разные прогоны со стоковыми +
  podlodka + v3-russian (не всегда все 4 модели из-за крэшей)

### Speaker 2 — `ASR benchmark Nastya.wav` (конвертирован из m4a, 16 kHz mono)

Отчёты с суффиксом `speaker2`:
- `benchmark-results-20260421-215110 speaker2.*`
- `benchmark-results-20260421-223305 speaker2.*`
- `benchmark-results-20260421-224323 speaker2.*`
- `benchmark-results-20260421-225134 speaker2.*`
- `benchmark-results-20260421-231417.*` (без speaker2-суффикса, но по timestamp — speaker 2)

### Speaker 3 — `28 Rue des Fossés Saint-Bernard.m4a` → `speaker3.wav` (13:17, 25 MB)

Отчёты с суффиксом `speaker3`:
- `benchmark-results-20260422-010413 speaker3.*`
- `benchmark-results-20260422-013528 speaker3.*`
- `benchmark-results-20260422-013657 speaker3.*`
- `benchmark-results-20260422-014119 speaker3.*`

WAV от speaker 3 в REAPER Media dir как `speaker3.wav` (оригинал m4a и WAV
с UTF-8-именем рядом). UTF-8 пути **не работают в ffmpeg на Windows** —
пришлось копировать в ASCII-имя.

### Checkpoints (важно)

- `benchmark-results-checkpoint.json/.md` — перезаписывается каждой invocation
- `benchmark-results-checkpoint-round1.json/.md` — сохранённая копия одного крэша

Checkpoint пишется после каждой модели — если крэш, данные предыдущих моделей
сохраняются. Финальный отчёт `benchmark-results-{timestamp}.json/.md` пишется
только при успешном завершении invocation.

## Pipeline helper pattern (JS in DevTools)

Помощник `runSpeakerNPipeline()` не хранится нигде — пастится в DevTools
консоль каждую сессию. Пример скелета для speaker N:

```js
window.runSpeakerNPipeline = async function() {
  const FILE = 'C:\\...\\speakerN.wav';  // 16 kHz mono PCM WAV
  const WARMUP = 'C:\\Users\\Egor Sokolov\\Documents\\REAPER Media\\01-260420_2148-01.wav';
  const V1 = 'Привет! Как дела? Он сказал: «Сделаем это сегодня — пока есть время». Конечно, не всё так просто, как кажется на первый взгляд; нужно принять во внимание погоду.';
  const V2 = 'Привет! Как дела? Наш English-speaking friend сказал: «Сделаем это сегодня — пока есть время». Мы выполняли эту разработку в Claude Code. Конечно, не всё так просто; нужно учесть погоду.';
  const NONWHISPER = ['parakeet-tdt-0.6b-v3','canary-1b-v2','gigaam-v3-e2e-ctc'];
  const WHISPER = ['breeze-asr','turbo','large','medium','whisper-podlodka-turbo','whisper-large-v3-russian','ggml-large-v3','ggml-medium'];
  const plan = [
    { label:'W-auto-v1', language:'auto', overrides:{prompt:V1, skipNoPrompt:false}, skipModels:NONWHISPER, runsPerCondition:3 },
    { label:'W-auto-v2', language:'auto', overrides:{prompt:V2, skipNoPrompt:true},  skipModels:NONWHISPER, runsPerCondition:3 },
    { label:'NW-auto-noCanary', language:'auto', skipModels:[...WHISPER,'canary-1b-v2'], runsPerCondition:1 },
    { label:'NW-ru', language:'ru', skipModels:WHISPER, runsPerCondition:1 },
    // LID-hack (Peng-style concatenated SOT tokens) — requires podlodka-turbo
    { label:'LID-hack-en-ru', language:'auto', overrides:{sotLangTokens:['en','ru']}, skipModels:[...WHISPER.filter(m=>m!=='whisper-podlodka-turbo'),...NONWHISPER], runsPerCondition:1 },
  ];
  const results = [];
  for (const [i, inv] of plan.entries()) {
    const args = { filePath:FILE, warmupPath:WARMUP, language:inv.language, skipModels:inv.skipModels, runsPerCondition:inv.runsPerCondition };
    if (inv.overrides !== undefined) args.overrides = inv.overrides;
    try {
      const path = await window.__TAURI_INTERNALS__.invoke('benchmark_transcription_file', args);
      results.push({ label: inv.label, path });
      console.log(`[${i+1}/${plan.length}] ${inv.label} → ${path}`);
    } catch (e) {
      results.push({ label: inv.label, error: String(e) });
      console.error(`[${i+1}/${plan.length}] ${inv.label} FAILED:`, e);
    }
  }
  console.table(results);
  return results;
};
```

## Critical user-behavior lessons

1. **НЕ правь `src-tauri/` пока запущен пользовательский benchmark.**
   `tauri dev` watch-ит и автоматически rebuild-ит Rust-код при изменении src —
   живой инстанс убивается посреди transcription. Пользователь обжёгся.
   Правило: любые code changes ДО старта user benchmark, либо — если уже
   запущено — ждать завершения.
2. **m4a путь через ffmpeg на Windows с UTF-8-именами не работает.**
   Решение: cp (через PowerShell, не через bash) в ASCII-имя, потом ffmpeg.
3. **Handy's benchmark accepts only WAV** (`hound::WavReader`).
   m4a/mp3/flac → ffmpeg preprocess.
4. **Пользователь предпочитает f16 (non-quantized) для честного сравнения.**
   Стоковые large/medium/breeze-asr квантизированы — асимметрия известна,
   пока терпится. breeze-asr конвертнуть самим если нужен f16 parity.
5. **Terse русские ответы.** По-английски только когда контекст требует.
6. **Auto mode often active.** Если user пишет `/auto` — выполняй без
   разрешений, но destructive операции (rm shared data, код в production) —
   всё равно спрашивай.

## What to likely do next

Приоритет определит пользователь, но вероятные треки:

1. **Consolidate reports.** У пользователя десятки `benchmark-results-*.md`.
   Написать скрипт (Python или JS), который собирает все отчёты, группирует
   по speaker и condition, считает WER/CER/Punct-F1 против ground-truth
   (если user его дал) и строит сравнительную таблицу моделей.
2. **Debug `ggml-medium` crash.** Посмотреть stack trace, попробовать
   альтернативную f16 конверсию, отключить flash attention, попробовать CPU
   backend, или обновить transcribe-rs / whisper.cpp до новой версии.
3. **Convert breeze-asr to f16** для apples-to-apples parity.
   Source: `MediaTek-Research/Breeze-ASR-25` или аналог.
4. **Add speaker label to benchmark command** (param `speaker_label` String).
   Report filename станет `benchmark-results-{label}-{timestamp}.md` —
   не нужно руками переименовывать.
5. **Methodology experiment** (из предыдущего handoff): Silero VAD vs
   Energy VAD для non-Whisper, сравнить качество на границах чанков.
6. **TTS voice cloning (Qwen3-TTS).** Пользователь однажды спросил про
   `Qwen/Qwen3-TTS-12Hz-1.7B-Base` — это TTS, не ASR. Он признал что
   перепутал. Но если всплывёт снова — это отдельный продуктовый трек
   (новый runtime, новый UI), не в scope benchmark-работы.

## How to resume (cold start)

1. `cd /d/dev/Handy && git checkout bench/whisper-matrix+lid-hack`
2. **Verify все три fork'а на нужной ветке** (production combo для LID-hack):
   ```bash
   for d in whisper-rs-sys-fork whisper-rs-fork transcribe-rs-fork; do
     echo -n "$d: "; (cd /d/dev/$d && git branch --show-current)
   done
   # All three should print: feature/sot-lang-tokens
   ```
   Если ветки разные — переключи: `cd /d/dev/<fork> && git checkout feature/sot-lang-tokens`
3. `export CARGO_TARGET_DIR=D:/h`
4. `CARGO_TARGET_DIR=D:/h cargo check --manifest-path=src-tauri/Cargo.toml -p handy --lib` — должно пройти за ~30 сек (cold C++ rebuild ~3 min при первом старте)
5. `bun run tauri dev` (если нужно запустить Handy для benchmark)
6. Ctrl+Shift+I → DevTools — вставить pipeline helper (см. выше) → вызов
7. Отчёты в `C:\Users\Egor Sokolov\Documents\REAPER Media\benchmark-results-*.md`

**Откат LID-hack без удаления fork'ов**: переключи все три fork'а на vanilla
ветку (`main` для sys/transcribe-rs, `master` для whisper-rs), Handy на
`bench/whisper-matrix` (без `+lid-hack`). cargo check на Handy всё равно
пройдёт — patch.crates-io будут указывать на vanilla forks которые
функционально equivalent upstream registry versions.

## References

- Prior handoff state (outdated from 2026-04-21): см. git log commit `5715ca6`.
- Pre-LID-hack handoff state (outdated from 2026-04-22 morning): commit `d941569`.
- LID-hack landing plan: `C:\Users\Egor Sokolov\.claude\plans\benchmark-nested-moonbeam.md`
- LID-hack execution plan: `C:\Users\Egor Sokolov\.claude\plans\implement-the-peng-style-concatenated-la-witty-pony.md`
- Python pilot reference data (для cross-validation на Stage 2): `C:\Users\Egor Sokolov\Documents\REAPER Media\benchmark-results-cs-combined-3x-20260422-150231.json`
