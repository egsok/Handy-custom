## PR Title

feat: add custom transcription prompt setting for Whisper models

## PR Body

### Problem

Whisper often drops punctuation entirely, producing a wall of unformatted text — especially for non-English languages. This is a well-known issue in the community:
- [openai/whisper#557](https://github.com/openai/whisper/discussions/557) [openai/whisper#194](https://github.com/openai/whisper/discussions/194) — punctuation loss discussions
- [OpenAI Whisper Prompting Guide](https://developers.openai.com/cookbook/examples/whisper_prompting_guide) — official guidance on using `initial_prompt` to steer style

The documented solution: pass a well-punctuated paragraph as `initial_prompt`. Whisper doesn't follow instructions — it copies the *style* of the prompt. A paragraph full of commas, question marks, and em-dashes nudges the decoder to keep producing punctuation.

### Why not just hardcode a default prompt

The prompt also acts as a language hint. A hardcoded prompt in one language biases Whisper's language detector — e.g., an English prompt causes Russian speech to be transcribed as English when the language is set to "Auto". So pre-filling a default prompt would break the experience for anyone using auto-detection with a non-English language.

Worth noting: a prompt in one language doesn't prevent Whisper from recognizing words in another. For example, a Russian prompt with language set to "Auto" works fine for mixed Russian/English speech — English words are still transcribed correctly.

In the future, we could consider auto-populating the prompt based on the selected transcription language — but for now, empty by default is the safe choice.

### Solution

A new **Transcription Prompt** setting under Settings → Advanced that lets users provide a sample text to guide Whisper's output style.

**Key design decisions:**

- **Empty by default** — no language bias introduced for users who don't need it. When Transcription Language is set to "Auto", a non-empty prompt in a specific language can reduce language detection accuracy, so opting in is intentional.
- **10-language preset dropdown** (EN, ES, FR, DE, PT, IT, RU, JA, ZH-CN, ZH-TW) — each preset uses native punctuation conventions (Russian «ёлочки», German „Gänsefüßchen", French « guillemets », Japanese 「括弧」, Chinese ""引号""). Users can also write their own prompt.
- **Token-aware budget** — Whisper's `initial_prompt` window is 224 tokens. Custom Words are prepended and share that budget, so the prompt is capped at 112 estimated tokens (half). A per-script token estimator (CJK ~2.2 tok/char, Cyrillic ~0.5, Latin ~0.25) enforces the limit in real time. A progress bar with color coding (gray → yellow at 80% → red at 95%) replaces the old character counter. A hint below the bar explains the shared budget: "a shorter prompt leaves more room for custom words."
- **Whisper-only** — this setting only affects Whisper models. When a non-Whisper model is selected (Parakeet, GigaAM, Moonshine, Canary, Cohere, SenseVoice), a warning is shown. The setting remains visible and persisted so users don't lose their prompt when switching models.

### Changes

**Backend (Rust):**
- `settings.rs` — new `transcription_prompt: Option<String>` field
- `transcription.rs` — concatenate prompt after custom words into `initial_prompt`. Custom prompt is placed last — Whisper truncates from the left, so dictionary words (lower priority) get truncated first
- `shortcut/mod.rs` — new `update_transcription_prompt` command
- `lib.rs` — register the command

**Frontend (TypeScript/React):**
- New `TranscriptionPrompt.tsx` component with preset dropdown, textarea, token-aware progress bar, and contextual warnings
- `AdvancedSettings.tsx` — include the new component
- `settingsStore.ts` — wire up the setting to the backend command
- `bindings.ts` — add `transcription_prompt` to `AppSettings` and `updateTranscriptionPrompt` command
- `en/translation.json` — all user-facing strings

### Test plan

- [ ] Select a Whisper model → setting is shown without warnings
- [ ] Select a non-Whisper model → yellow "Whisper only" warning appears below
- [ ] Pick a language preset → textarea populates with sample text
- [ ] Switch preset to "None" → textarea clears, setting is saved as null
- [ ] Type a custom prompt → saved on blur, persists across app restart
- [ ] Set Transcription Language to "Auto" with a non-empty prompt → language detection warning appears
- [ ] Set Transcription Language to a specific language → no warning
- [ ] Transcribe with Russian preset on Whisper Large → punctuation (commas, periods, em-dashes, «quotes») present in output
- [ ] Transcribe with empty prompt → default Whisper behavior (may lack punctuation)
- [ ] Verify Custom Words still work alongside the prompt
- [ ] Token budget hint appears when prompt is non-empty
- [ ] Progress bar fills slowly for Latin text (~0.25 tok/char), fast for CJK (~2.2 tok/char)
- [ ] At 100% budget → typing blocked; deleting text → bar decreases, typing resumes
- [ ] Each preset fills the bar to ≤69% (leaving room for Custom Words)
- [ ] Bar turns yellow at 80%, red at 95%
