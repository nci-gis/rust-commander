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
| `class` | `scope` · `interaction` · `multi-definition` · `deny-default` · `persisted-intent` · `docs-claim` · `evidence-stale` · `oracle-missing` · `tool-hazard` · `fragile-wait` · `process` |
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
| W01 | 2026-11-18 | `wait_for("copied")` matched the Interrupted dialog, and F10 then went to the job | P3 | fragile-wait | CI | test | 4 | waits on the status line | fixed | 0 | no |
| W02 | 2026-11-20 | `wait_for("deeper ")` matched a list row before Enter was taken | P3 | fragile-wait | CI | test | 4 | waits for the pane title (#49) | fixed | **1** | no |
| P01 | 2026-11-18 | #43 closed itself when its base PR (#42) merged | P3 | process | daily-use | (process) | 4 | no stacked PRs (plan v3) | held | 0 | no |
| C01 | 2026-11-19 | Copy into itself through a link to a folder not made yet: stack overflow (copy, move, plan) | P1 | multi-definition | research | QA | 5 | H-04 | open | 0 | no |
| C02 | 2026-11-19 | Pack's destination name is not checked for control characters | P1 | multi-definition | research | red-team | 5 | H-08 | open | 0 | no |
| C03 | 2026-11-19 | Four root checks with different rules; restore from the trash under `--root` likely always refused | P2 | multi-definition | research | red-team | 5 | H-03 | open | 0 | no |
| C04 | 2026-11-19 | `refused()` lets an unlisted action through (`OpenWithMenu` in read-only) | P2 | deny-default | research | red-team | 5 | H-05 | open | 0 | no |
| C05 | 2026-11-19 | `same_file` and `same_entry` differ on links: rename-many plans what apply refuses | P2 | multi-definition | research | test | 5 | H-04 | open | 0 | no |
| C06 | 2026-11-19 | A torn last `done` journal line marks a different folder done | P1 | persisted-intent | research | test | 5 | H-07 | open | 0 | no |
| C07 | 2026-11-19 | An interrupted sync (Alt+Y) is offered, then dropped with its kept parts; the docs say "Covered" | P2 | docs-claim | research | QA | 5 | H-07 | open | 0 | no |
| C08 | 2026-11-19 | `known()` drops 5 newer commands when the list is reloaded | P3 | multi-definition | research | test | 5 | H-10 | open | 0 | no |
| C09 | 2026-11-19 | Journal kinds `part` and `gone` are read but never written | P3 | multi-definition | research | red-team | 5 | H-07 | open | 0 | no |
| C10 | 2026-11-19 | `--kill-when` killed every xdevcmd by name | P2 | tool-hazard | research | red-team | 1 | `--launch` and a PID file (#47) | fixing | 0 | no |
| C11 | 2026-11-19 | A worker panic leaves the job "working"; two early returns skip restoring the terminal | P2 | scope | research | test | 5 | H-10 | open | 0 | no |
| C12 | 2026-11-19 | The result of setting the journal folder to 0700 is ignored | P3 | deny-default | research | red-team | 5 | H-10 | open | 0 | no |
| C13 | 2026-11-19 | `XDEVCMD_STATE`: a file to the code, a folder to the man page; a folder put the journal beside it | P2 | docs-claim | research | QA | 2 | refused at start, docs agree (#48) | fixing | 0 | no |
| C14 | 2026-11-19 | Manual-case Expected lines quote text that is not in `docs/` | P3 | oracle-missing | research | QA | 4 | lint planned (plan v3 P4) | open | 0 | no |
