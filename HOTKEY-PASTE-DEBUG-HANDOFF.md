# Hotkey + Paste debug handoff

**Date:** 2026-04-29
**Branch:** `bench/whisper-matrix+lid-hack`
**Build:** `D:\h\release\handy.exe` (rebuilt several times today; latest at commit `1443467`)
**Status:** instrumentation deployed, root cause for both issues NOT identified, recovery tools work, permanent fixes deferred

## Two open bugs

### Bug 1 — Hotkey transient deafness (PRIORITY: med-high)

**Symptom:** Pressing the configured global hotkey (`Ctrl+Space`, HandyKeys backend) sometimes does nothing — overlay doesn't appear, tray icon stays Idle. Window of broken behavior is variable. Today (2026-04-29) confirmed multiple occurrences in ~1 hour of normal use. **"Re-register Hotkeys" tray menu item reliably restores function** every time.

**Pre-existing:** observed for "couple of days" before this session; the April-24 release binary had this bug too. NOT introduced by this session's commits (cyr-fix, anti-halluc UI, LID UI, upstream merge, debug instrumentation).

**Diagnosis from logs:**
- Between last successful paste and next one, ZERO `handy-keys event` log lines appear
- Coordinator stays in `Idle`, ready for input
- The OS-level `WH_KEYBOARD_LL` hook stops delivering events to the handy-keys worker thread
- After `force_reinit`, `HotkeyManager (WH_KEYBOARD_LL hook) ready in 0ms` — instant reinstall, no AV interference
- Sometimes auto-recovers without user action; sometimes doesn't until manual Re-register

**Eliminated causes:**
- VS Code window-level hooks (can't suppress `WH_KEYBOARD_LL`, which fires at kernel)
- GitHub Copilot — user doesn't use it
- Visible UAC prompts, Win+L lock, RDP, secure-desktop transitions — user didn't observe any
- Screen recorder / streaming software changes — environment stable
- Today's commits — bug predates them all

**Plausible remaining causes:**
- handy-keys 0.2.4 internal hook callback regression / queue-mediation bug
- Windows `LowLevelHooksTimeout` accumulated misses leading to unhook (5s default, sometimes lower; if our process briefly slow during transcription burst, OS may flag and eventually unregister)
- Invisible secure-desktop transitions (Windows Hello, smart card prompts, etc.) that user didn't notice but suspended low-level hooks briefly

**What's already done in code (commit `1443467 debug(hotkey)`):**
- Event-flow instrumentation at info level: `shortcut event: ...`, `coordinator: input(...) | stage=...`, `coordinator: stage Recording/Processing/Idle (Processing took Nms)`, `HotkeyManager ready in Nms`, first-event-since-init one-shot log
- Tray menu items: `Force Reset Pipeline` (resets coordinator from stuck Processing) + `Re-register Hotkeys` (full hook teardown + reinit via `HandyKeysState::reinstall_hook` + re-register all bindings)
- Tauri commands `force_coordinator_reset` and `force_reinit_shortcuts` exposed for any future UI
- New `Command::ForceIdle` in coordinator (bypasses Cancel's Processing-state guard)
- `utils::force_reset_to_idle` helper

**Next-session work plan (recommended): implement auto-watchdog**

User originally deferred this thinking the bug was rare, but multiple occurrences in 1 hour confirm it's worth fixing.

Spec:
1. Add `last_event_at: Arc<AtomicI64>` to `HandyKeysState` (Unix millis, updated in worker thread on every event in [handy_keys.rs:128-155](src-tauri/src/shortcut/handy_keys.rs#L128-L155))
2. Add `watchdog_running: Arc<AtomicBool>` for clean shutdown
3. Add `is_idle()` getter to `TranscriptionCoordinator` so watchdog can avoid reinit during recording (uses an atomic mirror of `stage`, since the stage is currently inside the closure)
4. Spawn watchdog thread in `HandyKeysState::new()`:
   ```rust
   loop {
       thread::sleep(Duration::from_secs(30));
       if !watchdog_running.load(Relaxed) { break; }
       let elapsed_ms = now_ms() - last_event_at.load(Relaxed);
       if elapsed_ms > 5 * 60 * 1000 {  // 5 minutes
           if app.try_state::<TranscriptionCoordinator>().map_or(true, |c| c.is_idle()) {
               info!("Watchdog: {}s without hook events, auto-reinit", elapsed_ms / 1000);
               let _ = crate::shortcut::force_reinit(&app);
               last_event_at.store(now_ms(), Relaxed);
           }
       }
   }
   ```
5. In `Drop for HandyKeysState`, set `watchdog_running` to false so thread exits cleanly
6. Tune threshold based on observation — start with 5min, lower if too tolerant

Estimated: ~50 lines, ~30 min implementation, one rebuild. Files touched: `handy_keys.rs`, `transcription_coordinator.rs` (add `is_idle` getter).

**Alternative (more invasive but more surgical):** listen for Windows session change events (`WM_WTSSESSION_CHANGE`) via `windows` crate and reinit on `SESSION_UNLOCK`/`SESSION_REMOTE_CONNECT`. Better signal but more code; doesn't cover the LowLevelHooksTimeout case.

**Defer if:** the watchdog is implemented and triggers fix the issue; if it doesn't help (events stop arriving but `force_reinit` doesn't restore hook), root cause is deeper and we need to fork handy-keys crate to instrument it.

---

### Bug 2 — Cyrillic paste → ????? (PRIORITY: low, RARE)

**Symptom:** Reproduced once on 2026-04-29 09:18. Whisper transcription was correct in logs:
```
Transcription result: Список идей для подарков Насте или совместных активностей. Кулинарный мастер-класс. Она очень хотела.
```

But the text pasted into Claude Desktop showed:
```
?????? ???? ??? ???????? ????? ??? ?????????? ???????????. ?????????? ??????-?????. ??? ????? ??????.
```

Each Cyrillic codepoint → `?`. ASCII (spaces, period, dash) preserved. **Frequency:** rare (once observed in ~hours of normal use today). Not annoying enough to block.

**Diagnosis:** classic UTF-8 → ANSI CP-1252 conversion loss somewhere in the clipboard write/read pipeline. User has `app_language: en-US` → Windows ANSI code page = CP-1252 (Western European), which physically cannot represent Cyrillic. When Cyrillic UTF-16 gets converted to CP-1252 (either by our code, by `arboard`, or by Windows auto-synthesizing CF_TEXT from CF_UNICODETEXT for legacy-app readers), Cyrillic codepoints become `?` fallback.

**Pipeline (clipboard.rs:104-158 paste_via_clipboard, Windows path):**
1. `save_clipboard()` via `arboard::Clipboard` (read existing buffer)
2. `app_handle.clipboard().write_text(text)` via tauri-plugin-clipboard-manager (which uses arboard internally)
3. `sleep 60ms` (`paste_delay_ms`)
4. `enigo` simulates Ctrl+V
5. Target app reads clipboard
6. `restore_clipboard(saved)` via arboard

**Three localization candidates:**
- A. `arboard 3.6` `set_text` on Windows writes CF_TEXT (ANSI) instead of/in addition to CF_UNICODETEXT, causing target apps that prefer CF_TEXT to get `?` versions
- B. Target app (Claude Desktop / Electron-based) requests only CF_TEXT instead of CF_UNICODETEXT; Windows auto-synthesizes CF_TEXT from our CF_UNICODETEXT using current ANSI codepage CP-1252 → loss
- C. Some race between `save_clipboard` (which reads as text → may degrade), `write_text`, and `restore_clipboard` (which writes back the degraded read) — though restore is AFTER paste, so unclear how it'd affect THIS paste

**Diagnostic test plan (3 minutes, do at start of next session if reproduced):**
1. Switch `Settings → Advanced → Output → Paste Method` to `Direct (Type)`. Direct uses `enigo SendInput KEYEVENTF_UNICODE` — bypasses clipboard. If Direct works correctly with Cyrillic → confirms clipboard pipeline is at fault (rules in A or B).
2. With current `Clipboard (Ctrl+V)` method, paste **into Notepad** (legacy Win32, prefers CF_UNICODETEXT). If Notepad shows correct Cyrillic → rules out A (our write is fine), pinpointing B (target-app specific). If Notepad ALSO shows `?????` → confirms A.
3. Open `Win+V` (Windows clipboard history) immediately after a transcription, BEFORE pasting. If history shows correct Cyrillic → buffer is fine, target app reads wrong (B). If `?????` already there → our write is wrong (A).

**Likely fixes once localized:**
- If A: bump `arboard` to latest (≥3.7?) and check changelog for clipboard format fixes; or pin a known-good version. Possibly upstream `tauri-plugin-clipboard-manager` regression — check if its version was bumped in the upstream merge.
- If B: switch to Direct paste auto-detect (if text contains non-ASCII, prefer Direct). Or write only CF_UNICODETEXT explicitly without letting Windows auto-synth CF_TEXT for the same paste session.

**Defer if:** user can't reproduce on demand and the workaround (manual repaste, or switch paste method temporarily) is acceptable for the low frequency.

---

## What was committed this session (chronological)

```
1443467 debug(hotkey): event-flow instrumentation + recovery tray actions
76655e5 fix(transcription): catch single-letter Cyrillic/Latin words after period
3df462b feat(settings): LID hack UI for Whisper code-switching
ff9904d Merge remote-tracking branch 'upstream/main' into bench/whisper-matrix+lid-hack
1b2af61 feat(settings): anti-hallucination UI toggle for Whisper
6890bd9 feat(transcription): cyrillic word-boundary fix for Breeze ASR
```

## Working tree state at handoff

Pre-existing bench WIP NOT committed (intentionally — user's parallel work):
- `NEXT-AGENT-HANDOFF.md` (bench notes)
- `src-tauri/Cargo.lock`
- `src-tauri/Cargo.toml`
- `src-tauri/src/commands/benchmark.rs`
- `src-tauri/src/lib.rs` — contains `HANDY_DISABLE_SINGLE_INSTANCE` env var escape hatch for D13 multi-process benchmarking, NOT to be touched

## Files of interest (pointers, not to memorize)

- [src-tauri/src/transcription_coordinator.rs](src-tauri/src/transcription_coordinator.rs) — pipeline state machine, has `Command::ForceIdle`, stage transition logs
- [src-tauri/src/shortcut/handy_keys.rs](src-tauri/src/shortcut/handy_keys.rs) — HandyKeys backend, has `reinstall_hook`, `force_reinit`, manager_thread loop, `HotkeyManager::new_with_blocking()` timing log
- [src-tauri/src/shortcut/handler.rs](src-tauri/src/shortcut/handler.rs) — `handle_shortcut_event` dispatch + info log
- [src-tauri/src/shortcut/mod.rs](src-tauri/src/shortcut/mod.rs) — backend dispatcher, has `force_reinit` for both Tauri and HandyKeys impls
- [src-tauri/src/utils.rs](src-tauri/src/utils.rs) — has `force_reset_to_idle` helper
- [src-tauri/src/commands/mod.rs](src-tauri/src/commands/mod.rs) — has `force_coordinator_reset`, `force_reinit_shortcuts` Tauri commands
- [src-tauri/src/clipboard.rs](src-tauri/src/clipboard.rs) — paste pipeline, `paste_via_clipboard`, save/restore_clipboard logic
- [src-tauri/src/tray.rs](src-tauri/src/tray.rs) — tray menu builder including new recovery items

## How to rebuild release after edits

```
CARGO_TARGET_DIR=d:/h bun run tauri build --no-bundle
```

NOT `cargo build --release` (gives ERR_CONNECTION_REFUSED — see memory `feedback_release_build.md`).
NOT default target dir (Windows MAX_PATH overflow in whisper-rs-sys Vulkan shaders — see `feedback_short_target_dir.md`).
~5-7 min build time.

## Investigation logs to look at if reproduced

`%APPDATA%\com.pais.handy\logs\handy.log` (or rotated `handy.YYYY-MM-DD.log`)

Watch for in normal flow:
```
shortcut event: binding=transcribe, hotkey=ctrl_left+space, pressed=true
coordinator: input(...) | stage=Idle
coordinator: stage Recording (binding=transcribe)
coordinator: stage Processing (binding=transcribe)
coordinator: stage Idle (Processing took Nms)
```

Hook deaf signature: gap of multiple seconds-minutes between successful paste and next hotkey activity, ZERO `handy-keys event` or `shortcut event` lines in that window.
