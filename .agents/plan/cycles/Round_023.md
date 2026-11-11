# Round 023: Names sorted as people read them, in any script (v0.4 Q, item 21)

**Status**: In Progress (the owner: "Move next", 2026-11-11)
**Part of**: v0.4.0, entry Q (scope proposal §2, Should: "item 21 … correctness for the
owner's own Japanese names; part of Q"); ROADMAP item 21
**Date started**: 2026-11-11
**Date completed**: YYYY-MM-DD

## Goal

Sorting by name puts names where a person looks for them, in Latin, Japanese, Korean
and full-width text, and the same name sorts the same however the file system stored
it (macOS writes `é` and `が` decomposed). No new dependency, and the listing budget
(100,000 entries in under a second) is kept.

## Plan (kickoff review)

### 1. Map: how names are compared today

`xdevcmd_core::sort::natural_cmp` (`sort.rs`) is used for the Name and Ext keys and as
the tie-break for Size and Modified, through `sort_entries`:
- runs of ASCII digits compare as numbers (`file2` before `file10`);
- every other character is compared after `to_ascii_lowercase`, so case folds **only
  for A–Z**;
- an exact tie falls back to the raw string, so the order never depends on the order
  the directory was read in.

What that gets wrong, by example (each is a test in step 2):

| Names | Today | Expected |
|---|---|---|
| `Éclair`, `apple`, `zebra` | apple, zebra, Éclair (É is after z) | apple, Éclair, zebra |
| `Ölfeld`, `öl` | case not folded outside A–Z | öl, Ölfeld |
| `résumé` (NFC) and `résumé` (NFD, from macOS) | apart: the NFD one is `e` + U+0301 | together |
| `が.txt` (NFC) and `が.txt` (NFD: か + U+3099) | apart | together |
| `アイ.txt`, `あう.txt`, `いえ.txt` | all hiragana before all katakana | あう, アイ, いえ (kana type is secondary) |
| `ｆｉｌｅ２`, `file10` (full width) | full width after all ASCII; `２` not a number | ｆｉｌｅ２, file10 |
| `한글` NFC and NFD (jamo, from macOS) | apart | together |

Kanji (`設計`, `資料`) have no order a program can know without a reading dictionary.
They stay in code-point order, as on Linux file managers; Explorer's Japanese sort is no
different for names it cannot read.

### 2. Critique

- **ICU-grade collation is out.** ICU4X data is megabytes, and the binary has a 7 MB
  budget with 2.1 MB to spare. The cases above need small, exact rules, not a full
  collation.
- **Normalisation without a crate.** `unicode-normalization` would add a dependency and
  about 100 KB of tables. What file names need is narrower, and each rule is
  arithmetic or a short table:
  - combining marks U+0300–U+036F after a Latin letter are dropped at the first level
    (an accent is a second-level difference);
  - kana voiced marks (U+3099, U+309A, and the spacing ゛゜) join the kana before them,
    by the Unicode rule (base + 1, + 2), as precomposed が and ぱ are;
  - Hangul jamo compose into syllables by the Unicode formula (L, V, T);
  - Latin-1 and Latin Extended-A letters with accents map to their base letter (a table
    of about 190 characters);
  - full-width ASCII (U+FF01–U+FF5E) maps to ASCII, so `２` is a digit;
  - katakana maps to hiragana at the first level.
- **Two levels, as collation does.** First the folded form (no case, no accents, no
  kana type, no width). Then the folded form with accents. Then the raw string, so
  the order is total and deterministic.
- **Speed.** The comparator runs about 1.7 million times for 100,000 names. Folding
  must not allocate: an iterator over folded characters with one character of
  look-ahead, used directly by the comparison. The budget test runs before and after.
- **Where it shows**: the Name and Ext keys, the tie-break of Size and Modified, and
  Alt+F7's results and lists (they sort the same way). The quick filter and the search
  (`/`) match by their own rules; changing them is a separate question, not this round.

### 3. Steps

1. `sort.rs`: `Fold`, an iterator of first-level units (a folded character, or a run of
   digits, ASCII or full width, as a number), with the tables and rules above.
   `natural_cmp` compares by `Fold`, then by accents, then raw. S.
2. Tests: a table of the cases in §1; full-width digits as numbers; the old ones
   unchanged (`file2` < `file10`, case as the tie-break); a property that the order is
   total (sorting a shuffled list twice gives the same result). The 100,000-entry
   budget run, its time recorded. S.
3. Docs: docs/guide.md, a line on name order; CHANGELOG. S.

### 4. The owner's decisions

Taken on these unless the owner says otherwise ("Move next"):
1. **Kana**: hiragana and katakana sort together (あ, ア, い, イ; kana type is a
   second-level difference), as Japanese sorting does. Or kept apart.
2. **Accents**: `é` sorts with `e`, and comes after it on a tie, as Explorer and Finder
   order it. Or by code point.
3. **Width**: full-width `Ａ` and `１` sort as `A` and `1`. Or apart.
4. **No new dependency**: hand-written rules for what file names need. Or the
   `unicode-normalization` crate (complete NFD; a dependency and about 100 KB).
5. **Vietnamese** (found while testing, the owner's own language): Unicode's default
   order, taken here, treats `ă â ê ô ơ ư` as their letter with a mark, so `ước`
   (u-o-c) sorts before `uống` (u-o-n-g), and `đ` as a letter after `d`. A Vietnamese
   dictionary makes `ă â ê ô ơ ư` letters of their own (`uống` before `ước`). Which
   one: Unicode's (as now, the same in every language), or Vietnamese (right for
   Vietnamese names, odd for French `ô`).

## Do

Coded on the recommendations ("Move next"). Decision 5 (Vietnamese) was found while
testing and is open; the code follows Unicode's default order.

- `natural_cmp` became three levels over `Fold`, an iterator of units: a digit run (as
  `Unit::Num`, length and the first 38 digits) or a letter's key, with a second-level
  mark.
  - Rules: full width to ASCII; Latin-1, Extended-A, `ơ ư` and the Vietnamese block
    to their base letter; `æ ð đ ł ø œ þ` as letters of their own after their base;
    combining marks as second-level; voiced kana as plain kana plus a mark (1 or 2,
    the same for `が` and `か`+U+3099); katakana as hiragana; Hangul jamo composed by
    the Unicode formula.
  - The tables are indexed through a `OnceLock<Vec<char>>`, not scanned.
- **An ASCII fast path** (`ascii_cmp`): when both names are ASCII, the old rules are
  exact, and byte-wise. A test checks it agrees with `Fold` on every pair of a set of
  edge names. Both cap digit runs at 38 digits, so a sort of mixed names sees one
  order; a non-total comparator can make Rust's sort panic.
- **Budget**: 100,000 entries listed and sorted in 0.85 s before, **1.88 s** with
  `Fold` alone (over budget), **0.41 s** with the fast path. Debug build.
- Tests:
  - the §1 table (Vietnamese included);
  - NFC and NFD next to each other (é, が, パ, 한글);
  - a total order independent of input order;
  - the tables' sizes;
  - fast path against full path.
- Docs: docs/guide.md (Panes and tabs), CHANGELOG (Changed).
- Not done: half-width katakana (U+FF61–U+FF9F) are left as they are; rare in file
  names.

## Check

- [x] The §1 table, each row a test
- [x] Old natural-order tests unchanged; the order total and deterministic
- [x] 100,000 entries in under 1 s: 0.41 s (debug), from 0.85 s
- [x] Clippy for three targets, the full suite, 660 passed, 0 failed
- [ ] The owner: a folder of Japanese names on Windows, and the same copied from a Mac;
  decision 5
