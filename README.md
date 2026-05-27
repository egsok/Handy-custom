# Handy-custom

> Personal fork of [cjpais/Handy](https://github.com/cjpais/Handy) with Russian-language transcription tweaks.
> Pre-built installers (unsigned) on [Releases](https://github.com/egsok/Handy-custom/releases). Build from source for everything else.
> [English] · [Русский](README.ru.md)

This is my personal daily-driver fork of [Handy](https://github.com/cjpais/Handy), the offline speech-to-text Tauri app. I use it for Russian transcription on Windows. The fork stays close to upstream — I cherry-pick fixes and add small features that solve concrete problems I hit. Nothing here is meant to replace upstream Handy; if you don't have the same Russian-specific pain points, just use the original.

Fork is based on upstream `main` HEAD `10a4c31` (5 docs commits past `v0.8.3`).

## What's different from upstream

- **Custom transcription prompt.** Per-language initial prompt field in Settings → Advanced for Whisper models. Useful for forcing recognition of names, jargon and stylistic preferences.
- **Multi-format clipboard preservation.** Paste-and-restore now preserves Files / Image / HTML / Text — not just the plain-text payload. Lets you keep a clipboard you copied earlier even if Handy hijacks the buffer to inject the transcript.
- **Anti-hallucination toggle for Whisper.** Settings → Advanced switch that applies `n_max_text_ctx=128` + `entropy_thold=2.8` to `WhisperInferenceParams` (equivalent of OpenWhispr PR #552 / whisper.cpp#1507). Kills runaway loop hallucinations on long silences ("и я хочу и я хочу...").
- **Cyrillic word-boundary fixes for Breeze ASR.** Five regex passes unglue words that the Mandarin-trained Breeze model joins together (`cyr.cyr`, `lat.cyr`, `cyrCYR`, `latCYR`, single-letter variants, uppercase Latin acronyms). Gated on `selected_model == "breeze-asr"`, pure post-process in `src-tauri/src/audio_toolkit/text.rs`.
- **`/O2` compiler fix in the whisper-rs-sys fork.** cmake 4.2.3 + VS2022 silently drops `CMAKE_*_FLAGS_RELEASE` initializers, which builds whisper.cpp at `/Od` instead of `/O2`. The fork explicitly sets `/MD /O2 /Ob2 /DNDEBUG`. Empirical impact: `large-v3` RTF 0.10 → 0.06 (~1.67× faster Whisper inference on Windows).

The first four live as commits on top of upstream in this repo. The `/O2` fix lives in three sibling forks of the whisper Rust stack (`whisper-rs-sys-fork`, `whisper-rs-fork`, `transcribe-rs-fork`, all on `daily-stable` branches), wired in via `[patch.crates-io]` paths in `src-tauri/Cargo.toml`. Dropping those patches removes both the `/O2` speedup AND the anti-hallucination backend.

## Download

Pre-built installers are published to [Releases](https://github.com/egsok/Handy-custom/releases).

- **Windows:** download `Handy_0.8.3-N_x64-setup.exe` (NSIS) or `.msi` (N is the fork release number — 1, 2, ...). On first launch Windows SmartScreen will show "Windows protected your PC" — click **More info** → **Run anyway**. The binary is unsigned (see Build below).
- **Linux:** download `Handy_..._amd64.deb` / `.AppImage` / `.rpm` for your distro.
- **macOS (experimental):** download `Handy_..._x64.dmg` (Intel) or `Handy_..._aarch64.dmg` (Apple Silicon). Gatekeeper will refuse to launch the unsigned app — workaround: in Finder, right-click `Handy.app` → **Open** → **Open** again in the confirmation dialog. Or from terminal: `xattr -d com.apple.quarantine /Applications/Handy.app`. Note: macOS builds in v1 are CI-built but not tested by me — please report breakage in [issues](https://github.com/egsok/Handy-custom/issues).

## Build

If you want the bleeding edge, a platform not covered by releases, or want to audit the build yourself, build locally. (Otherwise grab a pre-built installer from [Download](#download) above.)

1. Follow upstream's [BUILD.md](BUILD.md) for platform prerequisites.
2. Make sure the three sibling forks are checked out next to `Handy/` and on the `daily-stable` branch (see `[patch.crates-io]` section of `src-tauri/Cargo.toml`).
3. On Windows, set `CARGO_TARGET_DIR=d:/t/handy` (short path; whisper-rs-sys Vulkan shader paths overflow MAX_PATH otherwise) and `CARGO_BUILD_JOBS=2` (release profile uses `lto=true`, parallel link OOMs on 16 GB).
4. `bun install && bun run tauri build`. Ignore the final "failed to bundle project: program not found" — that's the custom-signing step, the raw `.exe` is fine.

## Upstream

This fork tracks [cjpais/Handy](https://github.com/cjpais/Handy). For everything not listed above — installation, troubleshooting, platform-specific notes, model management, signal handling, CLI flags — see the [upstream README](https://github.com/cjpais/Handy/blob/main/README.md). I don't duplicate that here so it doesn't go stale relative to upstream.

If you want the official, supported app: get it from [handy.computer](https://handy.computer) or [cjpais/Handy/releases](https://github.com/cjpais/Handy/releases).

## Author

Built by [Egor Sokolov](https://egorsokolov.ru/) — 10 years in product (Sberbank, Rolf, Claustrophobia). Writing and experimenting with AI tooling — mostly Claude Code, Codex, and dev workflow tooling. I use Handy daily for Russian voice notes; this fork is what fell out of that.

Telegram channel about AI tooling: [@neiroset_ne_vinovata](https://t.me/neiroset_ne_vinovata).

Other open-source experiments: [plan-tango](https://github.com/egsok/plan-tango) — a Claude ↔ Codex plan-review loop for Claude Code.

## License

MIT, inherited from cjpais/Handy — see [LICENSE](LICENSE).
Original work © cjpais and contributors. Fork modifications © 2026 Egor Sokolov.
