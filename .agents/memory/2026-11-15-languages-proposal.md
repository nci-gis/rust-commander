# Proposal: languages (EN, JA, VI) and a pseudo-language

**Date**: 2026-11-15
**Agent**: Claude Code
**Confidence**: Medium (a plan, not built; the string count is an estimate)
**Status**: New
**Source**: The owner, 2026-11-15: "supported language: EN | JA | VI, dạng file cùng level
với docs, theme, languages; nếu bị xóa thì fallback về EN; có thể thêm 1 funny (pseudo
lang)" — for a beta.3, "chỉ lên plan chứ chưa quyết định".
**Review-by**: 2026-12-15 (or at 0.4.0's release, whichever is first)

## Problem

Should xdevcmd speak Japanese and Vietnamese as well as English, and should that come
before 0.4.0 as a beta.3? The owner also asked whether it would add credibility to
v0.4.0.

## Finding

- **Scale**: about 1,300 user-visible phrases in `crates/xdevcmd/src` (64 files; most in
  `keys.rs`, `main.rs`, `commands.rs`, `app/shell.rs`) and about 230 in
  `crates/xdevcmd-core/src` (the planner's warnings, errors, undo; some are not UI
  text). Call it 1,200–1,400 strings, on nearly every screen.
- **Not a stability guarantee**: languages are a wide feature. A beta.3 changes the
  owner's release path (beta.1 → beta.2 → rc.1 fixes only → 0.4.0, decided
  2026-11-13) and "Stable before bigger"; it costs 2–3 weeks plus a week of use.
- **The exception is a pseudo-language**, which is a quality check: on every screen it
  shows text not yet in the catalog and text cut off by the layout.

## Options

| | Contents | Risk to 0.4.0 |
|---|---|---|
| **A** | 0.4.0 in English; languages lead 0.5 beside item 44 | none |
| **B** | beta.3 = foundation: one catalog, English built in, language files and fallback, the pseudo-language; VI and JA as files after 0.4.0 | medium: every screen, one mechanical change |
| **C** | beta.3 = all of it: EN, JA, VI | high: extraction, two translations, a Japanese review |

Leaning: A or B; not C before 0.4.0.

## Design sketch

- **Files**: `languages/en.toml`, `ja.toml`, `vi.toml`, beside `docs/` and `themes/` in
  the release archives; looked for in the config folder, then beside the binary, as
  themes are.
- **Fallback to EN**: English is compiled in. A missing file, a missing key or a broken
  file means EN for that string, said once on the status line; a deleted file never
  stops the start.
- **Setting**: `[general] language = "en"` by default. `auto` (`LANG`, Windows' UI
  language) later: both crates forbid `unsafe`, so Windows needs a small crate.
- **Format**: plain TOML, `key = "Copied {n} files"`; no Fluent (crates and size against
  the 7 MB budget). Plurals: `.one`/`.other` for EN; JA and VI use one form.
- **Translated**: menus, dialogs, the status line, F1, Settings, error wording.
  **Not translated**: key names, config keys, `[keys]` action names, CLI flags, journals,
  the debug log (the compatibility promise is untouched); docs stay English at first.
- **Safety**:
  - a language file is untrusted input, like a theme: control characters refused (no
    escape injection), placeholders must match English's set or EN is used, a size
    limit, a fuzz target;
  - a translation never changes what a button does: keys stay fixed in code (`[O]`),
    and a dangerous dialog's order and default never change, only the label.
- **Width**: Japanese is two columns a character; every pad, cut and dialog size must use
  display width (CJK columns already do, Round 019; dialogs and menus to check).
- **Pseudo-language**: `en-XA`, a test tool (about 40 % longer, accented, bracketed:
  `[!! Ćöpý fîłéš !!]`); a fun one (pirate English) only as an opt-in example file, like
  the example themes.
- **Tests**: render tests stay English; one screenshot pass in `en-XA` and one in `ja`;
  a check that keys and placeholders in ja and vi match en.
- **The hardest part, xdevcmd-core**: to translate its ~230 strings, core returns
  structured kinds and the app words them, which touches the planner just verified.
  Left in English, the confirm table would be half translated.
- **Quality, no over-promising**: VI by the team; JA needs a native reviewer, and a file
  drafted by AI says "draft" until reviewed.
- **Size**: foundation and pseudo-language L (1–2 weeks); each language M, plus review.
- **Compatibility**: the language format marked experimental, outside
  `docs/compatibility.md`, until it settles.

## Evidence

- String count: a scan of string literals of two or more words, outside test modules
  and comments, 2026-11-15 (an estimate, not a catalog).
- Release path and Direction: `plan/ROADMAP.md` (Decisions, Direction).

## Recommendation

**Do**: decide A, B or C before Round 033 closes; if B or C, the round's kickoff starts
from this note and measures the catalog for real first.
**Don't**: start translating before the catalog and the pseudo-language exist; translate
key names, config keys or action names.

## Promotion candidate?

- [ ] `context/`
- [ ] `skills/`
- [x] Not yet — a proposal; the owner decides, then it goes into the ROADMAP
