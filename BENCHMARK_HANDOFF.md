# Benchmark Harness — Handoff

**Branch:** `bench/whisper-matrix`
**Last handoff:** 2026-04-22

## What this is

Tauri команда `benchmark_transcription_file` внутри Handy прогоняет один WAV
через матрицу (model × condition) для сравнения качества (WER/CER/пунктуация)
и скорости транскрипции. Вызов — из DevTools запущенного `tauri dev`. Нет UI.

## Current state

### Branch & commits

- Рабочая ветка `bench/whisper-matrix` (от `feat/clipboard-multiformat`).
- **Uncommitted changes** в `src-tauri/src/commands/benchmark.rs` (+95 строк):
  - Добавлены 16 `RunSpec` entry для 4 новых custom Whisper моделей
    (`whisper-podlodka-turbo`, `whisper-large-v3-russian`, `ggml-large-v3`, `ggml-medium`)
  - Добавлены параметры команды: `prompt_override: Option<String>` и `skip_no_prompt: Option<bool>`
  - Убраны неиспользуемые: `_transcription_manager: State<...>` и `output_dir: Option<String>`
    (чтобы уложиться в specta 10-param limit — было 12, стало 10)
  - Добавлена функция `write_checkpoint()` — пишет `benchmark-results-checkpoint.json/.md`
    после каждой завершённой модели, чтобы крэш не терял уже собранные данные
- Также изменён `src/bindings.ts` (авто-регенерация tauri-specta).

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

### Benchmark command signature (current, after this session's changes)

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
    prompt_override: Option<String>,       // NEW — when Some, replaces settings.transcription_prompt
                                           // for use_prompt=true conditions and clears custom_words
    skip_no_prompt: Option<bool>,          // NEW — when true, skips RunSpec entries with use_prompt=false
) -> Result<String, String>                // returns path to final JSON report
```

Invocation args (JS camelCase): `filePath`, `warmupPath`, `runsPerCondition`,
`skipModels`, `maxChunkSecsOverride`, `language`, `promptOverride`, `skipNoPrompt`.

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

## transcribe-rs fork (unchanged)

`D:\dev\transcribe-rs-fork\` — базирован на 0.3.8, патч добавляет
`n_max_text_ctx: Option<i32>` и `entropy_thold: Option<f32>` в
`WhisperInferenceParams`. Handy's `Cargo.toml` use `[patch.crates-io]`
указывающий туда. Не удалять, не трогать.

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
    { label:'W-auto-v1', language:'auto', promptOverride:V1, skipNoPrompt:false, skipModels:NONWHISPER, runsPerCondition:3 },
    { label:'W-auto-v2', language:'auto', promptOverride:V2, skipNoPrompt:true,  skipModels:NONWHISPER, runsPerCondition:3 },
    { label:'NW-auto-noCanary', language:'auto', skipModels:[...WHISPER,'canary-1b-v2'], runsPerCondition:1 },
    { label:'NW-ru', language:'ru', skipModels:WHISPER, runsPerCondition:1 },
  ];
  const results = [];
  for (const [i, inv] of plan.entries()) {
    const args = { filePath:FILE, warmupPath:WARMUP, language:inv.language, skipModels:inv.skipModels, runsPerCondition:inv.runsPerCondition };
    if (inv.promptOverride !== undefined) args.promptOverride = inv.promptOverride;
    if (inv.skipNoPrompt) args.skipNoPrompt = true;
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

1. `cd /d/dev/Handy && git checkout bench/whisper-matrix`
2. `export CARGO_TARGET_DIR=D:/h`
3. `CARGO_TARGET_DIR=D:/h cargo check --manifest-path=src-tauri/Cargo.toml -p handy --lib` — должно пройти за ~30 сек
4. `bun run tauri dev` (если нужно запустить Handy для benchmark)
5. Ctrl+Shift+I → DevTools — вставить pipeline helper (см. выше) → вызов
6. Отчёты в `C:\Users\Egor Sokolov\Documents\REAPER Media\benchmark-results-*.md`

## References

- Prior handoff state (outdated from 2026-04-21): см. git log commit `5715ca6`.
- Plan file of this handoff: `C:\Users\Egor Sokolov\.claude\plans\d-dev-handy-benchmark-handoff-md-enchanted-shamir.md`
