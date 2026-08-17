# Contributing — Reeda

> Status: draft · Version: 0.1 · Owner: @teambarlandz · Last updated: 2026-08-17
> Rules for every contributor. TL;DR: docs-first, PRs only, CI must pass,
> a11y in DoD.

## 1. Code of conduct

Be respectful; assume good faith; discuss design in issues before large
PRs. Harassment is not tolerated. (Formal CoC file to be added in M0 —
OQ-1 decision.)

## 2. Before you start

1. Read [TODO.md](../TODO.md) — it tracks every doc and its status.
2. Pick an issue labelled `good first issue` or discuss your idea in a
   new issue first (spec change = update the spec doc in the same PR).
3. **Docs-first rule**: a feature PR must include/update its spec
   (PRD line, EPUB_SPEC, TTS_SPEC, …). No spec, no merge.

## 3. Development workflow

- `main` is protected: **PRs only, 1 approval, CI green**. No direct
  pushes (BUILD_CI.md §1).
- Branch: `feat/xxx` (feature), `fix/xxx`, `docs/xxx`, `chore/xxx`.
- PR body template (required):
  ```
  ## What / Why
  ## Spec changes (link to doc lines)
  ## Tests added/changed
  ## Checklist: [ ] fmt [ ] clippy -D warnings [ ] tests [ ] docs/TODO.md updated
  ```
- Keep PRs reviewable (< 400 lines unless justified); rebase-merge style.

## 4. Code style & quality gates

- `cargo fmt` + `cargo clippy --workspace --all-targets -- -D warnings`
  (CI enforces).
- `#![deny(missing_docs)]` on engine crates; public API documented.
- No `unwrap`/`expect` outside tests; `unsafe` only at JNI/FFI with
  SAFETY comments (DRM_SECURITY.md §5).
- Errors typed with `thiserror` + i18n keys (TECHNICAL_DESIGN §8).
- New dependencies: justify in PR (why, size impact, audit status); CI
  runs `cargo audit`.
- Commits: conventional prefixes (`feat:`, `fix:`, `docs:`, `test:`,
  `chore:`, `refactor:`).

## 5. Testing obligations

- Every PR: unit/integration tests for changed logic (TESTING.md §1);
  goldens updated deliberately with before/after screenshots in PR.
- Fuzz targets extended for new parsers (nightly CI).
- Perf budgets: PR touching pagination/raster/index paths must run
  `scripts/bench_android.ps1` and report numbers (PERFORMANCE.md §9).
- a11y checklist from ACCESSIBILITY.md §6 for UI changes.

## 6. Documentation obligations

- TODO.md statuses updated (lifecycle rule 5).
- ADR: any architecture/dependency/format change = new ADR entry
  (ADR.md, append-only).
- GLOSSARY.md: new terms added in the same PR.
- CHANGELOG.md: user-visible change noted under Unreleased.

## 7. Review expectations

- Reviewers check: correctness, spec alignment, test coverage of the
  change, perf impact, a11y, docs, no secrets (`.env`/keystores never
  committed — see .gitignore).
- Author must address or rebut every comment; re-request review when done.

## 8. Getting help

- Issues labelled `question` for design Qs; `discussion` for RFC-style
  proposals. Doc-driven decisions go into ADR.md after consensus.
