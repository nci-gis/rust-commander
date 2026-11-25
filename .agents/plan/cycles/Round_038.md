# Round 038: docs that say what the code does (a review on demand)

**Status**: In Progress (the owner, 2026-11-25: D6 yes, in beta.4, skill approved; draft by the agent the same day; the owner agreed the shape the same day:
"chỉ cần skill để user chủ động gọi AI Agent review khi cần … ko cần phải maintain mỗi
lần release; docs ledger có thể chỉ ghi lại lần cuối review và danh sách docs đã review +
hash")
**Part of**: after Round 037 (beta.4); not tied to a release
**Branch**: `feat/v0.4.0-beta.4` (the owner, 2026-11-25: "làm trong beta.4")
**Date started**: YYYY-MM-DD
**Date completed**: YYYY-MM-DD

## Goal

Anyone can ask an agent "are the docs still true?" and get a reliable answer: which
pages changed, or had their code change, since they were last reviewed, and for each
page reviewed, every claim checked against the code or a test, with the evidence. No
step is added to a release; the review is run when wanted.

## Scope

In: `docs/*.md`, `README.md`, `SECURITY.md`, `crates/xdevcmd-core/README.md`, and
`CHANGELOG.md`'s `[Unreleased]`. Out: `.agents/` (historical, the owner), released
CHANGELOG sections (history), generated files already tested (`docs/keys.md`,
`lang/template.txt`).

## Facts at the start

- Tested today: `docs/keys.md` = `--print-keys`; every TOML example in README, config,
  security and language is a valid config; `lang/template.txt` = the code's keys; the
  docs link names this version's tag.
- Not checked: 76 links with an anchor, 115 `--flags`, 10 `XDEVCMD_*` variables, config
  keys and defaults in prose, and every sentence about behaviour.
- Seen in two days of beta.4 work: the guide said F7 refused Windows' characters (it did
  not); the plan said CI had a FAT mount (it had not). Both were prose nobody re-read
  after the code moved.

## Idea taken from OpenWiki (LangChain), not the tool

OpenWiki keeps a wiki written for agents and ties "material facts to versioned source
evidence", refreshing a page when that evidence changes. The idea fits; the tool does
not: it writes its own wiki (ours is written by people, for users) and needs npm and a
model provider in CI. So: claims with evidence, checked by an agent when asked.

## Plan

- [x] **D1 The ledger**: `.agents/ledgers/docs-review.tsv` (the owner, 2026-11-25: a
  result of the agents' skill; not under `docs/`, which ships in every archive). One row per page reviewed: `path`, `hash` (`git hash-object`) of the page as
  reviewed, `commit` the review read the code at, `date`, `by`, `result` (`ok` |
  `fixed` | `open`), `note`. Only the last review of each page is kept.
- [x] **D2 Pages to code** (`DOC_PATHS`, in `xtask`): which source each page speaks of
  (`shell.md` → `pane/term*`, `app/shell.rs`, `safety.rs`…). Coarse is fine: it only
  orders the review. A test: every path named exists.
- [x] **D3 `xtask docs status`**: for each page in scope, one line: `never reviewed`,
  `page changed` (its hash differs), `code changed` (a `DOC_PATHS` file changed since the
  row's commit, with how many commits), or `ok`. Read-only; no CI gate.
- [x] **D4 The skill `docs-review`** (drafted in
  `memory/2026-11-25-docs-review-skill.md`; promoted to `skills/` on the owner's word):
  run `xtask docs status`, take the pages not `ok` (or those named), and for each, one
  independent sub-agent that lists every claim the page makes and checks it against the
  code or a test, with evidence (`file::symbol` or a test's name). Then a report, the
  fixes proposed, and the ledger rows to write once the owner agrees.
- [ ] **D5 A first review of everything**, page by page, to set the ledger's baseline.
- [x] **D6 `xtask docs check`** (the owner: yes), the mechanical half: links and anchors,
  `--flags` against `--help`, `XDEVCMD_*` against the code, config keys and defaults
  against `--print-config`. Cheap and exact; could run in CI or only in the skill.

## Risks

- **A review that passes everything**: a claim counts as checked only with its evidence;
  "looks right" is not evidence. The skill says so, and the report lists claims with no
  evidence apart.
- **`DOC_PATHS` too narrow**: a page about the copy dialog not marked when `modal.rs`
  changes. D3 only orders the work; a review named by the owner reads the page anyway.
- **Ledger drift**: a page edited after review without a new row shows `page changed`,
  which is the point.

## Do

- 2026-11-25: `xtask/src/docs.rs`: `DOC_PATHS` (15 pages), `docs status`, `docs check`.
  Tests: every page and path is there and every `docs/*.md` is in scope; GitHub's
  slugs; what the check reads from a page; every link and anchor resolves; the ledger's
  rows are pages in scope. First `docs check`: 11 reports, 10 of them the check's own
  mistakes (`cargo install xdevcmd --locked` read as xdevcmd's flag, xtask's flags
  spread over its modules, a made-up `docs/b.md` in a test), now fixed; 1 real: shell.md
  named `[terminal] log`, a setting that does not exist yet (reworded). All 76 anchors
  resolve. `docs status`: 15 of 15 never reviewed (D5 next).
- 2026-11-25: D5, the first review (`/docs-review`, 7 read-only subagents, code at
  `26d055b`; report in `memory/2026-11-25-docs-review-first-pass.md`). About 470
  claims: 41 false, ~20 unverified. The owner agreed to all (2026-11-25). Four were the
  code's fault and were fixed with tests (`83b0740`): an `[[open]]` rule's program
  looked up in PATH's absolute directories only; a theme's `[style]` starting from its
  base; `lang check` leaving braces alone where the English has none; `try --small` at
  its 0.6 GiB. The 41 doc fixes, in language.md's style, and `DOC_PATHS` widened to the
  code each page describes (`f912ce6`). Ledger: 15 rows, 12 `ok`/`fixed`, 3 `open`
  (unverified claims in config, known-issues, troubleshooting).

## Check

- `docs status` shows `ok` for a row whose result is `open`: the result column is not
  read. Small follow-up if the owner wants it (say "ok, 1 open item").

## Act
