# Round 038: docs that say what the code does (a review on demand)

**Status**: Plan (draft by the agent, 2026-11-25; the owner agreed the shape the same day:
"chỉ cần skill để user chủ động gọi AI Agent review khi cần … ko cần phải maintain mỗi
lần release; docs ledger có thể chỉ ghi lại lần cuối review và danh sách docs đã review +
hash")
**Part of**: after Round 037 (beta.4); not tied to a release
**Branch**: to decide (a working branch off `dev`, or inside beta.4)
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

- [ ] **D1 The ledger**: `docs-review.tsv` at the repo root (not under `docs/`, which
  ships in every archive). One row per page reviewed: `path`, `sha256` of the page as
  reviewed, `commit` the review read the code at, `date`, `by`, `result` (`ok` |
  `fixed` | `open`), `note`. Only the last review of each page is kept.
- [ ] **D2 Pages to code** (`DOC_PATHS`, in `xtask`): which source each page speaks of
  (`shell.md` → `pane/term*`, `app/shell.rs`, `safety.rs`…). Coarse is fine: it only
  orders the review. A test: every path named exists.
- [ ] **D3 `xtask docs status`**: for each page in scope, one line: `never reviewed`,
  `page changed` (its hash differs), `code changed` (a `DOC_PATHS` file changed since the
  row's commit, with how many commits), or `ok`. Read-only; no CI gate.
- [ ] **D4 The skill `docs-review`** (drafted in
  `memory/2026-11-25-docs-review-skill.md`; promoted to `skills/` on the owner's word):
  run `xtask docs status`, take the pages not `ok` (or those named), and for each, one
  independent sub-agent that lists every claim the page makes and checks it against the
  code or a test, with evidence (`file::symbol` or a test's name). Then a report, the
  fixes proposed, and the ledger rows to write once the owner agrees.
- [ ] **D5 A first review of everything**, page by page, to set the ledger's baseline.
- [ ] **D6 (to decide) `xtask docs check`**, the mechanical half: links and anchors,
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

## Check

## Act
