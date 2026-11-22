# Past misses

> **Read this when**: triaging a finding, writing the QA report, or deciding whether
> a class of defect has recurred. The lessons behind the classes are in
> [reference/review-lessons.md](../../../reference/review-lessons.md).

One row per defect that got past a layer that should have stopped it. A row is
added when the defect is found and updated when it is fixed. A row is never deleted;
a fixed row stays, with its control.

## Columns

| Column | Meaning |
|---|---|
| `class` | `scope` · `interaction` · `multi-definition` · `deny-default` · `persisted-intent` · `docs-claim` · `evidence-stale` · `oracle-missing` · `tool-hazard` · `fragile-wait` · `validate-raw` · `os-format` · `process` |
| `detected_by` | `test` · `CI` · `QA` · `red-team` · `xmodel` (another model) · `hand-test` · `daily-use` · `external-review` · `research` |
| `should_have` | the earliest layer that should have caught it |
| `rung` | the control now holding it (1–5; see the lessons file) |
| `seeded` | `yes` if it was planted on purpose to measure a layer (plan v3 P8) |

**Escape rate** (reported by the QA skill) is the share of P0 and P1 rows whose
`detected_by` is later than `should_have`.

## Rows

| ID | Date | Defect | Sev | Class | detected_by | should_have | Rung | Control / fix | Status | Recur | Seeded |
|---|---|---|---|---|---|---|---|---|---|---|---|
| RR01 | 2026-11-17 | C finished a damaged kept part without reading it again | P1 | interaction | external-review | red-team | 2 | test `a_checked_resume_reads_a_kept_part_again_with_its_source` (#43/#44) | fixed | 0 | no |
| RR02 | 2026-11-17 | `may_tidy` checked only some journal kinds against `--root` | P1 | multi-definition | external-review | red-team | 2 | test; one kind table planned in H-07 | fixed | 0 | no |
| RR03 | 2026-11-17 | An existing link out of the root passed through its parent | P1 | deny-default | external-review | red-team | 2 | test (#44) | fixed | 0 | no |
| RR04 | 2026-11-17 | Copy As lost its new name across a kill | P2 | persisted-intent | external-review | QA | 2 | app test (#44); `Request` destructure test planned | fixed | 0 | no |
| RR05 | 2026-11-17 | A resume dropped the plan's decisions on a second kill | P2 | persisted-intent | external-review | QA | 2 | test (#44) | fixed | 0 | no |
| RR06 | 2026-11-17 | F reported removals that had failed | P2 | interaction | external-review | red-team | 2 | test (#44) | fixed | 0 | no |
| RR07 | 2026-11-17 | `release-check.sh` read the working tree, not the commit | P2 | scope | external-review | red-team | 2 | refuses a tree that differs from the commit (#44) | fixed | 0 | no |
| R01–R15 | 2026-11-17 | 15 docs claims wrong or misplaced (F's effects, kill scope, read-only promise, Windows paths, Undo, terminal close vs kill, Skip scope, shell) | P2 | docs-claim | external-review | QA | 4 | fixed in docs (#44); `docs_claims` planned | fixed | 0 | no |
| W01 | 2026-11-18 | `wait_for("copied")` matched the Interrupted dialog, and F10 then went to the job | P3 | fragile-wait | CI | test | 3 | waits on the status line; `tests/waits.rs` (#58) | fixed | 0 | no |
| W02 | 2026-11-20 | `wait_for("deeper ")` matched a list row before Enter was taken | P3 | fragile-wait | CI | test | 3 | waits for the pane title (#49); `tests/waits.rs` (#58) | fixed | **1** | no |
| P01 | 2026-11-18 | #43 closed itself when its base PR (#42) merged | P3 | process | daily-use | (process) | 4 | no stacked PRs (plan v3) | held | 0 | no |
| C01 | 2026-11-19 | Copy into itself through a link to a folder not made yet: stack overflow (copy, move, plan) | P1 | multi-definition | research | QA | 5 | H-04 (#52) | fixed | 0 | no |
| C02 | 2026-11-19 | Pack's destination name is not checked for control characters | P1 | multi-definition | research | red-team | 5 | H-08 (#54) | fixed | 0 | no |
| C03 | 2026-11-19 | Four root checks with different rules; restore from the trash under `--root` likely always refused | P2 | multi-definition | research | red-team | 5 | H-03 (#55): `Policy::allows_at`, E2 reproduced and fixed | fixed | 0 | no |
| C04 | 2026-11-19 | `refused()` lets an unlisted action through (`OpenWithMenu` in read-only) | P2 | deny-default | research | red-team | 5 | H-05 (#56): `Action::effect` exhaustive, both wildcard lints denied | fixed | 0 | no |
| C05 | 2026-11-19 | `same_file` and `same_entry` differ on links: rename-many plans what apply refuses | P2 | multi-definition | research | test | 5 | H-04 (#52) | fixed | 0 | no |
| C06 | 2026-11-19 | A torn last `done` journal line marks a different folder done | P1 | persisted-intent | research | test | 5 | H-07 (#53): `whole_lines` | fixed | 0 | no |
| C07 | 2026-11-19 | An interrupted sync (Alt+Y) is offered, then dropped with its kept parts; the docs say "Covered" | P2 | docs-claim | research | QA | 5 | H-07 (#53): misread (a sync's journal is `kind sync`, not offered); the docs claim was wrong, fixed | fixed | 0 | no |
| C08 | 2026-11-19 | `known()` drops 5 newer commands when the list is reloaded | P3 | multi-definition | research | test | 5 | H-10 (#61): `Extra::id`, exhaustive | fixed | 0 | no |
| C09 | 2026-11-19 | Journal kinds `part` and `gone` are read but never written | P3 | multi-definition | research | red-team | 5 | H-07 (#53): both readers removed (no version wrote them) | fixed | 0 | no |
| C10 | 2026-11-19 | `--kill-when` killed every xdevcmd by name | P2 | tool-hazard | research | red-team | 1 | `--launch` and a PID file (#47) | fixed | 0 | no |
| C11 | 2026-11-19 | A worker panic leaves the job "working"; two early returns skip restoring the terminal | P2 | scope | research | test | 5 | H-10 (#61): the job ends as failed; terminal guards | fixed | 0 | no |
| C12 | 2026-11-19 | The result of setting the journal folder to 0700 is ignored | P3 | deny-default | research | red-team | 5 | H-10 | open | 0 | no |
| C13 | 2026-11-19 | `XDEVCMD_STATE`: a file to the code, a folder to the man page; a folder put the journal beside it | P2 | docs-claim | research | QA | 2 | refused at start, docs agree (#48) | fixed | 0 | no |
| C14 | 2026-11-19 | Manual-case Expected lines quote text that is not in `docs/` | P3 | oracle-missing | research | QA | 2 | `tests/manual_cases.rs`: fields and quotes checked against `docs/` (P4) | fixed | 0 | no |
| X01 | 2026-11-21 | A typed name was trimmed before its control-character check: a tab or newline at either end went into another name (Pack, Save as, Log to, F7, Shift+F4, Copy As) | P3 | validate-raw | xmodel | red-team | 2 | checked on the raw text; test `a_control_character_at_either_end_of_a_name_is_refused_not_trimmed` (#54) | fixed | 0 | no |
| X02 | 2026-11-21 | The new clippy gate let a `_` over one variant through (`match_wildcard_for_single_variants` is a separate lint) | P2 | deny-default | red-team | red-team | 1 | both lints denied, shown to fire (#56) | fixed | **1** (C04) | no |
| X03 | 2026-11-21 | `matches!` hid an implicit `_` from clippy in `uses_terminal` | P2 | deny-default | red-team | red-team | 1 | `Action::effect`, one exhaustive match (#56) | fixed | **2** (C04, X02) | no |
| X04 | 2026-11-21 | Undo of a move or a rename wrote with no `--root` check | P2 | multi-definition | red-team | red-team | 2 | `allows_at` before the undo (#55) | fixed | 0 | no |
| X05 | 2026-11-21 | `same_entry` took two hard links for one entry: pack dropped one, F2 "renamed" onto its own inode | P3 | multi-definition | QA | test | 2 | tests (#52) | fixed | 0 | no |
| X06 | 2026-11-21 | Windows compared a link by its name as written: changing a link's case was refused as taken | P1 | multi-definition | QA | CI | 2 | `one_place`; tests run on Windows CI (#52) | fixed | 0 | no |
| X07 | 2026-11-21 | A test cut a hand-built journal path by 2 hex digits; on Windows a path is 4 per UTF-16 unit | P3 | os-format | CI | test | 2 | one character per OS (#53) | fixed | 0 | no |
| X08 | 2026-11-21 | `qa defs` journal-kind marks matched nothing once the table landed: the gate went silent | P3 | evidence-stale | red-team | red-team | 3 | marks count the table (#53) | fixed | 0 | no |
| X09 | 2026-11-21 | The journal table's round-trip test could not see a renamed word (writer, reader and checker change together) | P2 | oracle-missing | QA | red-team | 2 | hand-spelled beta.1 journal fixture (#53) | fixed | 0 | no |
| X10 | 2026-11-21 | Pack, Save as and Log to file lacked Rename's Windows name rules | P3 | multi-definition | red-team | red-team | 2 | `validate_name` in `typed_destination` (#54) | fixed | 0 | no |
| S1 | 2026-11-21 | P8 round 1 (H-04): 8 planted faults. Caught: Claude QA 8/8, red-team 8/8, ChatGPT 6/8 and 5/8 strict (7/8 by quote), CI 3 (Windows, as a group) | — | (seeded) | QA, red-team, xmodel, CI | — | — | results in the owner's research notes | measured | — | yes |
| S2 | 2026-11-21 | P8 round 2 (H-07): 7 planted faults (4 more dropped: existing tests caught them). Caught: Claude QA 7/7, red-team 7/7, ChatGPT 7/7 twice, CI 0 | — | (seeded) | QA, red-team, xmodel | — | — | results in the owner's research notes | measured | — | yes |
| E01 | 2026-11-21 | The missing-folder walk goes up by text: `a/missing/../../other` is refused as into itself | P3 | interaction | red-team | test | 5 | — | open | 0 | no |
| E02 | 2026-11-21 | A move onto a hard link of itself reports done while nothing changed | P3 | interaction | red-team | test | 5 | — | open | 0 | no |
| E03 | 2026-11-21 | A journal killed while its `host` line was written is judged another machine's and never tidied | P3 | persisted-intent | red-team | red-team | 5 | kept cautious on purpose | open | 0 | no |
| E04 | 2026-11-21 | Unknown journal tags are skipped, where compatibility.md says a journal it cannot read is left alone | P3 | docs-claim | xmodel | QA | 5 | — | open | 0 | no |
| E05 | 2026-11-21 | F7 under `--root` closes its box on refusal; the other name boxes keep it open | P3 | interaction | red-team | QA | 5 | — | open | 0 | no |
| E06 | 2026-11-21 | Context-menu rows can look enabled and then refuse (`cmd_action` not used for enablement) | P3 | interaction | red-team | QA | 5 | — | open | 0 | no |
| X11 | 2026-11-22 | The fix for a worker panic: unwinding dropped the `Journal`, whose `Drop` removed it, so the crash erased its own record and "the next start offers to continue" was false | P1 | persisted-intent | red-team | author | 2 | `Drop` keeps it while panicking; a real-panic test (#61) | fixed | 0 | no |
| X12 | 2026-11-22 | The synthetic end of a panicked copy went down the real end's path: Ctrl+Z said "nothing to undo" after a copy that had copied | P2 | interaction | red-team | author | 2 | `Summary::stopped`; undo says it cannot (#61) | fixed | 0 | no |
| X13 | 2026-11-22 | The panic message promised "offers to continue" for every job; sync, extract, pack and rename many are only tidied up | P3 | docs-claim | xmodel | red-team | 4 | message says "tidies up" (#61) | fixed | 0 | no |
| W03 | 2026-11-22 | `syntax::scan::a_file_changed_while_scanned_loses_its_checkpoints` failed once on ARM CI on an unrelated PR (#59), passed on re-run | P3 | fragile-wait | CI | test | 4 | — | open | **2** (W01, W02) | no |
| E07 | 2026-11-22 | Other workers whose panic leaves their screen waiting: the planner's "counting…", the size count, the image decode, the hash (not on a data path) | P3 | scope | red-team | test | 5 | — | open | 0 | no |
