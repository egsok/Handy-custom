# Next Agent Handoff — LID-Hack Variance Experiment Continuation

**Date:** 2026-04-22 evening
**Branch:** `bench/whisper-matrix+lid-hack` (tip: `0403ad8`)
**User:** Egor

## TL;DR (30 seconds)

- **LID-hack fully shipped and validated.** 3 upstream Rust crate forks + vendored whisper.cpp C++ patches + Handy wiring. `cargo build` green, E2E validated on `whisper-podlodka-turbo` (latency 3.2×, token-choice divergence, zero-regression on `sot_lang_tokens=None`).
- **Variance experiment underway.** User is running a breeze-asr variance probe as this handoff is being written — `bun run tauri dev` + DevTools invocation on sp1 then sp2. **Currently running; do NOT touch `src-tauri/` until it finishes.**
- **Known issue:** the in-flight invocation is against the PRE-surgery matrix (commit `63067ad` = 6 rows) because hot-reload didn't catch `0403ad8` (4 rows) before user pasted the snippet. Output is 30 transcripts per speaker (not 20). Data is still useful: 15 × noprompt+ah × 3 LID + 15 × promptv2+ah × 3 LID. Baseline None-LID data for Group 1 is missing from this run → needs a follow-up if user cares about it.
- **Two more phases (Group 2) await:** 60 transcripts across 10 configs. Matrix edits + DevTools snippets provided below.

## What shipped this session — the full story

### 1. Peng-style concatenated-SOT-tokens LID hack

Goal: let Whisper benchmark force `<|lang_a|><|lang_b|><|transcribe|>…` as the SOT prefix instead of the single `<|lang|>` auto-detect. Verified to alter the decoder on real Russian speech (podlodka-turbo latency spike 3.2× on `["en","ru"]` vs `None`; token-choice flips `Телеграм` Cyrillic → `Telegram` Latin).

### 2. Four forks set up with vanilla-baseline-first protocol

| Fork | Path | Vanilla branch | Feature branch tip | Version |
|---|---|---|---|---|
| `whisper-rs-sys` | `D:/dev/whisper-rs-sys-fork` | `main` (`526a5ee`) | `feature/sot-lang-tokens` (`e25fb7f`) | `0.15.1-lid-hack.1` |
| `whisper-rs` | `D:/dev/whisper-rs-fork` | `master` (`93ed595`) | `feature/sot-lang-tokens` (`93421f9`) | `0.16.0` |
| `transcribe-rs` | `D:/dev/transcribe-rs-fork` | `main` (`cd1e227`) | `feature/sot-lang-tokens` (`b154503`) | (cargo-package extraction, now git'd) |
| `Handy` | `D:/dev/Handy` | `bench/whisper-matrix` (`d941569`) | `bench/whisper-matrix+lid-hack` (`0403ad8`) | — |

**Source-of-truth upstream:** `whisper-rs` is at `https://codeberg.org/tazz4843/whisper-rs` (the GitHub mirror is stale at 0.14.3). whisper.cpp is the ggerganov repo, vendored as a plain directory inside whisper-rs-sys-fork (de-submoduled) so patches survive.

**Critical:** all three non-Handy forks must be on `feature/sot-lang-tokens` for the Handy binary to build. Verify:
```bash
for d in whisper-rs-sys-fork whisper-rs-fork transcribe-rs-fork; do
  echo -n "$d: "; (cd /d/dev/$d && git branch --show-current)
done
```

### 3. C++ patches (5 atomic commits in whisper-rs-sys-fork)

Inside `whisper.cpp/`:
- `include/whisper.h` lines ~531-537: added `sot_lang_tokens` / `sot_n_lang_tokens` fields to `whisper_full_params`.
- `src/whisper.cpp` line 5943: default-init (`nullptr` / `0`) in `whisper_full_default_params`.
- `src/whisper.cpp` line 6962: primary SOT assembly (`whisper_full_with_state`) — branches on non-NULL pointer.
- `src/whisper.cpp` line 8843: DTW timestamp path — same branch.
- `Cargo.toml` version bump.

### 4. Rust plumbing

- **whisper-rs-fork**: `set_sot_lang_tokens(&[i32])` on `FullParams` mirroring `set_tokens` pattern. `lang_token_id(&self, code: &str) -> Option<i32>` on `WhisperContext` (+ re-export from wrapper).
- **transcribe-rs-fork**: `sot_lang_tokens: Option<Vec<i32>>` field on `WhisperInferenceParams` + wiring in `infer()` + `ctx_lang_token_id` pass-through on `WhisperEngine`.
- **Handy**:
  - `AppSettings::whisper_sot_lang_tokens: Option<Vec<String>>` field (`settings.rs`).
  - `managers/transcription.rs:554-588` resolves string codes → token IDs via `whisper_engine.ctx_lang_token_id(c)`.
  - `commands/benchmark.rs`:
    - `RunSpec::sot_lang_tokens: Option<&'static [&'static str]>`
    - `BenchmarkRunRecord::sot_lang_tokens: Option<Vec<String>>`
    - **BREAKING**: `promptOverride` + `skipNoPrompt` + `sotLangTokensOverride` bundled into `overrides: BenchmarkOverrides` (due to tauri-specta's 10-param cap).
    - Per-run mutation: `overrides.sotLangTokens > RunSpec.sot_lang_tokens > None`.

### 5. Validation (Stage 1)

Report: `C:\Users\Egor Sokolov\Documents\REAPER Media\benchmark-results-20260422-171442.json`. 6 podlodka-turbo runs on warmup wav (30s). 3 signals confirm patches reach decoder:
- Latency spike 1.7× / 3.2× on hack rows vs baseline.
- Token-choice divergence (`Телеграм` Cyrillic → `Telegram` Latin without any initial_prompt).
- JSON serialization of `sot_lang_tokens` correct per row.
- Zero-regression: rows 0-3 (`None`) byte-identical in length stdev to pre-patch baseline.

### 6. Stage 2-style variance experiment (podlodka-turbo, 2 files)

Reports: `benchmark-results-20260422-173406.json` (sp1-30s) + `-173504.json` (sp1-2min). Findings:
- `["en","ru"]` produces +7% (2-min) / +13% (30s) length inflation with stdev 14-21 (vs 0-1 for baseline).
- `["ru","en"]` indistinguishable from baseline length (stdev 0-9).
- Determinism: baseline + `["ru","en"]` essentially zero length variance.
- **Order matters**: first-lang-correct = no effect; first-lang-wrong = inflation + stdev.

### 7. Benchmark harness QoL

- `kill-port 1420` added to `predev` hook — auto-cleans stale vite before each `bun run tauri dev` (commit `a50efef`). Fixes the recurring "Port 1420 already in use" after WebView2 crashes.
- `bindings.ts` regenerated from tauri-specta with proper rustdoc comments (commit `7093320`).

### 8. `BENCHMARK_HANDOFF.md` fully rewritten

Documents fork chain, new command signature, LID-hack semantics, Stage 1 validation table, updated pipeline helper, upstream update procedure (commit `a96fa0d`).

## Current state

### Handy branch commits (bench/whisper-matrix+lid-hack, oldest → newest since d941569)

```
90e2fec chore(deps): pin whisper-rs and whisper-rs-sys to local LID-hack forks
b7c73c6 feat(settings): add whisper_sot_lang_tokens field to AppSettings
29a92f2 feat(transcription): resolve language codes and pass sot_lang_tokens into WhisperInferenceParams
9e245ba feat(benchmark): plumb sot_lang_tokens into RunSpec, BenchmarkRunRecord, and command signature
af90f01 feat(benchmark): add LID-hack RUN_MATRIX rows for whisper-podlodka-turbo
a96fa0d docs(bench): document LID-hack feature, fork chain, and Stage 1 validation
d54d978 feat(benchmark): add LID-hack RUN_MATRIX rows for breeze-asr (3 LID × 2 prompt+ah configs)
63067ad chore(bench): narrow breeze-asr matrix to LID variance experiment
7093320 chore(bindings): regenerate src/bindings.ts with tauri-specta docstrings
a50efef chore(deps): auto-clean port 1420 before vite dev
0403ad8 chore(bench): narrow breeze-asr matrix to Group 1 (noprompt+ah variance)
```

### Current RUN_MATRIX state (commit 0403ad8)

breeze-asr block has 4 rows: `noprompt+ah` × {`None`, `["ru"]`, `["en","ru"]`, `["ru","en"]`}. All other models unchanged from `bench/whisper-matrix` tip.

**BUT** the currently-running tauri dev binary is built against commit `63067ad` (previous matrix with 6 breeze rows = 3 noprompt+ah LID + 3 promptv2+ah LID). Hot-reload didn't finish before user pasted. See "In-flight experiment" below.

## In-flight experiment (as of this handoff)

**Running:** Group 1 probe via DevTools snippet (see below for the snippet they pasted). Processing sp1 first (`Voice to text benchmark.wav`), then sp2 (`ASR benchmark Nastya.wav`). Uses `runsPerCondition: 5`, `skipModels` = all-but-breeze, `overrides: {}`.

**What's ACTUALLY running** (stale-matrix state, 6 breeze rows × 5 runs):
- 3 × `up=F, ah=T, sot=["ru"]` × 5 = 15 per speaker
- 3 × `up=F, ah=T, sot=["en","ru"]` × 5 = 15 per speaker
- 3 × `up=F, ah=T, sot=["ru","en"]` × 5 = 15 per speaker
- 3 × `up=T, ah=T, sot=["ru"]` × 5 = 15 per speaker (bonus V2+lid_ru — note: override doesn't set prompt so V2 rows get no prompt actually, so this is effectively noprompt+ah+lid_ru again)
- Similar for other promptv2+ah+LID rows

Wait — let me re-verify. The 6-row matrix at 63067ad has rows with use_prompt=true for 3 of them. When user's snippet uses `overrides: {}`, the use_prompt=true rows get a prompt from `original_settings.transcription_prompt` which is whatever the UI has set (likely None). So those rows effectively run with no prompt despite use_prompt=true — giving ADDITIONAL noprompt-like data for those LID modes.

Net effect: 30 per speaker = 15 "pure noprompt+ah × 3 LID × 5" + 15 "use_prompt=true-but-empty-prompt × 3 LID × 5". The second half is essentially equivalent to noprompt (except for the settings mutation side-effects).

**Missing from this run**: baseline `None` LID data for noprompt+ah (not in the stale matrix).

**DO NOT** commit to `src-tauri/` while this is running. File-watch hot-reload will restart the binary and crash the invocation mid-flight.

## Outstanding work

### Step 1 — Wait for Group 1 to finish

User will report two reportPath strings (sp1, sp2). Parse the JSONs and summarize: median/stdev of length and latency per (sot_lang_tokens, use_prompt) group × 5 runs each.

### Step 2 — Optional: Group 1 baseline patch-up

If user wants the missing `None` LID baseline for clean Group 1 variance data, do a small 1-row matrix surgery and run a targeted invocation. This requires the Handy binary to actually pick up commit `0403ad8` (current tip) or to apply a new surgery for just the baseline row. Propose this to the user after they see Group 1 results.

### Step 3 — Group 2 Phase A (Matrix surgery #2A)

Replace the breeze-asr block with 7 rows (V1-compatible: 3 V1-using rows + 4 noprompt rows across breeze + ggml-medium + medium). Snippet for the replacement:

```rust
    // breeze-asr: Group 2 Phase A (V1 prompt + noprompt champion candidates).
    RunSpec { model_id: "breeze-asr",   engine_label: "whisper", use_prompt: true,  use_anti_halluc: true , sot_lang_tokens: Some(&["ru"]) },  // V1 + ah + lid_ru
    RunSpec { model_id: "breeze-asr",   engine_label: "whisper", use_prompt: false, use_anti_halluc: false, sot_lang_tokens: Some(&["ru"]) },  // noprompt + noah + lid_ru
    RunSpec { model_id: "ggml-medium",  engine_label: "whisper", use_prompt: true,  use_anti_halluc: false, sot_lang_tokens: Some(&["ru"]) },  // V1 + noah + lid_ru
    RunSpec { model_id: "ggml-medium",  engine_label: "whisper", use_prompt: false, use_anti_halluc: false, sot_lang_tokens: Some(&["ru"]) },  // noprompt + noah + lid_ru
    RunSpec { model_id: "ggml-medium",  engine_label: "whisper", use_prompt: false, use_anti_halluc: true , sot_lang_tokens: Some(&["ru"]) },  // noprompt + ah + lid_ru
    RunSpec { model_id: "medium",       engine_label: "whisper", use_prompt: true,  use_anti_halluc: false, sot_lang_tokens: Some(&["ru"]) },  // V1 + noah + lid_ru
    RunSpec { model_id: "medium",       engine_label: "whisper", use_prompt: false, use_anti_halluc: false, sot_lang_tokens: Some(&["ru"]) },  // noprompt + noah + lid_ru
```

Replace the existing breeze block (the 4 rows from `0403ad8`). Also **remove** the existing `medium` and `ggml-medium` blocks from the matrix (they're baseline None rows that would add 8 extra runs per invocation). Or temporarily skip them via `skipModels` in the DevTools snippet — either works.

**Careful:** cargo check, verify tauri dev hot-reloads (wait 60s for rebuild), then commit.

**DevTools snippet Phase A:**

```js
(async () => {
  const SKIP_ALL_BUT_TARGETS = ['turbo','large','whisper-podlodka-turbo','whisper-large-v3-russian','ggml-large-v3','parakeet-tdt-0.6b-v3','canary-1b-v2','gigaam-v3-e2e-ctc'];
  // Note: do NOT skip 'breeze-asr', 'ggml-medium', 'medium' — those are the targets.
  const WARMUP = 'C:\\Users\\Egor Sokolov\\Documents\\REAPER Media\\01-260420_2148-01.wav';
  const SP1 = 'C:\\Users\\Egor Sokolov\\Documents\\REAPER Media\\Voice to text benchmark.wav';
  const SP2 = 'C:\\Users\\Egor Sokolov\\Documents\\REAPER Media\\ASR benchmark Nastya.wav';
  const V1 = 'Привет! Как дела? Он сказал: «Сделаем это сегодня — пока есть время». Конечно, не всё так просто, как кажется на первый взгляд; нужно принять во внимание погоду.';
  const plan = [{ label: 'sp1-phA', filePath: SP1 }, { label: 'sp2-phA', filePath: SP2 }];
  const results = [];
  for (const inv of plan) {
    console.log(`[${inv.label}] starting Phase A (7 rows × 3 runs = 21 transcripts)...`);
    const t0 = Date.now();
    try {
      const reportPath = await window.__TAURI_INTERNALS__.invoke('benchmark_transcription_file', {
        filePath: inv.filePath, warmupPath: WARMUP, language: 'auto',
        runsPerCondition: 3, skipModels: SKIP_ALL_BUT_TARGETS,
        overrides: { prompt: V1 },
      });
      const elapsed = ((Date.now() - t0) / 1000).toFixed(1);
      console.log(`[${inv.label}] done in ${elapsed}s → ${reportPath}`);
      results.push({ label: inv.label, elapsed, reportPath });
    } catch (e) { console.error(`[${inv.label}] FAILED:`, e); results.push({ label: inv.label, error: String(e) }); }
  }
  console.table(results);
  window.__phaseA_results = results;
})();
```

Expected: 2 × 7 × 3 = 42 transcripts.

### Step 4 — Group 2 Phase B (Matrix surgery #2B)

Replace the breeze-asr block from Phase A with 3 V2-using rows. Remove the ggml-medium and medium rows added in Phase A (Phase B is breeze-only).

```rust
    // breeze-asr: Group 2 Phase B (V2 prompt × 3 LID modes, all noah).
    RunSpec { model_id: "breeze-asr", engine_label: "whisper", use_prompt: true, use_anti_halluc: false, sot_lang_tokens: Some(&["ru"]) },
    RunSpec { model_id: "breeze-asr", engine_label: "whisper", use_prompt: true, use_anti_halluc: false, sot_lang_tokens: Some(&["ru", "en"]) },
    RunSpec { model_id: "breeze-asr", engine_label: "whisper", use_prompt: true, use_anti_halluc: false, sot_lang_tokens: Some(&["en", "ru"]) },
```

**DevTools snippet Phase B:**

```js
(async () => {
  const SKIP_NON_BREEZE = ['turbo','large','medium','whisper-podlodka-turbo','whisper-large-v3-russian','ggml-large-v3','ggml-medium','parakeet-tdt-0.6b-v3','canary-1b-v2','gigaam-v3-e2e-ctc'];
  const WARMUP = 'C:\\Users\\Egor Sokolov\\Documents\\REAPER Media\\01-260420_2148-01.wav';
  const SP1 = 'C:\\Users\\Egor Sokolov\\Documents\\REAPER Media\\Voice to text benchmark.wav';
  const SP2 = 'C:\\Users\\Egor Sokolov\\Documents\\REAPER Media\\ASR benchmark Nastya.wav';
  const V2 = 'Привет! Как дела? Наш English-speaking friend сказал: «Сделаем это сегодня — пока есть время». Мы выполняли эту разработку в Claude Code. Конечно, не всё так просто; нужно учесть погоду.';
  const plan = [{ label: 'sp1-phB', filePath: SP1 }, { label: 'sp2-phB', filePath: SP2 }];
  const results = [];
  for (const inv of plan) {
    console.log(`[${inv.label}] starting Phase B (3 rows × 3 runs = 9 transcripts)...`);
    const t0 = Date.now();
    try {
      const reportPath = await window.__TAURI_INTERNALS__.invoke('benchmark_transcription_file', {
        filePath: inv.filePath, warmupPath: WARMUP, language: 'auto',
        runsPerCondition: 3, skipModels: SKIP_NON_BREEZE,
        overrides: { prompt: V2 },
      });
      const elapsed = ((Date.now() - t0) / 1000).toFixed(1);
      console.log(`[${inv.label}] done in ${elapsed}s → ${reportPath}`);
      results.push({ label: inv.label, elapsed, reportPath });
    } catch (e) { console.error(`[${inv.label}] FAILED:`, e); results.push({ label: inv.label, error: String(e) }); }
  }
  console.table(results);
  window.__phaseB_results = results;
})();
```

Expected: 2 × 3 × 3 = 18 transcripts.

### Step 5 — After all experiments finish

**Restore matrix to clean `bench/whisper-matrix`-style state** before final merge. The surgery commits (`d54d978`, `63067ad`, `0403ad8`, + Phase A/B) on feature branch should either be squashed to a single "feat(benchmark): LID-hack RUN_MATRIX additions" or reverted to leave just the permanent hack rows (podlodka-turbo + a few breeze-asr) if user decides what to keep.

Suggest: git rebase interactive once experiments done, or just cherry-pick the cleanup commit.

## Critical gotchas

1. **Windows MAX_PATH:** `export CARGO_TARGET_DIR=D:/h` before ANY cargo command (including the ones tauri dev spawns internally).
2. **Hot-reload timing:** `src-tauri/` edits trigger cargo rebuild in tauri dev. Rebuild takes 3-30s incremental, 2+ min from cold. If user pastes DevTools snippet DURING rebuild, they might hit the still-running pre-rebuild binary. After any `src-tauri/` change, wait for "Running `D:/h\debug\handy.exe`" line in tauri dev output before invoking benchmark.
3. **kill-port hook:** `bun run tauri dev` now auto-frees port 1420 via `predev` hook (commit `a50efef`). If vite errors out saying port busy anyway, investigate — something weird.
4. **WebView2 intermittent error on hot-reload:** `HRESULT(0x80070057)` seen occasionally. Usually means restart tauri dev cleanly.
5. **tauri-specta 10-param cap:** don't blindly add more top-level params to `benchmark_transcription_file`. Extend `BenchmarkOverrides` instead.
6. **Three forks must match branch:** any time switching between vanilla and feature behaviors, switch ALL THREE forks together. Mismatched state (e.g. sys=feature + wr=master) can produce compile errors or wrong runtime behavior.
7. **SettingsGuard scope:** `whisper_sot_lang_tokens` is restored on drop via `AppSettings::clone()`. Don't leak state between benchmark invocations by forgetting to clear the field — `SettingsGuard` handles it automatically now.

## Reference files / paths

- Main benchmark doc: `D:/dev/Handy/BENCHMARK_HANDOFF.md` (comprehensive, includes fork chain details, upstream-update procedure, resume instructions)
- Validation reports (Stage 1 + variance):
  - `C:/Users/Egor Sokolov/Documents/REAPER Media/benchmark-results-20260422-171442.{json,md}` — podlodka-turbo Stage 1 (warmup 30s)
  - `C:/Users/Egor Sokolov/Documents/REAPER Media/benchmark-results-20260422-173406.{json,md}` — podlodka-turbo variance sp1-30s
  - `C:/Users/Egor Sokolov/Documents/REAPER Media/benchmark-results-20260422-173504.{json,md}` — podlodka-turbo variance sp1-2min
  - Group 1 breeze variance: **in progress**, will be at `-2026042...{json,md}` with timestamp close to this handoff.
- Python pilot reference data: `C:/Users/Egor Sokolov/Documents/REAPER Media/benchmark-results-cs-combined-3x-20260422-150231.json`
- Plan files:
  - Landing plan: `C:/Users/Egor Sokolov/.claude/plans/benchmark-nested-moonbeam.md`
  - Execution plan: `C:/Users/Egor Sokolov/.claude/plans/implement-the-peng-style-concatenated-la-witty-pony.md`

## Fork resume procedure (cold-start in new session)

1. `cd /d/dev/Handy && git checkout bench/whisper-matrix+lid-hack`
2. Verify fork branches (all on feature/sot-lang-tokens):
   ```bash
   for d in whisper-rs-sys-fork whisper-rs-fork transcribe-rs-fork; do
     echo -n "$d: "; (cd /d/dev/$d && git branch --show-current)
   done
   ```
3. `export CARGO_TARGET_DIR=D:/h`
4. `cargo check --manifest-path=src-tauri/Cargo.toml -p handy --lib` — cold build ~3 min, warm ~30s.
5. `bun run tauri dev` — `predev` hook will auto-free port 1420.
6. Open DevTools (F12) once window appears → Console → paste snippet.

## For the next agent: probable user intent after this session

Based on the behavioral arc of this session, user will likely want to:
1. Analyze Group 1 results (once it completes) — compare noprompt+ah × 4 LID variance at n=5.
2. Execute Group 2 Phase A and B as specified above.
3. Possibly extend to more speakers (sp3 from `speaker3.wav`) once champions are identified.
4. Eventually clean up the feature-branch matrix surgeries and prepare a PR back to `bench/whisper-matrix` or direct to `main` with just the core LID-hack commits (omit the experiment-specific matrix rows).

**Do NOT:**
- Implement Tier 2 UI (Settings dropdown for LID modes) — explicitly out-of-scope per user spec.
- Push any fork to external remotes without user confirmation.
- Force-push or rewrite history on the Handy branch.
