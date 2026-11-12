# Round 028: Syntax-highlighted viewer (v0.4.0, step 5, item 33)

**Status**: Review
**Part of**: v0.4.0, step 5 of `ROADMAP.md` ("Next"); item 33 (the owner's go,
2026-11-12: "yes, pls go as plan")
**Date started**: 2026-11-12
**Date completed**: YYYY-MM-DD

## Goal

F3 and quick view colour source code, config files, diffs and logs, from the theme's own
colours, without costing the viewer what makes it a viewer: a 2 GB log still opens at
once, a jump to line 1,000,000 never parses the file on the UI thread, and the binary
stays under the 7 MB budget with room for pillars 1–4. Off by setting, off past a size
cap, off under `NO_COLOR`; the shell's history (Shift+F12) is never coloured.

## Plan (kickoff review)

### 1. Where it starts

- ROADMAP step 5, item 33: "the binary measured on day one; over budget → a small
  language set, not dropped. Designed with pillar 2's progressive indexing (A03) in
  mind, or done in one round with it, so the viewer changes once." Release bar 3: **the
  binary at most 7 MB** (owner: important).
- A03 (`memory/2026-11-12-four-pillars-for-rc1.md`, the review's row A03): `Document`
  keeps a file handle and a line index and reads only the lines on screen; but the index
  is built **before `open` returns**, on the UI thread (F3 calls it synchronously); the
  remedy asked for is a progressive/background index with cancel, sparse checkpoints if
  needed. Not this round's job, but this round must not add a second whole-file pass on
  the UI thread, and its own background pass must be the shape the indexer can take over.
- **The code today.** `xdevcmd-core/document.rs`: `Document::open` sniffs 8 KiB (NUL →
  binary), indexes line starts in 256 KiB reads (cap 4,000,000 lines; `fully_indexed`
  false past it); `line(i)` reads one line, cut at 64 KiB, `\r`/`\n` stripped,
  `from_utf8_lossy`. `viewer.rs`: `render_text` draws `display_line(raw)` (tabs spread,
  controls as `·`) in pieces (wrap, or from `col`); `highlight()` colours the search term
  (fg `bg`, bg `mark`, bold) over plain `fg`; a mouse selection sets the row's bg; a
  history's paths are underlined on top. `render_hex` is separate. Quick view
  (`app/pictures.rs`) is the same `Viewer` drawn in a box, refused past 64 MiB. Five
  call sites build a `Viewer`: F3 (`open.rs`), a file from an archive (`archives.rs`,
  extracted to a temporary file), F3 on a path in a history (`shell.rs`, may jump to a
  line), the history itself (`shell.rs`, `Document::from_bytes`), quick view.
- **Colours**: `theme.rs`, sixteen slots, `Copy`, built-in themes and theme files;
  `quiet.rs` fits every cell to the terminal's depth (256, 16) at the backend. The
  16-colour theme uses ANSI names only. `xdevtheme_light_reads_on_its_own_background`
  checks names at 4.5:1 on the panel and cursor row, at true colour and 256. xdevcmd
  does not read `NO_COLOR` itself today (crossterm drops colour escapes under it).
- **Budgets**: `tests/budgets.rs` checks the release binary at most 7 MB when a release
  build exists. **Today: 5,359,944 bytes (5.11 MiB)**, this machine, Linux x86_64,
  `lto = "thin"`, `codegen-units = 1`, `strip`.

### 2. Day one: measured before deciding

All on this machine (Linux x86_64, 2 cores), release profile as shipped, a probe that
loads the syntaxes and highlights a file so nothing is dropped as dead code.

| Candidate | Release binary | vs budget (7 MiB = 7,340,032 B) |
|---|---|---|
| Today, no highlighting | 5,359,944 B (5.11 MiB) | 1.89 MiB left |
| (a) syntect 5.3, `default-syntaxes` + `regex-fancy`, all 75 default syntaxes | 7,597,416 B (7.25 MiB) | **over by 0.25 MiB** |
| (a) syntect 5.3, `default-fancy` (as named in the brief) | 7,598,552 B (7.25 MiB) | over (themes unused, so dropped) |
| (a) syntect, 22 syntaxes (Rust, Python, JS, JSON, YAML, Markdown, bash, C, C++, Go, Java, HTML, CSS, diff, XML, Makefile, SQL and their helpers) | ≈ 7,365,000 B (7.02 MiB), computed: grammars 345 KB → 113 KB | **over by ≈ 25 KB** |
| (a) syntect, Rust and Python only | ≈ 7,269,000 B (6.93 MiB) | 71 KB left: no room for pillars 1–4 |
| (b) tree-sitter 0.25 + highlight + Rust + Python grammars (standalone binary 3,668,896 B vs an empty one 339,136 B) | ≈ +3.33 MB → ≈ 8.5 MiB | **over by ≈ 1.5 MiB with two languages** |
| (c) a small lexer of our own | measured in Do (expected under 0.1 MB) | — |

- **Where syntect's 2.24 MB goes** (`cargo bloat --crates`): `regex_automata` 362 KiB,
  `regex_syntax` 178 KiB (and its Unicode tables in `.rodata`), `aho_corasick` 140 KiB,
  `fancy_regex` 73 KiB, `syntect` 62 KiB of code; the grammars themselves are only
  0.34 MB of it. Trimming the language set cannot fix it: the regex engine is the cost,
  and syntect turns on fancy-regex's `perf` features, which we cannot turn off.
- **Missing from syntect's default set**: TOML, TypeScript, INI, Dockerfile. Adding them
  means vendoring third-party `.sublime-syntax` files (each with its own licence) and
  `yaml-load` in a build script.
- **cargo-deny on syntect's tree**: licences ok (MIT/Apache); **advisories FAILED**:
  RUSTSEC-2025-0141, *bincode is unmaintained* (syntect's dump format), which would need
  an `ignore` entry with a reason; bans: a second `thiserror` (1 and 2), a warning.
- **Compile time**: release, only the changed crates rebuilt: 68 s today → 74 s with
  syntect (+6 s on 2 cores).
- **(b) tree-sitter, rejected**: over budget with two grammars (a grammar's compiled
  `parser.c` is 0.5–2 MB; C++ and TypeScript are the large ones); C code built by `cc`
  on every target we ship (MSVC, musl on ARM, macOS cross) where today the build is
  pure Rust apart from Windows' resource file; the bindings are `unsafe` FFI (allowed
  in dependencies, but C parsers on untrusted input are exactly what hard rule 4 and
  the fuzzing bar are wary of).
- **(c) our own lexer, chosen**: one pass per line, a state that fits in a few bytes
  (in a block comment, in a multi-line string and its delimiter, in a fenced block),
  keyword tables per language. Cost: the code is ours to maintain (about 1,000 lines
  with tests), it is not a parser (no nested template expressions, no heredoc bodies,
  no context-sensitive keywords), and each new language is a table and a test. The
  gain: tens of kilobytes; no new dependency, no licence, no advisory; checkpoints are
  nearly free because the state is tiny; it runs at hundreds of MB/s, so a background
  pass over a big file is short.

**Recommendation: (c), with this language set** (the owner's list, all of it, since
size no longer forces a cut): Rust · Python · JavaScript and TypeScript (one table,
`.js .mjs .cjs .jsx .ts .tsx`) · JSON · TOML · YAML · Markdown · shell (sh, bash, zsh)
· C and C++ · Go · Java (and Kotlin, C#: the same table shape, cheap) · HTML and XML ·
CSS · diff/patch · INI (and `.conf`, `.cfg`, `.properties`, `.gitconfig`, `.editorconfig`)
· SQL · logs (`.log`: levels, timestamps). Anything else: plain, as today.

### 3. Critique: what must be got right

- **Per visible region, never the whole file.** A line's colours need the state at its
  start (inside `/* … */`? inside `"""`?). Design:
  - **Checkpoints**: the state at the start of every 128th line, in a store shared with
    a background thread that scans the file once, in 1 MiB reads, lexing for state only.
    Line 0's state is known at once, so the first screen never waits.
  - **Drawing line i**: from the nearest known start (a checkpoint, or a line the viewer
    lexed recently, kept in a small bounded cache) lex forward to i. At most 127 lines
    when the checkpoint is there; scrolling by a line costs one line.
  - **A jump past what the scan has reached** (G on a 50 MB file just opened, `:1000000`,
    F3 from a history at line 900,000): if a known start is within 2,048 lines, lex from
    it on the UI thread (bounded, milliseconds); else draw the lines **plain** until the
    scan gets there, and redraw when it does (the loop is told, as for a directory
    loading). Never a whole-file parse on the UI thread.
  - **Same colours either way.** The scan must cut lines exactly as `Document::line`
    does (64 KiB cap, trailing `\r`/`\n` stripped, lossy UTF-8, the 4,000,000-line
    limit): one shared function in core, and a test that lines reached by scrolling
    and by a jump get the same colours, on a file with long lines, CRLF, invalid UTF-8
    and comments that span checkpoints.
  - **A03's hook**: the scan is a `Scan` fed byte chunks (`feed`, `finish`), filling a
    `Checkpoints` store. The background indexer of pillar 2 reads the same chunks to
    find line starts; when it exists it feeds them to `Scan` too and the second read
    disappears. Nothing in the viewer changes then.
  - The scan opens the file again by path (its own handle: `Document` seeks its own,
    and a shared cursor would race). Cancelled when the viewer closes. Documents held in
    memory (the history) are never scanned: they are not coloured.
- **Caps.** F3: files up to 64 MiB are coloured (the size quick view already refuses
  past); bigger ones plain, said once on the message line ("over 64 MiB — not
  coloured"). Quick view: up to 1 MiB, and no background scan (it shows the top of the
  file; lines past 2,048 stay plain). **Lines over 4 KiB are drawn plain** (minified
  JavaScript), still lexed for the state after them so the next line is right.
- **Binary files and hex view**: never coloured; `h` back to text colours again.
- **Priority of colours**: syntax is the base; the search term's highlight replaces it
  where it matches; a mouse selection sets the background only; a history's paths (not
  coloured anyway) keep theirs.
- **Detection**: the file name first (`Makefile`, `Dockerfile` → shell-like,
  `Cargo.lock` → TOML, `.bashrc`, `.gitconfig`), then the extension (case-insensitive),
  then the first line: a shebang (`#!/usr/bin/env python3`, `#!/bin/sh`, `node`), `<?xml`,
  `<!DOCTYPE html`, `diff --git`. Unknown → plain.
- **Colours from the theme, no new theme keys.** Each kind maps to an existing slot:
  comment → `text_dim`, string → `executable`, number and constant → `archive`, keyword
  and tag → `symlink`, type, key and attribute → `directory`, function and heading →
  `accent`, attribute/decorator/preprocessor/section → `marked`, diff added →
  `executable`, removed and log errors → `error`, log warnings → `marked`. Every
  built-in theme, every theme file and the user's own get syntax colours at once; the
  16-colour theme is ANSI names already; `quiet.rs` fits them to 256/16. A `[syntax]`
  table can come later if anyone asks (0.5). **xdevtheme-light**: its contrast test is
  extended to these colours on the viewer's own background (`bg`, not the panel) at
  4.5:1, true colour and 256.
- **Setting**: `[viewer] highlight = true` (default **true**: it is the point of the
  item, and a newcomer sees it), a row in Settings → Files beside the viewer's others.
  **`NO_COLOR`** set and not empty → off (and nothing is lexed).
- **Never coloured**: the shell's history (Shift+F12), which is the shell's output, not
  a file; a document in memory.
- **No panic**: invalid UTF-8 (already lossy), empty files, a 64 KiB line, a file that
  shrinks while scanned (the scan stops at its end; states stay consistent with what the
  viewer reads, or the lines stay plain), a vanished file (the scan stops; plain).

### 4. Steps

1. This kickoff; ROADMAP step 5 and item 33 → Round 028. **[skip ci]**
2. Core `syntax.rs`, detection: `Lang`, `detect(name, first_line)`. Unit tests.
3. Core lexer and checkpoints: `lex_line`, `State`, `Checkpoints`, `Scan`,
   `Highlighter` (background scan, bounded cache, `spans(doc, i)` → `None` when not
   ready). Unit tests: same colours by scrolling and by jump; a checkpoint test (a block
   comment across a checkpoint); invalid UTF-8, huge lines, empty file; cancel.
4. Viewer integration: kinds → theme styles, per-character styles under search and
   selection, the redraw when the scan arrives, the caps and their message. Render
   tests: a Rust and a Python file coloured at true colour; plain with `highlight =
   false` and under `NO_COLOR`; the search highlight on top; the history not coloured.
5. Quick view: the same code, 1 MiB, no scan. A render test.
6. Setting, Settings row, `docs/config.md`.
7. Theme: the xdevtheme-light contrast test over the syntax colours on `bg`.
8. Budgets: release build, the size test, the size recorded; time to first paint of a
   50 MB file with highlighting on and off.
9. Docs: guide (viewer), config.md, CHANGELOG. No new licence, so `THIRD-PARTY.md` and
   `about.toml` unchanged.
10. Do/Check: fmt, clippy `-D warnings`, `cargo test --workspace` twice, `cargo deny
    check`; push; CI.

### 5. Decisions (recommendations; the coder follows them unless the owner objects)

1. **Our own lexer (c)**, not syntect (over budget even trimmed; an unmaintained-crate
   advisory) nor tree-sitter (over budget with two languages; C on every target).
2. **The full requested language set**, listed above, since (c) leaves the budget alone.
3. **Checkpoints every 128 lines from a background scan, fed in chunks** (`Scan`), so
   A03's indexer can take it over; jumps beyond it lex at most 2,048 lines on the UI
   thread, else plain until the scan arrives.
4. **Caps**: 64 MiB for F3, 1 MiB for quick view; lines over 4 KiB plain.
5. **Theme slots, no `[syntax]` table**; xdevtheme-light checked at 4.5:1 on `bg`.
6. **`[viewer] highlight = true` by default**; `NO_COLOR` turns it off.
7. **Not coloured**: the history, documents in memory, hex, binary files.

### Not doing (this round)

- The progressive line index itself (A03, pillar 2); this round only shapes the hook.
- Embedded languages (JavaScript inside HTML, code inside Markdown fences: the fence is
  one colour), heredoc bodies, nested template expressions.
- A `[syntax]` colour table, user-defined languages, a language picker in the viewer.

## Do

- **2026-11-12, kickoff** (`19e9821`): this file; ROADMAP step 5 and item 33 → Round 028.
- **Detection and lexers** (`11e522b`): core `syntax/mod.rs` (`Lang`, `Kind`, `Span`,
  `State`, `detect`: whole names, then extensions, then a shebang or `<?xml`,
  `<!DOCTYPE html`, `diff --git`) and `syntax/lex.rs`: one table-driven lexer (`Spec`:
  comments, nesting, quotes and which go on past a line, raw quotes, triple quotes,
  Rust's raw strings/chars/lifetimes/`name!`, Python's prefixes, decorators, the
  preprocessor, `#[…]`, shell's `$…`, `Capitalised` types and `ALL_CAPS` constants,
  `fn`/`def`/`class` naming the next word) for Rust, Python, JS/TS, JSON, shell, C/C++,
  Go, Java, Kotlin, C#, CSS, SQL; lexers of their own for Markdown (fences carry over),
  diff, logs (a leading timestamp, levels in capitals or before `:`), INI, TOML (keys,
  tables, `'''`/`"""` over lines), YAML (keys, list markers, anchors) and HTML/XML
  (tags, attributes, comments and tags over lines, entities). Spans start and end only
  at ASCII bytes or the line's end, so on character boundaries (asserted by every test).
  Tests: each language's pieces, states carried over lines, and every lexer from every
  state over odd input (lone quotes, `r#"`, `${`, `<`, U+FFFD, emoji, 75 KB lines).
- **Checkpoints** (`bd0909a`): `syntax/scan.rs`. `Checkpoints` (the state at every
  128th line), `Scan` (fed byte chunks, cuts lines with `document::line_text`, the rule
  `Document::line` now calls too, and the 64 KiB cap), `Highlighter` (`scan_file` on a
  thread of its own with its own handle, cancelled on drop, a panic marks it done;
  `spans(doc, i)` from the nearer of the checkpoint and a recently lexed line, `None`
  past 2,048 lines; a bounded cache; lines over 4 KiB lexed for the state, drawn
  plain). Tests: 5,000 lines with comments, strings and raw strings over checkpoints,
  CRLF, invalid UTF-8, a 70 KiB line with `/*` past the cut and a long line opening a
  comment: every 7th line and the edges, reached by jumping with the scan, equal the
  same lines scrolled to one by one; a C comment across line 128 fed in 5-byte chunks;
  a far jump is `None` without the scan and `Some` within 2,048 lines or after it;
  empty and one-line files; dropping stops an endless scan; a missing file.
- **The viewer and quick view** (`71f1c45`): `Viewer::with_syntax(on, max, scan)`
  (never for binary or in-memory documents; over the cap the message says "<lang> file
  over 64 MiB — not coloured"); a style per character (a tab's spaces take its style),
  then the search hit over it, char-wise (the old byte slicing of a lower-cased copy
  could cut a character whose lower case is longer); the language in the title; lines
  the scan has not reached drawn plain and the frame drawn again when it moves on
  (`App::background`); `--screenshot` waits for it as for a job. `theme::syntax_style`
  maps kinds to slots. F3, a file from an archive and F3 on a path in a history get the
  scan; quick view gets 1 MiB and none; the history none. `[viewer] highlight` in the
  config (default true), `colours_on` adds `NO_COLOR`. Render tests: Rust and Python
  at true colour (keyword, string, comment and a comment over two lines, call,
  decorator, triple string over two lines); off by setting and under `NO_COLOR`; hex
  and back; the search hit's background with the string's colour kept beside it; `G`
  on 30,000 lines under one open comment, coloured from the scan; quick view. Unit:
  a document in memory is never coloured; the cap, its message, the setting.
- **Settings and config.md** (`43df979`): "Viewer colours source" in Settings → Files,
  on by default (render test).
- **Contrast** (`72da39c`): `xdevtheme_light_reads_on_its_own_background` now checks
  every kind's colour on the viewer's `bg` at 4.5:1, true colour and 256 (passes with
  the existing slots; shown to fail at 2.86:1 with comments mapped to `border`).
  `syntax_style_in(theme, kind)` so the test does not touch the global theme.
- **Docs** (`9767e68`): guide (viewer), CHANGELOG. `config.md` in `43df979`. No new
  dependency: `Cargo.lock`, `THIRD-PARTY.md` and `about.toml` unchanged.
- **Measurements** (this machine, Linux x86_64, 2 cores, release):
  - Binary **5,451,752 B (5.20 MiB)**, +91,808 B over the 5,359,944 B before; 1.80 MiB
    under the 7 MiB budget (`tests/budgets.rs` passes on the release build).
  - Time to first paint, a 50 MB Rust file (653,680 lines), `--screenshot=120x40`,
    shell wait 0, median of 5 (each includes start-up, about 36 ms, and the
    screenshot's 30 ms pause per key): `Down,F3` off **107 ms**, on **108 ms**;
    `Down,F3,G` off 147 ms, on 610 ms: there the screenshot waits for the background
    scan to reach the end (about 0.46 s for 50 MB, ~110 MB/s); a person sees the end at
    once, plain, coloured when the scan arrives.

- **Independent review, 2026-11-12** (in-house lexer approved; its fuzzing: 17.1M
  lines, no panic). Fixes, one commit each, each test shown to fail without its fix:
  - `0de21e0` On-the-spot lexing was bounded by lines (2,048) only: a jump to the end of
    64 MiB in 32 KiB lines lexed it all on the UI thread (the review: 704 ms; here, the
    `measure_` test in release: **609 ms → 28 µs**, plain). Now also `SYNC_BYTES` = 1 MiB
    (from `Document::line_start`), and the scan keeps a checkpoint at least every
    512 KiB as well as every 128 lines, so after the scan a line is never more than
    that from one. Test: 200 lines of 32 KiB, a jump to the line after them is `None`
    in under 100 ms, then coloured after the scan.
  - `d15fbb7` Lines over 4 KiB were lexed again on every frame, and the viewer's line
    was read twice. Now a long line whose next state is cached returns at once, and
    `spans_for` takes the text the viewer has read. A frame of 50 lines of 32 KiB
    (release): **12 ms → 0.7 ms** on the second frame (the review saw 50–89 ms; the
    0.7 ms left is the measurement's own reads). Test counts lexes: 50, then none.
  - `cee8521` (nit) A file whose length or modification time changed during its scan
    has its checkpoints dropped at the end (and the viewer's cache of states with
    them), so it is not coloured from the old text; lines far from the top go plain.
  - `39e6eef` (nit) The selection's background went over the search hit (fg = bg): it
    now skips the hit's cells. The hit is also found on the whole line before it is
    cut into wrapped rows, so one across a wrap is lit on both rows. Render test.
  - `f5842bb` (nit) xdevtheme-light: every syntax colour at 4.5:1 on
    `row_cursor_active` too (the viewer's selection), true colour and 256; passes.
  - **Correction** to the kickoff's "hundreds of MB/s": measured about 85 MB/s by the
    review and about 110 MB/s here (the 50 MB scan above); fast enough for a background
    pass, not hundreds.

## Check

- [x] Detection, lexer, checkpoints: unit tests (scroll = jump, checkpoint crossing,
  invalid UTF-8, huge lines, empty file, cancel)
- [x] Render: Rust and Python coloured; off by setting and under `NO_COLOR`; search on
  top; quick view; a jump coloured from the scan. History plain: a unit test (it is a
  document in memory), not a render test (no shell in the render suite)
- [x] xdevtheme-light contrast for the syntax colours on the viewer, true colour and 256
- [x] Release binary 5.20 MiB, under 7 MiB; first paint of 50 MB on vs off (above)
- [x] fmt --check, clippy `-D warnings` (Linux), `cargo deny check` (advisories, bans,
  licences, sources ok), `cargo test --workspace` twice
- [x] CI on three systems: run 43253994728 green first time (Linux x86_64 and ARM,
  Windows, macOS, cargo-deny, the windows-gnu/darwin cross-check); no flake seen
- [x] Review fixes: byte bound (609 ms → 28 µs), long lines lexed once (12 ms →
  0.7 ms a frame), a changed file's checkpoints dropped, search over selection, the
  selection contrast; fmt, clippy, `cargo test --workspace`; release binary now
  5,454,952 B (5.20 MiB)
- [ ] **Not verified**: colours on a real terminal by eye (only screenshots); the
  256- and 16-colour look of the syntax colours beyond the light theme's contrast test
  (the 16-colour theme is ANSI names, so the user's palette decides); Windows and
  macOS by hand; the plain-then-coloured redraw after a jump seen live (the screenshot
  waits for the scan instead); a file changed while it is scanned is tested only with a forced
  "changed" (and the stamp itself), not a real write racing the scan; quality of the
  colours in real projects beyond the samples (it is a lexer: no heredoc bodies, no
  code inside Markdown fences or HTML `<script>`, regex literals in JavaScript not
  recognised).

## Act

**Learnings**:

- ...

**Promotions**:

- [ ] → reference/features.md : the viewer's colours, caps and checkpoints
