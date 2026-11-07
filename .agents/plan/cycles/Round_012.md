# Round 012: Two mouse touches — the path, and an even border

**Status**: In Progress
**Part of**: [ROADMAP.md](../ROADMAP.md); ideas from the owner, 2026-11-07
**Date started**: 2026-11-07 (owner: "hoãn v0.2.0 lại để thêm 2 cái này")
**Date completed**:

## Goal

Two small things the mouse should do, before 0.2.0: click a directory in a pane's path
to go there, as in a file browser's breadcrumb; and double-click a border to share it
evenly between the two panes it separates — only those two, when there are more
(owner: "nếu 3 panes (>2) thì phải cẩn thận, chỉ tính chung của 2 panes đang thao tác").

## Plan

- [x] The path: the pane records where each directory name of its title was drawn,
      following `elide_middle`'s layout, so a name cut by the `…` is clickable on what
      shows and one hidden by it is not. A click goes there through `set_cwd`, which
      checks `--root` and leaves an archive as Alt+G would.
- [x] The border: `Node::even` flattens the same-axis splits under the boundary's
      split into pieces (a pane, or a stack split the other way), sets the two pieces
      either side to their average and writes the percentages back, so every other
      piece keeps its size. A double-click is two presses on the same cell within the
      list's double-click time; one press still starts a drag.
- [x] The same for a boundary between shells in a terminal tab.
- [x] Tests: `crumbs` (plain and elided), `Node::even` (two panes, `A|(B|C)` both ways,
      `(A|B)|C`, a stack), render tests for both.
- [x] guide.md (Mouse), CHANGELOG.

## Do

Built on `feat/round-012`, cut from `feat/pre-release-review` (the docs and security
review, PR #11), so the final review covers both.

- The boundary between the panes and the terminal panel is a boundary like any
  other: a double-click shares it evenly too (half the height each).

## Check

- [x] Unit and render tests above; the whole suite, clippy and fmt pass locally.
- [ ] The owner, by hand, in a real terminal.
- [ ] A last docs and security review over this round and the review branch.

Record what you could **not** verify as explicitly as what you could.

## Act

**Learnings**:

- ...

**Promotions**:

- (none yet)
