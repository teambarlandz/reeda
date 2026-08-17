# Read-Aloud (TTS) Specification — Reeda

> Status: draft · Version: 0.2 · Owner: @teambarlandz · Last updated: 2026-08-17
> Implementation: `reeda-tts` (Android `TextToSpeech` via JNI — ADR-008).
> Feature requirements: PRD §3.6 (TTS-01…TTS-08).

## 1. Scope

Read-aloud for **EPUB** content in v1 (reflowable text → chunked speech).
PDF narration (TTS-07) is P2 (needs `reeda-pdf` text extraction, PDF_SPEC §6).

## 2. Platform integration (Android)

- Java shim (≤ ~100 lines, `android/`): wraps
  `android.speech.tts.TextToSpeech` (initialization, `speak`,
  `stop`, `pause`?→ handled via queueMode + stop; `setSpeechRate`,
  `setPitch`, voice via `Voice` selection or system settings intent),
  `AudioAttributes` (USAGE_MEDIA, CONTENT_TYPE_SPEECH), and
  `UtteranceProgressListener` (`onStart`, `onDone`, `onError`,
  `onRangeStart(start, end, frame)`).
- Rust side (`android_bridge.rs`, `jni` crate): JNIEnv obtained via
  `android-activity`'s `vm`/`activity` handles; all calls marshalled off the
  UI thread (binder thread + channel).
- **Foreground service** (`foregroundServiceType="mediaPlayback"`):
  created when narration starts, with a media-style notification
  (play/pause/stop/skip-back/skip-forward, speed stepper) whose actions
  dispatch back to the engine via pending intents → JNI. Stopped when
  narration ends.
- **Audio focus** (TTS-08): `requestAudioFocus(AUDIOFOCUS_GAIN)`; on
  `AUDIOFOCUS_LOSS` → pause; on `TRANSIENT` → pause+resume; on
  `DUCK` → lower volume via `AudioAttributes`.
- **Wake lock**: partial wake-lock while narrating (TTS-04) only if user
  setting enabled; screen-on is default off (screen can sleep; service
  continues).

## 3. Chunking & text preparation

- Source: current chapter's `DocumentModel` rendered to plain text with
  formatting-stripped sentences; per-paragraph granularity.
- Rules:
  - Sentence boundary detection (`. ! ? …` + closing quotes) with
    abbreviation guard-list (Mr., Dr., etc. — locale-aware later).
  - Chunk max 4000 chars / ~30 s at 1× (TTS engine limits); chunk boundary
    forced at sentence boundary; paragraphs are not split unless > limit.
  - Skip: footnotes (unless option), captions (option), `[TOC]` items,
    repeated headers (chapter title repeated each page → speak once).
  - Strip: markdown artifacts, soft hyphens, `&nbsp;`, control chars.
- Each chunk maps to CFI via `reeda-epub::locate` (offset → CFI) so word
  highlight + page turns are exact (TTS-05).

## 4. Utterance model & queue

- One Android `speak()` per chunk, `QUEUE_ADD`, unique monotonically
  increasing utterance id; `UtteranceId → { chunk_id, cfi_range }` map in
  the engine.
- Engine queue depth: 2 (current + next prefetched to mask latency);
  `onStart` → engine marks chunk "speaking".
- Speed: engine-level `setSpeechRate` (0.5×–2.5×, step 0.1, persisted per
  user). Pitch 0.5–1.5.
- Voice: expose system TTS settings screen (`ACTION_TTS_SETTINGS`) (TTS-06);
  selected voice read back via `getVoices`.

## 5. Narration state machine

```
            ┌────────┐
   Start ▶ │ Idle   │── Stop/Done/Error(final) ─▶ Idle
            └───┬────┘
                │ StartNarration
            ┌───▼────┐
            │ Loading│  (chunk 1 built)
            └───┬────┘
            ┌───▼──────┐   speak()                    ┌──────────┐
            │ Speaking │ ◀──────────────────────────▶ │  Paused  │
            └───┬──────┘    pause()/resume()          └──────────┘
                │ onDone(chunk n) → advance to n+1; if chapter end → next
                │   chapter's first chunk (auto-continue, per setting)
                ▼
        (loop to Speaking)  ·  onError → retry policy
```

**Retry policy**: transient error → skip chunk (log); 3 consecutive errors →
`Paused` + user notification `NarrationState::Error(msg)`.

**Page sync**: when narration crosses a page boundary (chunk CFI > page-end
CFI), engine emits `TurnPage` command to core → reader auto-advances
(TTS-05). Word highlight: `onRangeStart` → engine emits `WordHighlight { cfi,
(offset_start, offset_end) }`; UI scrolls highlighted line into view (no
smooth scroll during TTS).

## 6. Controls & UI

- Reader chrome TTS bar: play/pause, stop, −15 s/+15 s (within chunk →
  re-utter at offset), skip chapter fwd/back, speed chip (cycles 0.5–2.5).
- Notification (locked screen) mirrors the same controls.
- State surfaced via `NarrationState` in StateSnapshot; every transition
  updates both bar and notification.
- End-of-book: stop, toast "Book finished", clear notification.

## 7. Errors & edge cases

| Case | Behavior |
|------|----------|
| No TTS engine / init failed | Error dialog with "Open system TTS settings" |
| TTS language missing voice | Fallback locale chain: book lang → system lang → en-US |
| Engine callback without utterance | Ignored + warn |
| Rotation during narration | State in App (not UI) → UI rebuilds controls; narration continues |
| App killed while narrating | Service keeps speaking via notification; book position saved on page-turn |
| Battery saver / Doze | Partial wake-lock + foreground service → exempt; document caveat |
| Chapter with only images | Skip to next text chapter, log |

## 8. Testing

- Unit: chunker (sentence detection, limits, CFI mapping), state machine
  (all transitions + error paths) with a `FakeTtsHost` recording calls.
- Instrumented (emulator): real TTS init, speak/chunk flow, notification
  actions, focus loss (simulate `AUDIOFOCUS_LOSS` via `am broadcast`),
  rotation, lock screen, Doze.
- Golden: chunk CFI mapping for fixtures must match paginator output.
