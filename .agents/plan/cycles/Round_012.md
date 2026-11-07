# Round 012: Two mouse touches — the path, and an even border

**Status**: Complete
**Part of**: [ROADMAP.md](../ROADMAP.md); ideas from the owner, 2026-11-07
**Date started**: 2026-11-07 (owner: "hoãn v0.2.0 lại để thêm 2 cái này")
**Date completed**: 2026-11-07 (owner tested: "đã test xong, confirm okay nhé"; merged in #12)

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
- The final review found the path click counted characters, not cells (CJK names,
  macOS's decomposed accents): it now measures with `unicode-width`. It also found
  that directories given relative on the command line stayed relative, which broke
  the title and Backspace: they are made absolute at start.
- The final security review's fixes landed on this branch too: Alt+F5 under `--root`,
  an archive linked from outside the root, Shift+F4 on a link out, the allowlist even
  with `[commands] opener` set, a shorter allowlist (no web pages, SVG, XML, old office
  or OpenDocument files), more types that ask "Run it?", quick view opened without
  blocking and checked on the handle, batch files found as the pty library finds them,
  the temp directory tied to its identity and refreshed every 30 s, Windows device
  names refused as theme names, a dangling linked config written through.

## Check

- [x] Unit and render tests above; the whole suite, clippy and fmt pass locally.
- [x] The owner, by hand, in a real terminal (2026-11-07).
- [x] A last docs and security review over this round and the review branch; what it
      found is fixed here, with tests, except what the memory file lists as open.

Record what you could **not** verify as explicitly as what you could.

## Act

**Learnings**:

- **A boundary in a binary layout tree is not between two panes**: it is between two
  subtrees. Sharing "the two panes beside it" means flattening the same-axis splits
  into pieces and changing only the two at the boundary.
- **Clicks on drawn text must be measured in cells, not characters**: wide CJK
  characters and macOS's decomposed accents put a character count off by several cells.
- A final review after a review still found real problems (packing outside the root,
  a configured opener skipping the question): worth the time before a release.

**Promotions**:

- (none yet)
