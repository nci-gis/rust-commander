# Round 037: 0.4.0-beta.4: languages (EN, VI, JA preview)

**Status**: In Progress (the owner, 2026-11-24: "tạo nhánh feat/v0.4.0-beta.4 để làm i18n";
the mechanism chosen the same evening, below)
**Part of**: v0.4.0, `ROADMAP.md`, the row for **0.4.0-beta.4** (the owner, 2026-11-15,
restated 2026-11-23): the i18n mechanism and a pseudo-locale; English, Vietnamese, and
Japanese as a preview
**Branch**: `feat/v0.4.0-beta.4`, from `feat/v0.4.0-beta.3` at `66e6930` (beta.3's freeze)
**Date started**: 2026-11-24
**Date completed**: YYYY-MM-DD

## Goal

Every text xdevcmd shows a user comes from one catalogue per language, chosen at start
(config, else the system's locale), with English as the source and the fallback.
Vietnamese is complete; Japanese covers what a user sees first (menus, key bar, status
line, the common dialogs) and says it is a preview. Nothing a user does changes but the
words; widths already measure CJK and Vietnamese right (beta.3).

## Rules for this round (the owner, 2026-11-24)

- **Commits go to `feat/v0.4.0-beta.4` only, each with `[skip ci]`.** No CI and no
  review until beta.3 is released; then one CI run and the reviews on the whole branch.
- **Tests run locally** (`cargo test --workspace`, clippy, fmt) before each commit.
- beta.3's code is frozen except for a hotfix; a hotfix lands on `feat/v0.4.0-beta.3`
  and is merged into this branch, never the other way.
- Old state stays readable: config keys, session words and journal lines are not
  translated (they are data, not text for a user).

## Facts at the start (`66e6930`)

- No catalogue, no locale handling: every text is an English literal in the code.
- Literals of four characters or more before the test modules: about 2,200 in
  `xdevcmd`, 1,800 in `xdevcmd-core`. Many are not for a user (config keys, paths,
  environment names, log fields); the user-facing share is to be measured in W1.
- Text built in pieces, which a translation cannot reorder (beta.3 review, L1–L3 in
  `memory/2026-11-24-beta3-review-carryover.md`): `Mode::turned_off(what, next)`,
  `"{} mode"` in `app/open.rs`, `DIED` glued onto other text.

## Decided (the owner, 2026-11-24, 20:09–20:27)

**A, and small**: a bonus the user controls, not a translation project.
- A folder of TOML files, one per language, shipped beside the program (`lang/`); a user
  adds or changes one by hand in the config folder's `lang/`, which wins.
- The code asks by key with its English beside it; no file or no key: the English.
- `{params}` are filled in; a text naming one the call does not give falls back to the
  English for that key (answered: yes, hand-written code handles params and their
  mistakes).
- Only the UI; docs, code, comments, config keys stay English. An engine ready for more.
- A language's name and code are its file's name, unless the file says `"$__name"` /
  `"$__code"` (32 characters at most, cut); a common ISO 639-1 code has a display name.
- A pseudo-language is just a file of one's own.

## Options for the mechanism (as proposed; A chosen)

| | How | For | Against |
|---|---|---|---|
| A | Catalogues as TOML files in the repo (`i18n/en.toml`, `vi.toml`, `ja.toml`), embedded at build time; `t!("key")`, `t!("key", n = 3)` with `{name}` placeholders and `one`/`other` plural for English | No new dependency (TOML is already read); one simple way; a test can check every key and placeholder | Plurals and gender beyond `one`/`other` need code if a language ever asks (VI and JA have none) |
| B | Fluent (`fluent-bundle`) | Full plural and select rules, a known format for translators | A new dependency tree and binary size (the 7 MB budget), a second syntax to learn |
| C | gettext `.po`, the English text as the key | Translators' tools | The key changes whenever the English is edited; a crate or hand parser |

**Recommendation: A.** One simple way, no new dependency, and the tests below make a
missing or broken translation fail locally. B if a language with richer plurals is
planned for 0.5.

## Plan

- [ ] **W1 Measure.** `qa defs` gains "user text outside a catalogue": literals that reach
  the screen (draw, status, dialogs, errors shown) and do not go through `t!`. Baseline
  at `66e6930`.
- [ ] **W2 The mechanism.** `xdevcmd::i18n`: catalogues embedded, `t!`, placeholders,
  plurals, the fallback to English. Choice of language: `[general] language = "auto" |
  "en" | "vi" | "ja"`, `auto` from `LC_ALL`, `LC_MESSAGES`, `LANG` (Unix) and the user's
  locale on Windows and macOS. A pseudo-locale `xx` (accents, brackets, a third longer)
  for finding text left out and widths too small. Tests: every key of `en` in each
  language or marked as falling back; the same placeholders in each; no key unused; a
  screenshot in `xx` with no unbracketed text.
- [ ] **W3 Text built in pieces** (L1–L3): each message whole, with placeholders, before
  it is moved into the catalogue.
- [ ] **W4 Extract by area**, one commit per area, the user-facing count going down in
  each: key bar and menus; status line; dialogs (copy, move, delete, the plan table,
  Interrupted, Failed); errors and refusals; settings; help (F1) and `--help`.
- [ ] **W5 Vietnamese**, complete; the owner reads it.
- [ ] **W6 Japanese preview**: menus, key bar, status line, common dialogs; the rest falls
  back to English, and About says the translation is a preview.
- [ ] **W7 Docs**: `docs/config.md` (`language`), a page on adding a language.
- [ ] **W8 Names Windows does not allow** (the owner, 2026-11-24, carried from Round
  035's Act): a new folder (F7), a new file and a rename refused by one check, the one
  copy and move use (`plan::name_problem`): Windows' characters, a dot or space at the
  end, `CON`/`NUL`/`COM1`…, on Windows and on a FAT, exFAT or NTFS drive anywhere. Red
  test first (F7 with `a:b` and `CON` on Windows CI; the rules on a FAT mount on Linux).
  Remove the row from Known issues and the note in the Guide.
- [x] **W9 The fuzz crate in CI** (Round 035's Act): `cargo check` in `fuzz/`, in the
  cross-check job.

### To check at the cut

- [ ] `qa defs` "user text outside a catalogue": 0, or each one named and why.
- [ ] Screenshots in `xx`, `vi`, `ja` at 80×24 and 120×30: nothing cut or overlapping.
- [ ] Hand test in Vietnamese and Japanese on each system's terminal (the owner).
- [ ] CI and the red-team on the whole branch, after beta.3 is out.

## Risks

- **Scope**: ~4,000 literals to sort. W4 goes area by area; what is not extracted by the
  timebox stays English, and the pseudo-locale shows where.
- **Widths**: Vietnamese and Japanese are wider or narrower than English; dialogs sized
  to English text may cut. The `xx` locale and the screenshots catch it.
- **Data mistaken for text**: a config key, a session word or a log field translated
  would break old files. W1's measure leaves them out; the old-state tests of beta.3
  (pinned words) still pass.
- **No CI for weeks**: Windows and macOS breakage piles up. A cross-check build
  (`cargo check --target …`) is not possible here (the proxy blocks targets); so W2 keeps
  platform code untouched, and the first CI run after beta.3 is read before more work.

## Do

- **The engine** (2026-11-24): `crates/xdevcmd/src/lang.rs` (the app only; no new
  dependency: `toml` was there). `tr!("key", "English")`, `tr!("key", "English {n}", n =
  x)`, `Text::new("key", "English")` for tables. Keys whole, dotted or in sections.
  `[general] language = "auto"` (`LC_ALL`, `LC_MESSAGES`, `LANG`) or a code; `lang/`
  beside the program, then in the config folder; `$__code`, `$__name`, an ISO table of 24
  names. A problem reading a file is a note on the status line at start; Help → Config
  in use names the language in use and those found. Tests: lookup and fallback, params
  (missing, unknown, unclosed, braces), the choice from the environment, folder order,
  a broken file, `$__` keys and the cut, a screenshot in Vietnamese and one with no file.
- **`xtask lang`**: `template` (every key and its English, in sections) and `check FILE`
  (texts that would show in English, keys the code lacks); tests read the source's keys
  (one English each) and check the shipped files.
- **First texts with keys**: the key bar and the menu bar; `lang/vi.toml` and
  `lang/ja.toml` (preview) ship them; the release packs `lang/`.
- **Tests stay English** whatever the developer's locale: render, pty and the kill
  harness give xdevcmd a config folder of their own.
- `docs/config.md`: Languages; the docs-example test loads a `# lang/<file>` example as a
  language file.
- All local (the owner's rule for this branch): workspace tests, clippy, fmt.

## Check

## Act
