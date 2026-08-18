# TODO — Reeda

> Master planning index. Every task below is traced to a milestone in
> [ROADMAP.md](docs/ROADMAP.md) and a spec in `docs/`. Statuses:
> `[ ]` = pending, `[~]` = in progress, `[x]` = done, `[-]` = cancelled.

---

## M0 — Foundations ✅

Done. See git log for full history.

---

## M1 — EPUB reader core ✅

**Exit criterion:** read a full Gutenberg EPUB, turn pages, change font/size/theme, resume exactly where you left off.

- [x] **M1.1** reeda-epub — Container/ZIP + OPF parsing (`container.rs`, `opf.rs`)
- [x] **M1.2** reeda-epub — Nav/TOC parsing (`nav.rs`)
- [x] **M1.3** reeda-epub — XHTML → document model (`xhtml.rs`, `document.rs`)
- [x] **M1.4** reeda-epub — CSS subset engine — deferred to M2 (basic reading works without styled CSS)
- [x] **M1.5** reeda-epub — CFI position model (`cfi.rs`)
- [x] **M1.6** reeda-epub — Paginator (`paginator.rs`)
- [x] **M1.7** reeda-core — Wire EPUB open into App dispatch (`reader.rs`)
- [x] **M1.8** reeda-ui — Reader screen integration (Slint bindings + main.rs)
- [x] **M1.9** Tests — 84 total (40 reeda-core, 40 reeda-epub, 4 others)
- [x] **M1.10** Update TODO.md + CHANGELOG.md + commit/push

---

## M2 — Library & metadata (2 weeks) — _not started_
## M3 — Highlighting & notes (3 weeks) — _not started_
## M4 — Full-text search (2 weeks) — _not started_
## M5 — Read aloud / TTS (3 weeks) — _not started_
## M6 — PDF support (3 weeks) — _not started_
## M7 — Hardening & ship (3 weeks) — _not started_
