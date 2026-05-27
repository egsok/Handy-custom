# Handy-custom

> Персональный форк [cjpais/Handy](https://github.com/cjpais/Handy) с правками под русскую транскрипцию.
> Готовые установщики (без подписи) — на [Releases](https://github.com/egsok/Handy-custom/releases). Сборка из исходников — для всего остального.
> [English](README.md) · [Русский]

Это мой персональный daily-форк [Handy](https://github.com/cjpais/Handy) — оффлайн speech-to-text приложения на Tauri. Использую для русской транскрипции под Windows. Форк держится близко к upstream: cherry-pick'и фиксов плюс небольшие фичи под конкретные проблемы, которые ловлю в работе. Замена upstream это не пытается быть — если у тебя нет таких же специфических болячек на русском, просто бери оригинал.

База: upstream `main` HEAD `10a4c31` (+5 docs-коммитов после `v0.8.3`).

## Чем отличается от upstream

- **Custom transcription prompt.** Поле под язык в Settings → Advanced для Whisper-моделей — задаёшь initial prompt. Полезно, чтобы вытаскивать имена, термины и стилистику.
- **Multi-format clipboard preservation.** Paste-and-restore теперь сохраняет ВСЕ форматы буфера (Files / Image / HTML / Text), не только plain text. Если в буфере было что-то посерьёзнее текста, и Handy перехватил буфер для вставки транскрипта — после возврата не теряется.
- **Anti-hallucination toggle для Whisper.** Переключатель в Settings → Advanced применяет `n_max_text_ctx=128` + `entropy_thold=2.8` к `WhisperInferenceParams` (эквивалент OpenWhispr PR #552 / whisper.cpp#1507). Гасит зацикленные галлюцинации на длинных тишинах ("и я хочу и я хочу...").
- **Cyrillic word-boundary фиксы для Breeze ASR.** Пять regex-проходов разлепляют слова, которые Mandarin-trained Breeze склеивает (`cyr.cyr`, `lat.cyr`, `cyrCYR`, `latCYR`, single-letter варианты, uppercase Latin аббревиатуры). Гейтится `selected_model == "breeze-asr"`, чистый post-process в `src-tauri/src/audio_toolkit/text.rs`.
- **`/O2` фикс компилятора в whisper-rs-sys форке.** cmake 4.2.3 + VS2022 молча дропает `CMAKE_*_FLAGS_RELEASE` инициализаторы → whisper.cpp собирается с `/Od` вместо `/O2`. Форк явно задаёт `/MD /O2 /Ob2 /DNDEBUG`. Эмпирически проверено: `large-v3` RTF 0.10 → 0.06 (~1.67× быстрее Whisper-инференса на Windows).

Первые четыре — коммиты поверх upstream в этом репозитории. `/O2` фикс лежит в трёх соседних форках Rust-стека whisper (`whisper-rs-sys-fork`, `whisper-rs-fork`, `transcribe-rs-fork`, все на ветке `daily-stable`), подключаются через `[patch.crates-io]` пути в `src-tauri/Cargo.toml`. Если убрать эти patch'и — отвалятся и `/O2`, и backend для anti-hallucination.

## Загрузка

Готовые установщики публикуются в [Releases](https://github.com/egsok/Handy-custom/releases).

- **Windows:** скачай `Handy_0.8.3-N_x64-setup.exe` (NSIS) или `.msi` (N — номер форк-релиза: 1, 2, ...). При первом запуске Windows SmartScreen покажет "Windows protected your PC" — кликни **More info** → **Run anyway**. Бинарь не подписан (см. Сборка ниже).
- **Linux:** скачай `Handy_..._amd64.deb` / `.AppImage` / `.rpm` под свой дистрибутив.
- **macOS (экспериментально):** скачай `Handy_..._x64.dmg` (Intel) или `Handy_..._aarch64.dmg` (Apple Silicon). Gatekeeper откажется запускать неподписанное приложение — обход: в Finder right-click `Handy.app` → **Open** → **Open** в подтверждающем диалоге. Или из терминала: `xattr -d com.apple.quarantine /Applications/Handy.app`. Замечание: macOS сборки в v1 собираются CI, но мной не тестируются — баги репортить в [issues](https://github.com/egsok/Handy-custom/issues).

## Сборка

Если хочешь bleeding edge, платформу не покрытую релизами или сам проверить билд — собирай локально. (Иначе бери готовый установщик из [Загрузка](#загрузка) выше.)

1. Платформенные пререквизиты — в upstream [BUILD.md](BUILD.md).
2. Три соседних форка должны лежать рядом с `Handy/` и быть на ветке `daily-stable` (см. секцию `[patch.crates-io]` в `src-tauri/Cargo.toml`).
3. На Windows ставь `CARGO_TARGET_DIR=d:/t/handy` (короткий путь — Vulkan-шейдеры whisper-rs-sys иначе упираются в MAX_PATH) и `CARGO_BUILD_JOBS=2` (release-профиль с `lto=true`, параллельный линк OOM'нется на 16 ГБ).
4. `bun install && bun run tauri build`. Финальное "failed to bundle project: program not found" игнорируй — это custom-signing step, сам `.exe` собран нормально.

## Upstream

Форк трекает [cjpais/Handy](https://github.com/cjpais/Handy). Всё, что не упомянуто выше — установка, troubleshooting, платформенные заметки, управление моделями, signal handling, CLI-флаги — смотри в [upstream README](https://github.com/cjpais/Handy/blob/main/README.md). Дублировать не стал, чтобы не разъезжалось с актуальной версией.

Если нужен официальный поддерживаемый Handy — забирай с [handy.computer](https://handy.computer) или со страницы релизов [cjpais/Handy/releases](https://github.com/cjpais/Handy/releases).

## Автор

Сделал [Егор Соколов](https://egorsokolov.ru/) — 10 лет в продукте (Сбер, Рольф, Клаустрофобия). Пишу и экспериментирую с AI-инструментами — в основном Claude Code, Codex и dev-воркфлоу. Сам пользуюсь Handy для русских голосовых заметок; этот форк — то, что из этого выпало в код.

Telegram-канал про AI-инструменты: [@neiroset_ne_vinovata](https://t.me/neiroset_ne_vinovata).

Другие открытые эксперименты: [plan-tango](https://github.com/egsok/plan-tango) — Claude ↔ Codex ревью-цикл для планов в Claude Code.

## Лицензия

MIT, наследуется от cjpais/Handy — см. [LICENSE](LICENSE).
Исходная работа © cjpais и контрибьюторы. Правки форка © 2026 Егор Соколов.
