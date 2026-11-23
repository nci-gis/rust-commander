# Review lessons

> **Read this when**: you run the QA or red-team skill, triage a finding, or decide
> how to stop a class of defect from coming back. The defects themselves are in
> [skills/qa/references/past-misses.md](../skills/qa/references/past-misses.md).

---

Each lesson names a class of defect that got past us and the control that should
stop it. Controls are ranked on a **ladder**: the higher the rung, the less it
depends on someone remembering.

| Rung | Control |
|---|---|
| 1 | A design that makes the defect impossible (one definition, one table) |
| 2 | The compiler or a test breaks |
| 3 | A script or CI check finds it |
| 4 | A rule in a skill or doc |
| 5 | Judgement |

**Escalation:** a class that recurs while it is held only at rung 4 or 5 moves to
rung 3 or higher before the next cut. "Recurs" means a second entry in
past-misses with the same class.

Sources: the external review and hand tests of 2026-11-17/18, the cosf triangle
study R1–R7 with its follow-up of 2026-11-19/20, and the beta.2 hardening of
2026-11-21 (L18–L22: red-team, blind QA, cross-model review and P8; the owner's
research notes, outside this repository). The cosf plan's own numbering was not kept;
L16 and L17 match it.

| ID | Lesson | Evidence | Rung now → target | Where it applies | Status |
|---|---|---|---|---|---|
| L1 | One concept, one definition. Copies of a concept drift apart, and a fix reaches one copy. | Git history: lines deleted per line added 0.16 (yazi 0.53, MC 0.74); RR02 (journal kinds versus `may_tidy`); cosf #3 (4 root checks), #5 (`same_file` vs `same_entry`) | 4 → 1 | Plan v3 consolidation (H-03 to H-08); red-team scope | open |
| L2 | A fix must reach every copy ("sibling proof"). | ca48a81 (USERPROFILE fixed in bookmarks only), 84e6a24 (Save As fixed, Pack missed), b505ba1 | 4 → 3 | Red-team of every data-path PR | open |
| L3 | Red-team the boundary a change sits on, not only its diff: the siblings, then the callers. | RR01–RR03 sat outside every reviewed diff | 4 | red-team skill | open |
| L4 | Features break where they meet. List the new feature crossed with each mode: verify (C), `--root`, read-only, resume, Copy As, a failed removal. | RR01 (C × checkpoint), RR03 (root × recovery), RR04 (Copy As × resume), RR06 (F × failed removal) | 4 → 3 | QA skill, step 1 | open |
| L5 | A status line comes from the outcome, never from the input. | RR06: F said "removed" while removal errors were ignored | 4 | red-team; code review | open |
| L6 | A safety check denies by default: `_ =>` in a refusal or filter is a defect. Deny both clippy lints (`wildcard_enum_match_arm` misses a `_` over one variant), and do not hide a list in `matches!`. | RR02, cosf #4, X02, X03 | **1** | `Action::effect` and `cmd_action`, exhaustive (H-05, #56) | fixed (recurred twice inside H-05) |
| L7 | Evidence older than the code is no evidence. | 14 commits on data paths after the last QA (cosf R2) | 4 → 3 | `evidence_ledger` (plan v3 P2) | open |
| L8 | Every promise in the docs is checked against the code by someone other than its writer. Absolute words ("never", "every", "nothing") get a scope check. | R01–R15; cosf #7 (Alt+Y documented as Covered) | 4 → 3 | docs reviews; `docs_claims` (rc) | open |
| L9 | Put what the reader must do where they do it. | R09; the `reset` hint at check A (#45) | 4 | the docs reference | open |
| L10 | Every role is the same model family, so its blind spots are shared. | cosf 4.4 | 5 → 3 | Plan v3 P7 (cross-model review), P8 (planted faults with a known count) | open |
| L11 | A test that waits for text must wait for text that appears for that reason only. | `wait_for("copied")` matched the dialog (2026-11-18); `wait_for("deeper ")` matched a row (#49) | **3** | `tests/waits.rs`: a bare-word wait needs a `// wait:` reason; the 44 older ones may only go down (#58) | escalated |
| L12 | An intent written to disk must hold every field a resume needs. | RR04 (Copy As name), RR05 (decisions) | 4 → 2 | One journal-line table (H-07, #53); a `Request` destructure test is still to do | partly |
| L13 | A record cut short is not a record. A parser of stored lines fails closed. | cosf #6: a torn `done` line marks another folder done | **2** | `whole_lines`, test (H-07, #53) | fixed |
| L14 | A pull request based on another one closes when that one merges. | #43 | 4 | Plan v3: every PR is based on the integration branch | held |
| L15 | A gate is trusted only after it has been shown to fire, for each case it claims. | the red-team skill; `evidence_ledger`'s fixture; X02 (a lint that did not fire for one variant); X08 (marks that went silent) | 4 → 2 | every new check, script or lint: break it on purpose once | held |
| L16 | The oracle must exist in the docs: an Expected line quotes `docs/`. | Manual cases whose quotes appear only in `tests/pty.rs` (cosf V1a); 8 of 13 quotes were not in `docs/` | **2** | `tests/manual_cases.rs` (P4) | fixed |
| L17 | A test tool never touches the owner's real data or processes. | `--kill-when` killed every xdevcmd by name; isolating `HOME` tests the wrong target | **1** | `xtask try --launch` with a PID file (H-01, #47) | fixed |
| L18 | Check the input as it was given, before normalising it. A check after `trim`, case folding or a lossy conversion misses what that step removed. | X01 (found by ChatGPT, missed by Claude) | 4 → 2 | every validator of typed or stored text | open |
| L19 | A stored format is pinned by a fixture spelled by hand in the old format. A round trip through one table cannot see a word changed in that table. | X09; P8 round 2 (S2, a renamed `skip`) | 4 → 2 | journals, config, session files | open |
| L20 | One inode is not one entry, and one name is not one entry on another OS. Identity is "the same name in the same folder", told by the folder's own listing. | X05, X06 | **1** | `paths::same_entry` / `one_place` (H-04, #52) | fixed |
| L21 | A test that builds stored data by hand follows each OS's encoding (bytes on Unix, UTF-16 units on Windows). | X07 | 4 | tests of journals and paths | open |
| L22 | Planting faults inside the diff under review measures "reads every changed line", not "finds what the change left out". | S1, S2: every layer 7–8 of 7–8 | 5 | a later P8 plants omissions (a sibling reverted) | open |
| L23 | A panic unwinds: every `Drop` on the way runs. A `Drop` that tidies up (removes a journal, a temp file, a lock) erases the crash's record unless it checks `std::thread::panicking()`. | X11 | 2 | the journal (#61); termlog already did | open (audit other tidying `Drop`s) |
| L24 | A made-up result goes down the real result's path. Mark it, and check what each consumer does with it (undo, status, counts). | X12 | 4 | — | open |
