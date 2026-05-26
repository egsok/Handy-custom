# Next Agent Handoff — D12 Overnight Run In Progress (empirical RTF calibration)

**Date:** 2026-04-26 (overnight of 2026-04-25/26, in flight at handoff time)
**Branch:** `bench/whisper-matrix+lid-hack` (tip: `e86758c` in Handy — unchanged)
**User:** Egor

## TL;DR (60 seconds)

- **D12 night run is currently running.** 23 invocations driving 89 of 90 cids from `D:/dev/General tasks and research/asr_eval/artifacts/D12_overnight_inference_targets.md`. Started 2026-04-25 20:48Z; at handoff time **10/23 done in 7h20min, ~6h remaining**, ETA ~10:00Z / 13:00 МСК on 2026-04-26.
- **Queue file:** `C:/Users/Egor Sokolov/Documents/REAPER Media/d12-night-run-queue.js`. Local scratch (not in repo). 23 invocations grouped by (prompt × language × audio_set), covering SP1/SP2 long-form (§A+§B+§C+§C2) and SP1/SP2 live (§D+§E).
- **Universal `run_idx_offset = 50`** on every invocation. Avoids collision with all prior data (max prior n was 29). Eval driver dedups by `(cid, audio_file, run_idx)` and never relies on contiguity.
- **One cid skipped: `breeze-asr__noprompt__auto__lid_en+ru__ah`** (§C row 30). The matrix at `benchmark.rs:42-50` has breeze noprompt+ah only with `["ru"]`, not `["en","ru"]`. Adding it would require a one-line `RunSpec` + cargo rebuild (~7 min). Deferred per the no-touch-`src-tauri/` discipline. The other 89 cids are covered.
- **Empirical RTF calibration** (this is the load-bearing finding for next agents):

  | model_id | observed RTF (sp1 long-form, /O2 binary) | D12 spec implied RTF |
  |---|---|---|
  | breeze-asr | **0.054** | ~0.055 |
  | ggml-medium | **0.044** | ~0.045 |
  | turbo | **0.017** | ~0.022 (D12 was 30% too pessimistic) |
  | large | ~0.050 (assumed = breeze, both large-class) | ~0.051 |
  | whisper-podlodka-turbo | ~0.020 (turbo family + margin) | not in §A-§C2 |

  Pre-DRY estimates were guesswork; **post-DRY estimates landed within +1.7% mean error** vs actuals across the first 10 invocations. **Use observed RTFs above for any future budget calc; do NOT eyeball.**

## What this session shipped

1. Wrote `d12-night-run-queue.js` from scratch using the v17/R2 pattern. New fields used: `run_idx_offset` (universal 50), `audio_batch` (live invocations only), `only_conditions` (per-invocation pin), `prompt` override (V1/V2/V2b/V3/V4/null).
2. **DRY validated.** Two reports verified end-to-end FFI chain: `effective_initial_prompt` populated correctly (None for noprompt, full Cyrillic V1 for prompt rows), `decoder_prompt_init_tokens` populated (`[50258, 50263, 50359]` = `<|sot|><|ru|><|transcribe|>` for breeze; turbo uses 50360 transcribe-token), `run_idx=50` confirmed, no transcribe errors.
3. Refined wallclock estimate **using observed RTFs**, not vibes. Final: ~13.4h (vs handoff's "5–12h" guess). The +/-1.7% mean error against actuals proves the new estimating method.

## Reports on disk (in flight)

- **Long-form (single-file, anchored at REAPER Media/):** `C:/Users/Egor Sokolov/Documents/REAPER Media/benchmark-results-2026042{5,6}-*.json`. 13 invocations total (7 SP1 long + 6 SP2 long).
- **Live (audio_batch, anchored at audio's parent):**
  - `D:/dev/General tasks and research/asr_eval/artifacts/live_v1/audio_wav/benchmark-results-*.json` — 5 SP1 live invocations.
  - `D:/dev/General tasks and research/asr_eval/artifacts/live_v1/audio_sp2/benchmark-results-*.json` — 5 SP2 live invocations.
- All records carry `run_idx ∈ [50, 50 + Δ)` where Δ is the per-invocation `runsPerCondition`.
- DRY records (run_idx=50, runs=1) for first 2 invocations were on disk before the full run started. Resume-from-checkpoint matches `(cid, audio_file, run_idx)`; the run_idx=50 slot for those 8 conditions was already filled by DRY records, so the full run wrote run_idx=51..71 for breeze-noprompt and run_idx=51..72 for V1.auto. Eval should treat that slot as authoritative (one record only per slot — not duplicated).

## Working tree state

Unchanged from prior 2026-04-25 handoff:

```
M src-tauri/Cargo.lock
M src-tauri/src/commands/benchmark.rs
M src-tauri/src/lib.rs
M src/bindings.ts
```

No new commits this session. Last commit: `e86758c docs: update handoff after user-terminated night run + daily-use mode`. **Do not commit without explicit user instruction.**

## When the queue finishes

1. Verify all 23 expected JSON reports exist with the appropriate paths above.
2. Spot-check the last invocation's report — `audio_file` field present in batch-mode records, `run_idx` ∈ [50, 50+Δ), `effective_initial_prompt` populated where prompt was non-null, no `error` fields populated in `runs[]`.
3. Settings should be auto-restored by `SettingsGuard` to whatever was active when queue paste ran (daily-use config: breeze + LID=`["ru"]` + V2 prompt in UI, per prior handoff).
4. Hand reportPaths off to analytical chat (D12 §H pipeline: re-aggregate metrics → re-run `_analysis_v25_quantization_audit.py` → `_analysis_v25_bootstrap.py` → CI flip check).

## Notes for the next agent

- **The queue runner uses the harness's resume-from-checkpoint based on `file_path` stem** (no explicit `resume_from` field). Re-paste of `d12-night-run-queue.js` after a crash is safe; harness will skip whatever already completed via the `(model_id, use_prompt, effective_initial_prompt, use_anti_halluc, sot_lang_tokens, language, audio_file, run_idx)` RunKey.
- **Some over-runs by design.** Each invocation's `runsPerCondition` = max Δ among grouped cids; cids with smaller Δ get extra runs. Eval can subset by `run_idx` range later. Total over-run cost was budgeted into the 13.4h estimate.
- **Top time sinks remaining:** `sp2.long.V3.auto` (~78 min, 22 runs × 6 conditions) is the biggest single invocation in the queue. After it finishes, only live remains (~3h, mostly cheap turbo).
- **All open items from prior handoff still apply.** The "Pending cleanups before merging upstream" list in the v17 handoff below is still the punch list:
  - revert TEMP `tauri/devtools` Cargo.toml feature
  - revert `lib.rs:530-536` `open_devtools()` block
  - decide on the 9 LID matrix rows (keep as feat or split into separate commits)
  - 3 queue-runner files in REAPER Media/ are NOT in repo — local scratch
  - bindings.ts auto-regenerates next dev/build

## File paths quick reference

- This handoff (top of file = newest): `D:/dev/Handy/NEXT-AGENT-HANDOFF.md`
- D12 targets spec: `D:/dev/General tasks and research/asr_eval/artifacts/D12_overnight_inference_targets.md`
- D12 queue file: `C:/Users/Egor Sokolov/Documents/REAPER Media/d12-night-run-queue.js` (23 invocations)
- Plan file (this session): `C:/Users/Egor Sokolov/.claude/plans/d-dev-handy-next-agent-handoff-md-gleaming-mochi.md`
- Active branch: `D:/dev/Handy` @ `bench/whisper-matrix+lid-hack` @ `e86758c` (uncommitted on top, same as prior handoff)
- Forks unchanged: `D:/dev/whisper-rs-sys-fork` @ `198f290`, `whisper-rs-fork` @ `eb282bf`, `transcribe-rs-fork` @ `8454e4e`
- Release binary in use: `D:/h/release/handy.exe` (mtime ~2026-04-25 22:00, /O2 + devtools).

---

# Next Agent Handoff — v17 + live_v1 R1/R2 Complete (audio_batch + run_idx_offset)

**Date:** 2026-04-25 (overnight of 2026-04-24/25)
**Branch:** `bench/whisper-matrix+lid-hack` (tip: `e86758c` in Handy; `198f290` in whisper-rs-sys-fork)
**User:** Egor

## TL;DR (90 seconds)

- **Three night runs completed successfully.** v17 (508 runs ~3h actual), live_v1 Round 1 (34 invocations / ~7920 transcripts), live_v1 Round 2 (47 invocations / ~11468 transcripts, **205 min actual** vs 7h plan = exactly 2× off). All reports on disk; analytical chat owns aggregation.
- **`benchmark.rs` significantly extended.** New fields and modes:
  - `BenchmarkOverrides.audio_batch: Option<Vec<String>>` — switches harness to multi-file mode (model loads once per condition, iterates files × N runs). Single-file legacy path retained.
  - `BenchmarkRunRecord.audio_file: Option<String>` — bare filename per record (e.g. `"live-18-7s.wav"`). None in single-file mode; Some(name) in batch mode.
  - `BenchmarkOverrides.run_idx_offset: Option<u32>` — shifts `run_idx ∈ [offset, offset+N)`. Used by Round 2 T3 to append 8 runs (12..19) to Round 1 baseline (0..11) without collision when eval merges by `(cid, audio_file, run_idx)`.
  - Resume-key includes `audio_file` + raw `run_idx`, so resume survives both batch mode and offset.
  - All 4 `BenchmarkRunRecord` construction sites (success path + 3 error paths) updated for `audio_file` + offset.
- **RUN_MATRIX grew from 38 → 47 rows** across three sessions:
  - v17 (`feat: add 6 LID matrix rows`): +6 rows (4 breeze LID + 2 turbo LID)
  - v20 R1 (`feat: add 2 LID matrix rows for live_v1 Tier 5`): +2 rows (podlodka [ru,en] + ggml-medium [ru,en])
  - v20 R2 H37: +1 row (whisper-large-v3-russian noprompt+ah+[ru])
- **DevTools auto-open shipped.** `lib.rs:530` calls `main_window.open_devtools()` after `win_builder.build()` — was needed because Tauri 2's `devtools` feature is API-only, F12 not bound by default. Both this AND the `tauri/devtools` Cargo.toml feature are TEMPORARY for benchmark work.
- **Queue runners (production):**
  - `C:/Users/Egor Sokolov/Documents/REAPER Media/night-run-queue-v17.js` — v17 (19 invocations, 508 runs)
  - `C:/Users/Egor Sokolov/Documents/REAPER Media/live-v1-queue.js` — Round 1 (34 invocations, 4 scripted + 30 live × 22 audio × 12 runs)
  - `C:/Users/Egor Sokolov/Documents/REAPER Media/live-v1-queue-r2-dry1.js` — Round 2 DRY-1 (audio_batch path on new audio)
  - `C:/Users/Egor Sokolov/Documents/REAPER Media/live-v1-queue-r2-dry2.js` — Round 2 DRY-2 (H37 NEW row LID)
  - `C:/Users/Egor Sokolov/Documents/REAPER Media/live-v1-queue-r2-prod.js` — Round 2 production (47 invocations / 11468 runs / T3 with `run_idx_offset: 12`)

## Reports on disk (read-only inventory)

- **v17 reports**: `C:/Users/Egor Sokolov/Documents/REAPER Media/benchmark-results-2026042{3,4}-*.json` (19 files spanning 23:31 Apr 23 → ~04:00 Apr 24).
- **Round 1 reports**: same `REAPER Media/` folder for scripted-mode jobs; `D:/dev/General tasks and research/asr_eval/artifacts/live_v1/audio_wav/benchmark-results-*.json` for batch-mode jobs (audio_wav was first audio's parent, harness anchors there).
- **Round 2 reports**: `audio_wav/benchmark-results-202604{24-22*,25-0*}*.json` — 50 files (3 DRY + 47 prod), spanning **22:02 Apr 24 → 01:25 Apr 25** ≈ 205 min total. Each prod report carries 22-37 audio × N runs. T3's 10 reports verified to have run_idx ∈ [12..19].

## Working tree state (uncommitted)

```
M src-tauri/Cargo.lock          ← incidental (fork timestamps refreshed during builds)
M src-tauri/src/commands/benchmark.rs  ← matrix +9 rows + audio_batch/audio_file/run_idx_offset
M src-tauri/src/lib.rs          ← TEMP: open_devtools() in setup
M src/bindings.ts               ← stale (auto-regen via tauri-specta on next dev/build)
```

Last commit: `e86758c docs: update handoff after user-terminated night run + daily-use mode`. Nothing committed since the prior 2026-04-23 handoff — all three night-run features are still uncommitted in working tree. User's pattern is to defer commits until ready to merge upstream; do NOT commit without explicit user instruction.

## Pending cleanups before merging upstream

1. **Cargo.toml `tauri/devtools` feature** (line 41) — TEMPORARY for benchmark F12 access. Revert before any release/main merge.
2. **`lib.rs:530-536` `open_devtools()` block** — depends on the Cargo.toml feature above. Both must revert together; will fail to compile if only one is reverted (compile-time safety net is intentional).
3. **9 matrix rows added** (lines 46-50, 58, 72-73, 91 area) — these are real LID variants the night-run system actually uses. Decision: keep them (they're feature work, not scaffolding) or revert and re-add via separate commits in proper feature-branch hygiene before PR. User previously expressed preference for a single squashed "feat(benchmark): LID-hack RUN_MATRIX additions" before main merge.
4. **3 queue-runner files** in `REAPER Media/` are NOT in repo — they're user's local scratch. Don't try to commit them.
5. **`bindings.ts`** — should auto-regenerate cleanly on next `bun run tauri dev`. If diff persists, force-regenerate with explicit specta export call.

## Known good binary

`D:/h/release/handy.exe` mtime `2026-04-25 ~22:00` (built last with H37 row + run_idx_offset + open_devtools). 104.78 MB. Includes `/O2` fix + `devtools` feature + auto-open DevTools call. Custom-signing error at end of `bun run tauri build` is expected and ignorable (just a missing optional bundling step; raw .exe is built fine).

## Architecture notes for next agent

**Audio_batch mode internals (`benchmark.rs:716-740` area):**
- If `overrides.audio_batch` is `Some(non-empty)`: harness pre-loads ALL files into `Vec<(Option<String> filename, Vec<f32> samples, f64 dur)>` before main loop. Fail-fast on bad WAV — entire invocation aborts with clear error if any file fails to decode.
- If `audio_batch` is None or empty: legacy single-file mode using positional `file_path` arg, `audio_file` field set to None on records (preserves backward compat with v17-style invocations).
- `file_path` arg in batch mode is **label-only** — used for `report.input_file` and checkpoint filename, existence NOT checked. User passes labels like `"round2.T3.H05.br.V2.lid_ru.ah"`.
- `output_dir_path` in batch mode anchors to parent of FIRST `audio_batch[0]` file (Round 2 → `audio_wav/`). In single-file mode, parent of `file_path`.

**Run_idx_offset internals (`benchmark.rs:680, 1013, ~885, ~916`):**
- Read once at function start: `let run_idx_offset = overrides.run_idx_offset.unwrap_or(0);`
- All 4 `for run_idx in ...` loops shifted to `run_idx_offset..(run_idx_offset + runs_per_condition)` — covers success path + 2 error paths (model not downloaded, switch_active_model failed) + the resume-key construction.
- Resume-from-checkpoint compares full RunKey including `run_idx` directly (no offset compensation needed) — different offsets produce distinct keys, can coexist in the same checkpoint history.

**Eval driver responsibilities (analytical chat scope, NOT this agent's):**
- `live_v1_eval.py` and `live_v1_report.py` parse `runs[]` from each JSON, group by `(model_id, use_prompt, use_anti_halluc, sot_lang_tokens, effective_initial_prompt, language, audio_file)` to derive `condition_id`, then merge across all matching JSONs.
- T3 N=20 emerges automatically: Round 1 contributes run_idx 0..11 (one JSON per condition), Round 2 T3 contributes run_idx 12..19 (separate JSON per condition with offset=12). Eval driver dedups by `(cid, audio_file, run_idx)` across all files.

## Time-estimate calibration note

My estimates for these long runs were ~2× too pessimistic on `/O2` fast binary:
- v17: planned ~4.2h, actual ~3h.
- live_v1 Round 2: planned ~7h (~420 min), actual ~205 min — user explicitly flagged.
**Cause:** I apply ~30% reload overhead, but on this hardware overhead is closer to 5–10%; RTF is ~0.04, not 0.05. **Calibration:** for next overnight run, present user with a range like "X/2 to X" or "expect lower bound on /O2 fast binary".

## File paths quick reference

- This handoff (top of file = newest): `D:/dev/Handy/NEXT-AGENT-HANDOFF.md`
- Plan file: `C:/Users/Egor Sokolov/.claude/plans/d-dev-handy-next-agent-handoff-md-expressive-bird.md`
- Active branch: `D:/dev/Handy` @ `bench/whisper-matrix+lid-hack` @ `e86758c` (uncommitted on top)
- Forks unchanged from prior handoff: `D:/dev/whisper-rs-sys-fork` @ `198f290`, `whisper-rs-fork` @ `eb282bf`, `transcribe-rs-fork` @ `8454e4e`

---

# Next Agent Handoff — Night-Run Orchestration + MSVC /O2 Fix

**Date:** 2026-04-23 (overnight of 2026-04-22/23)
**Branch:** `bench/whisper-matrix+lid-hack` (tip: `5be5a82` in Handy; `198f290` in whisper-rs-sys-fork)
**User:** Egor

## TL;DR (60 seconds)

- **Parameter-provenance chain shipped (B1 + B2a from prior handoff backlog).** Every `BenchmarkRunRecord` now carries `effective_initial_prompt`, `effective_n_max_text_ctx`, `effective_entropy_thold`, and `decoder_prompt_init_tokens` (actual SOT tokens read back from whisper.cpp state via new FFI getter). Smoke verified on 4 breeze-asr rows at 2026-04-22 22:26.
- **Long-run reliability landed.** Per-run per-input-file checkpoint (`benchmark-checkpoint-<stem>.{json,md}`), resume-from-checkpoint tolerant of missing/corrupt files, resume-key widened to `(model_id, use_prompt, effective_initial_prompt, use_anti_halluc, sot_lang_tokens, language, run_idx)`.
- **RUN_MATRIX reshaped to 38-row superset** covering all blocks needed for the 2026-04-23 night run (A / A2 / B / C / C2 / D / F / G / J). New `BenchmarkOverrides.only_conditions: Vec<ConditionFilter>` pins each invocation to an exact tuple subset without relying on skipModels/skipNoPrompt. Matrix must be restored to `bench/whisper-matrix` shape before merging upstream.
- **Queue-runner written:** `C:/Users/Egor Sokolov/Documents/REAPER Media/night-run-queue.js` chains 52 invocations (~593 transcripts) with resume. DRY_RUN flag for smoke verification.
- **🔥 MAJOR FIND: MSVC /O2 was silently disabled.** cmake 4.2.3 + VS2022 generator dropped the default `CMAKE_CXX_FLAGS_RELEASE` initializers, so ggml-cpu.c and whisper.cpp TUs compiled at effectively `/Od`. Large-model RTF was 0.10 vs historical 0.06. Fix (whisper-rs-sys-fork `198f290`): explicit `/MD /O2 /Ob2 /DNDEBUG` in build.rs. Confirmed working: fresh sp2 Block C first run at **RTF 0.047** (even better than historical). Night-run budget shrank from ~8h to ~4h.
- **Night run was terminated early by the user** (not left to run overnight). Partial results exist — see "Current state" below. Final aggregation / morning-checklist bullet is moot; what's actually available on disk is a handful of final reports from Phase 1 Block C under two binary configs (slow pre-/O2 + fast post-/O2).
- **User is now running Handy in daily-use mode** with `breeze-asr__noprompt__auto__lid_ru__ah` (LID=["ru"]) applied via a DevTools settings snippet — the `whisper_sot_lang_tokens` field has no UI binding, so settings were patched directly via `plugin:store|load`+`|set`+`|save`+`set_active_model`.

## Current state (2026-04-23 ~01:00, user going offline to continue in another chat)

### Where Handy is right now
- Running as `D:/h/release/handy.exe` (release build with `/O2` fix AND `devtools` feature enabled via temporary `Cargo.toml` change).
- Settings have been patched to `breeze-asr__noprompt__auto__lid_ru__ah`:
  - `selected_model = "breeze-asr"`
  - `selected_language = "auto"`
  - `transcription_prompt = null`, `custom_words = []`
  - `whisper_anti_hallucination = true`
  - `whisper_sot_lang_tokens = ["ru"]` ← **no UI binding; any Settings-panel interaction risks Zustand writing a stale snapshot back to the store and clobbering this**
- User switched their record shortcut's mode from Hold-to-Talk to Toggle (via Settings UI Shortcuts tab — safe because Shortcuts tab doesn't touch the whisper_sot_lang_tokens field).
- The queue-runner IIFE is no longer running; `window.__night_results` may be present with whatever partial results accumulated before the run was terminated.

### Reports from tonight (under `C:/Users/Egor Sokolov/Documents/REAPER Media/`)
- **Two completed final reports** from Block C (first invocation per speaker, large.ru.V1.ah.N10):
  - `benchmark-results-20260423-000455.json` — sp1 large N=10, **RTF ~0.10 (SLOW pre-/O2 binary)**
  - (possibly) a sp2 final report written during resume if that invocation finished before user killed it
- **Smoke reports from 2026-04-22 evening** (pre-night run): `benchmark-results-20260422-222629.json` (breeze 4×1), `-234242` / `-234432` (large×1 smoke)
- **Checkpoints** may still be on disk for partially-completed invocations: `benchmark-checkpoint-<stem>.{json,md}`. The prior cleanup code only removes them on successful completion of that invocation, so aborted invocations leave them behind.

### Known artifact: /O2-mixed Block C data
- sp1 `large.ru.V1.ah.N10` data is entirely pre-/O2 (RTF ~0.10).
- sp2 `large.ru.V1.ah.N10` data (if finalized) is 1 slow run + 9 fast runs mixed — or simpler: only a checkpoint exists.
- If the user wants clean RTF stats post-/O2 on any of these conditions, a re-run (tiny targeted invocation, ~10 min for 20 runs) will give clean data. Prior `benchmark-results-20260422-*` reports were from the pre-/O2 binary (~0.06 RTF historically, though that's from a different code state) — none are directly usable as a post-/O2 baseline.

## What was shipped this session (chronological, with commit hashes)

### Stage 0 — Handy record enrichment (commit `926afdc`)
- Hoisted anti-halluc thresholds to `pub const ANTI_HALLUC_N_MAX_TEXT_CTX / _ENTROPY_THOLD` in `managers/transcription.rs` (128 and 2.8 — the hardcoded values the decoder actually uses).
- Added three fields to `BenchmarkRunRecord`: `effective_initial_prompt: Option<String>`, `effective_n_max_text_ctx: Option<i32>`, `effective_entropy_thold: Option<f32>`.
- Added helper `fn compute_effective_initial_prompt(s: &AppSettings) -> Option<String>` mirroring the `custom_words + "\n\n" + transcription_prompt` concat from `transcription.rs:557-572`.
- Populated in both success and pre-transcribe error paths (4 record-construction sites total).

### Stage 1 (B1) — C++ stderr log (whisper-rs-sys-fork `7e19fa8`, bumped to `0.15.1-lid-hack.2`)
- In `whisper.cpp/src/whisper.cpp` after `prompt_init` assembly in `whisper_full_with_state` (~line 6989): `WHISPER_LOG_INFO("%s: prompt_init size=%zu tokens=[%s]\n", __func__, ...)` with comma-separated token IDs. Fires once per transcribe. Single-line so `rg 'prompt_init size='` filters stderr cleanly.

### Stage 2a — C++ capture + FFI getter (whisper-rs-sys-fork `c2ebb62`, bumped to `0.15.1-lid-hack.3`)
- Added `std::vector<whisper_token> last_prompt_init;` member to `whisper_state` struct.
- Assignment `state->last_prompt_init = prompt_init;` in the primary SOT path (after B1 log).
- New C function `whisper_get_last_prompt_init(whisper_state*, int* out_count)` declared in `whisper.h`, implemented in `whisper.cpp`. Returns nullptr+0 on empty state.

### Stage 2b — whisper-rs wrapper (whisper-rs-fork `eb282bf`)
- `WhisperState::last_prompt_init(&self) -> Vec<WhisperTokenId>`. Safe wrapper around the unsafe FFI call; copies into owned Vec.

### Stage 2c — transcribe-rs pass-through (transcribe-rs-fork `8454e4e`)
- `WhisperEngine::last_prompt_init(&self) -> Vec<i32>`. Delegates to `WhisperState::last_prompt_init`. Mirrors existing `ctx_lang_token_id` pass-through pattern.

### Stage 2d — Handy capture into record (commit `fbca06f`)
- Added `last_whisper_prompt_init: Arc<Mutex<Option<Vec<i32>>>>` field to `TranscriptionManager`.
- In `managers/transcription.rs::transcribe`: after `whisper_engine.transcribe_with(...)` succeeds (or fails), read back `whisper_engine.last_prompt_init()` and store in the mutex.
- Also clear the mutex at start of `transcribe` and `transcribe_long_form` (so non-Whisper calls don't leak stale whisper-side data).
- Public getter `take_last_whisper_prompt_init(&self) -> Option<Vec<i32>>` on TranscriptionManager (consuming — takes from mutex).
- `BenchmarkRunRecord.decoder_prompt_init_tokens: Option<Vec<i32>>` field. Populated by calling the getter after each transcribe in benchmark.rs.

### Phase A matrix surgery (commit `848480e`)
- Narrowed RUN_MATRIX to 7 champion-candidate rows for a brief Phase A smoke test. Superseded later by `5be5a82`.

### Per-run per-input-file checkpoint (commit `0b92e95`)
- New `checkpoint_paths(output_dir, input_file) -> (PathBuf, PathBuf)` helper producing `benchmark-checkpoint-<sanitized_stem>.{json,md}`. Sp1/sp2 no longer clobber each other.
- `write_checkpoint` now takes pre-computed paths + flushes per-run (moved inside the inner `for run_idx` loop).
- Checkpoint files removed after successful final report write.

### Resume-from-checkpoint (commit `cc2035f`)
- New `BenchmarkOverrides.resume_from: Option<String>` field.
- Start of `benchmark_transcription_file`: if `resume_from` points to existing parseable file, seed `runs: Vec` with its non-errored records and build a HashSet of completed run-keys. Tolerant to missing/corrupt (warn, fresh start).
- Inner loop skips run_idx iterations whose full key is already in the HashSet.
- `BenchmarkReport / BenchmarkRunRecord / BenchmarkAggregate` made `Deserialize` (were `Serialize`-only).

### Superset matrix + only_conditions + prompt-aware resume (commit `5be5a82`)
- RUN_MATRIX expanded to 38 rows covering 8 Whisper models × 4 (use_prompt × ah) combos × needed LID variants + 3 non-whisper.
- New `BenchmarkOverrides.only_conditions: Option<Vec<ConditionFilter>>` — strict pin. Rows not in the list are silently skipped.
- `ConditionFilter` struct: `{ model_id, use_prompt, use_anti_halluc, sot_lang_tokens }`.
- Resume-key widened to include `effective_initial_prompt` AND `language`, so V1/V2/V3/V4 runs and ru/auto runs for the same matrix row are distinct and don't conflate during resume.
- Cargo.toml: **TEMPORARY** `devtools` feature on `tauri` so F12 works in the release build. Revert after night run completes.

### MSVC /O2 fix (whisper-rs-sys-fork `198f290`, bumped to `0.15.1-lid-hack.4`)
- In `build.rs` inside `if cfg!(target_os = "windows")`, added:
  ```rust
  config.define("CMAKE_C_FLAGS_RELEASE", "/MD /O2 /Ob2 /DNDEBUG");
  config.define("CMAKE_CXX_FLAGS_RELEASE", "/MD /O2 /Ob2 /DNDEBUG");
  ```
- **Why:** cmake 4.2.3 + VS2022 generator was producing vcxproj files with empty `<Optimization></Optimization>` for the Release config. Verified via `.tlog` that `ggml-cpu.c` was compiled without `/O2 /Ob2 /DNDEBUG`. This caused a ~1.67× slowdown on all whisper.cpp TUs relative to historical builds (large-model RTF 0.06 → 0.10). Post-fix confirmed at RTF 0.047 on the same condition.
- **Safe wrt numerics:** these flags affect only optimization/inlining, not FP semantics (no `/fp:fast`, no `/Qfast_transcendentals`). Bit-exact output preserved.
- **Rebuild required** because whisper.cpp TUs need to recompile with new flags. Full `bun run tauri build` took ~7 min.

## Pending cleanups before merging back to `bench/whisper-matrix` (or wider)

1. **RUN_MATRIX superset + only_conditions + matrix surgery commits** (`0b92e95`, `cc2035f`, `5be5a82`, and earlier `848480e`) — all were experiment-specific scaffolding. Before merging the core LID-hack feature + provenance chain upstream, revert the matrix surgery and keep the reusable infra (resume/checkpoint/only_conditions + provenance fields).
2. **Cargo.toml `devtools` feature on `tauri`** — still in place from commit `5be5a82`. Temporary to make F12 work on the release binary; revert before shipping.
3. **Post-hoc filter or re-run the sp1/sp2 Block C data** collected pre-/O2. sp1 large N=10 is entirely slow (RTF ~0.10); sp2 large is either 0 runs, a partial checkpoint, or 1-slow+9-fast depending on exact termination point. Re-running just those specific conditions is ~10 min at the fast binary.
4. **Night-run queue did not complete** — the 52-invocation master plan was not executed end-to-end. Either (a) re-kick it off on a fresh run with the fast binary, or (b) pick subset of blocks that matter most and run those. The queue-runner file at `C:/Users/Egor Sokolov/Documents/REAPER Media/night-run-queue.js` is ready to paste again; resume-from-checkpoint handles anything that happened to complete tonight.
5. **B3 / B4 from prior handoff backlog** — integrity diff + fidelity test in transcribe-rs. Not done this session; still deferred.

## Resuming the night-run queue (when user is ready)

Settings are currently in daily-use mode (`breeze-asr` + LID=["ru"] etc), NOT in the matrix-baseline state the queue expects. The queue-runner invocation's `SettingsGuard` will save whatever's in store at each invocation start and restore it on drop — so when queue runs, it starts from current (daily-use) settings, mutates per-row, restores to (daily-use). This is fine for the queue but means after the queue finishes, settings are back to `breeze-asr`+LID=["ru"]+... If user wants a different daily-use config, they'd set that AFTER the queue.

To resume the queue:
1. `D:\h\release\handy.exe` already running (or re-launch it)
2. F12 → Console → paste entire `C:/Users/Egor Sokolov/Documents/REAPER Media/night-run-queue.js` content
3. `DRY_RUN = false` already set
4. Resume-from-checkpoint will skip whatever completed tonight; runs the rest

## Safe checks (read-only, anytime):
```powershell
# What final reports from tonight exist?
Get-ChildItem "C:\Users\Egor Sokolov\Documents\REAPER Media\benchmark-results-20260423-*.json" | Select-Object Name, Length, LastWriteTime

# Any leftover checkpoints from aborted invocations?
Get-ChildItem "C:\Users\Egor Sokolov\Documents\REAPER Media\benchmark-checkpoint-*.json" -ErrorAction SilentlyContinue

# Current store settings snapshot (to verify daily-use config still intact)
Get-Content "$env:APPDATA\com.handy.app\settings_store.json" -Raw -ErrorAction SilentlyContinue | ConvertFrom-Json | Select-Object -ExpandProperty settings | Select-Object selected_model, selected_language, transcription_prompt, custom_words, whisper_anti_hallucination, whisper_sot_lang_tokens
```

(The store path may vary — check `%APPDATA%\com.handy.app\` or look up via Handy logs.)

## File paths for next agent

- **Night-run queue**: `C:/Users/Egor Sokolov/Documents/REAPER Media/night-run-queue.js`
- **This handoff**: `D:/dev/Handy/NEXT-AGENT-HANDOFF.md` (prepended; prior content below retained for history)
- **Plan file**: `C:/Users/Egor Sokolov/.claude/plans/d-dev-handy-next-agent-handoff-md-elegant-karp.md` (last updated with reliability + resume plan)
- **Release binary**: `D:/h/release/handy.exe` (with /O2 + devtools feature, built 2026-04-23 00:35)
- **Whisper.cpp fork tip**: `D:/dev/whisper-rs-sys-fork` @ `feature/sot-lang-tokens` @ `198f290`
- **Handy branch tip**: `D:/dev/Handy` @ `bench/whisper-matrix+lid-hack` @ `5be5a82`

---

# Previous handoff — LID-Hack Variance Experiment Continuation (archival)

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

## Group 1 results (FINISHED — actuals, not estimates)

**Reports** (in `C:/Users/Egor Sokolov/Documents/REAPER Media/`):
- `benchmark-results-20260422-204039.{json,md}` — sp1, 30 runs
- `benchmark-results-20260422-210338.{json,md}` — sp2, 30 runs

**Matrix used**: commit `63067ad` (the PRE-`0403ad8` surgery — 6 breeze rows: 3 noprompt+ah LID + 3 promptv2+ah LID, NO `None` baseline). Hot-reload didn't catch `0403ad8` before user pasted.

**Critical surprise**: user has **V2 prompt set in the Handy UI** (`transcription_prompt` settings field). The DevTools snippet used `overrides: {}` (no prompt override), so use_prompt=true rows fell back to `original_settings.transcription_prompt` = V2 string. Verified by inspecting JSON record `transcription_prompt` field.

**Actual data breakdown** (60 transcripts total):

| Slice | Count | Maps to Group 1 spec? |
|---|---:|---|
| noprompt+ah × 3 LID × 5 runs × 2 speakers | 30 | ✅ matches "Group 1 noprompt+ah variance" (3 of 4 LID modes) |
| **noprompt+ah × `None` LID × 5 × 2** | **0** | ❌ MISSING — needed for full Group 1 spec |
| V2+ah × 3 LID × 5 × 2 (bonus, accidental) | 30 | 🎁 useful contextual data, but NOT Group 2 Phase B (Phase B is V2+**noah**) |

**Implications for next agent**:
1. To complete Group 1 cleanly, run a single-row matrix surgery (just `breeze noprompt+ah None`) + invocation × 5 × 2 = 10 transcripts to fill the missing baseline. OR accept that the 30 noprompt+ah hack rows include enough determinism evidence to compute baseline indirectly from prior validation reports.
2. The 30 bonus V2+ah+LID rows are NOT a substitute for Group 2 Phase B (V2+noah+LID). Phase B still needs to be run.
3. **Tell the user to clear the UI prompt field** before running future invocations that pass `overrides: {}`, or always pass `overrides: { prompt: '' }` explicitly to neutralize.

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

## Backlog — rigorous parameter-provenance verification

**Context:** current `BenchmarkRunRecord` JSON records settings-layer values (what we wrote into the store before calling `transcribe()`). This matches what `transcription.rs:487 get_settings(...)` reads because benchmark.rs is single-threaded. But the chain settings → WhisperInferenceParams → FullParams → C FFI → whisper.cpp has three silent-transform points that the JSON does not prove:

1. **Resolution gap (`whisper_sot_lang_tokens` codes → token IDs):** in `transcription.rs:592-602`, codes that fail `ctx_lang_token_id()` are silently `filter_map`'d out. JSON shows the input strings; if any code didn't resolve, the actual FFI input was shorter or `None`. Benign for `ru`/`en` (always resolve in multilingual Whisper vocab), but a footgun for future experiments (`"zh"`, `"yue"`, typos).
2. **`transcription_prompt` ≠ actual `initial_prompt`:** the record stores the raw prompt field, not the concatenation `custom_words.join(", ") + "\n\n" + transcription_prompt`. benchmark.rs clears `custom_words = vec![]` on L744/L751, but this is an unchecked invariant: if it ever regresses, JSON would silently under-report.
3. **`use_anti_halluc` boolean, not applied thresholds:** JSON records `ah: true/false` but the actual `n_max_text_ctx=128`, `entropy_thold=2.8` values are hardcoded in `transcription.rs:577-586`. If those constants change (e.g. someone tunes to 256/3.0) the JSON won't reflect it.

### What to build (in order)

**B1 — C++ stderr log of prompt_init** (lowest level, smoking-gun evidence):
- In `whisper-rs-sys-fork/whisper.cpp/src/whisper.cpp:6954-6974` (primary SOT path, already patched), add a `WHISPER_LOG_INFO` after `prompt_init` is fully assembled, printing its length and the numeric tokens. Something like:
  ```cpp
  WHISPER_LOG_INFO("%s: prompt_init (%d tokens):", __func__, (int)prompt_init.size());
  for (size_t i = 0; i < prompt_init.size(); ++i) {
      WHISPER_LOG_INFO(" %d", prompt_init[i]);
  }
  ```
  Not just on the LID path — unconditional, so even baseline runs log their prompt_init (which proves zero-regression too). Commit on `feature/sot-lang-tokens` branch of whisper-rs-sys-fork.
- This appears in Handy's tauri dev stderr. We can grep the log file to verify per-run what tokens went to the decoder.

**B2 — Structured prompt_init capture into BenchmarkRunRecord:**
- This is more work: need a way for Rust to READ OUT the tokens that whisper.cpp actually received. Two options:
  - (a) Add a new FFI getter `whisper_get_last_prompt_init(ctx, out_tokens, out_count)` that returns the prompt_init from the last `whisper_full()` call. Requires whisper.cpp to store it on the ctx (new member) and expose it. Bigger change.
  - (b) Capture the stderr from B1 and parse it out-of-band. Ugly but doesn't require a new FFI.
  - (c) Do the check ONLY in debug_assertions: stash the expected token sequence on the Rust side before calling `state.full()` and assert inside a test. Doesn't solve "is this the same thing the decoder saw" — that requires (a).
- Recommend (a). Add new fields to `BenchmarkRunRecord`:
  ```rust
  /// Token IDs actually submitted to the decoder's prompt_init (vs sot_lang_tokens
  /// which is the settings-layer codes). Lets us verify the FFI received what we
  /// intended without relying on log parsing. None = not captured (e.g. non-Whisper
  /// engine, or captured-via-FFI failed).
  decoder_prompt_init_tokens: Option<Vec<i32>>,
  /// Effective initial_prompt string as built by transcription.rs (custom_words +
  /// transcription_prompt concatenation), so the record reflects what whisper.cpp
  /// actually saw, not just the raw settings field.
  effective_initial_prompt: Option<String>,
  /// Effective anti-halluc thresholds as applied. None = not set. Records the
  /// numeric values to survive future changes to the hardcoded 128/2.8 defaults.
  effective_n_max_text_ctx: Option<i32>,
  effective_entropy_thold: Option<f32>,
  ```
- Requires wiring through transcribe-rs (return the effective params alongside the text) or just computing them in benchmark.rs before/after transcribe.

**B3 — Integrity invariant check in benchmark.rs:**
- After each transcribe() call, compare the `applied_*` snapshot (what we wrote) to the `effective_*` / `decoder_prompt_init_tokens` (what actually ran). Log a warning (or fail the record) if they diverge. Catches silent regressions in the chain without requiring a full re-run.

**B4 — Fidelity test:**
- Unit test in `transcribe-rs-fork` that constructs known-good token IDs, sets them via `set_sot_lang_tokens`, runs a tiny mock transcription, and inspects the resulting prompt_init via B2's FFI getter. Guards the chain from silent layer-shuffle regressions.

### Priority

- **B1 first** (smallest diff, biggest evidence payoff): ~15 lines of C++, one commit on whisper-rs-sys-fork feature branch, rebuild. Ready for visual verification via logs on the next experiment.
- **B2 (a)** when rigor matters (e.g. publishing results, champion-candidate evaluation): ~50 lines across 4 crates.
- **B3** after B2: ~20 lines in benchmark.rs.
- **B4** before merging the LID-hack feature branch: ~30 lines of test code.

### Rationale for not doing this inline this session

User explicitly asked for runtime variance experiments, not chain-hardening. Current empirical evidence (Stage 1 latency spike + variance determinism asymmetry + token-choice flip) is strong indirect proof that the patches reach the decoder for the specific codes (`ru`, `en`) under test. B1 is the cheapest way to upgrade from "strong indirect" to "observed directly" and should be done before expanding to exotic code sets (`zh`, rare languages) where silent resolution failures become plausible.
