use std::cmp::Ordering;

use crate::entry::{Entry, Kind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Ext,
    Size,
    Modified,
}

#[derive(Debug, Clone, Copy)]
pub struct Sort {
    pub key: SortKey,
    pub reverse: bool,
    /// Directories above files regardless of the key. The orthodox default, and the one
    /// people complain loudest about when it is missing.
    pub dirs_first: bool,
}

impl Default for Sort {
    fn default() -> Self {
        Self {
            key: SortKey::Name,
            reverse: false,
            dirs_first: true,
        }
    }
}

pub fn sort_entries(entries: &mut [Entry], s: Sort) {
    entries.sort_by(|a, b| {
        // `..` is never sorted; it is the way out of the directory and lives at the top.
        match (a.kind == Kind::Parent, b.kind == Kind::Parent) {
            (true, true) => return Ordering::Equal,
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            (false, false) => {}
        }

        if s.dirs_first {
            match (a.is_dir(), b.is_dir()) {
                (true, false) => return Ordering::Less,
                (false, true) => return Ordering::Greater,
                _ => {}
            }
        }

        let ord = match s.key {
            SortKey::Name => natural_cmp(&a.name, &b.name),
            SortKey::Ext => {
                let (_, ae) = a.stem_ext();
                let (_, be) = b.stem_ext();
                natural_cmp(ae, be).then_with(|| natural_cmp(&a.name, &b.name))
            }
            SortKey::Size => a
                .size
                .cmp(&b.size)
                .then_with(|| natural_cmp(&a.name, &b.name)),
            SortKey::Modified => a
                .modified
                .cmp(&b.modified)
                .then_with(|| natural_cmp(&a.name, &b.name)),
        };

        if s.reverse {
            ord.reverse()
        } else {
            ord
        }
    });
}

/// Compare names the way a person reads them: `file2` before `file10`, `Éclair` with
/// `eclair`, `が` with `か`, `ア` with `あ`, full-width `ｆｉｌｅ２` with `file2`, and a name
/// the same however the file system stored it (macOS writes `é` and `が` decomposed).
///
/// Three levels, as collation has: the folded name (no case, accents, voicing, kana
/// type or width; digit runs as numbers); then the accents; then the raw string, so
/// the order is total and never depends on the order a directory was read in. Kanji
/// stay in code-point order: nothing short of a reading dictionary knows better.
///
/// No collation library: ICU's data is megabytes. File names need a short list of
/// exact rules ([`Fold`]), and this runs about 1.7 million times to sort 100,000
/// names, so it allocates nothing.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    // Most names are ASCII, and for those folding A-Z is all there is to do: the quick
    // way, which keeps the 100,000-entry budget.
    if a.is_ascii() && b.is_ascii() {
        return ascii_cmp(a, b);
    }
    let (mut fa, mut fb) = (Fold::new(a), Fold::new(b));
    let mut accents = Ordering::Equal;
    loop {
        match (fa.next(), fb.next()) {
            (None, None) => return accents.then_with(|| a.cmp(b)),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some((ua, xa)), Some((ub, xb))) => {
                let o = ua.cmp(&ub);
                if o != Ordering::Equal {
                    return o;
                }
                if accents == Ordering::Equal {
                    accents = xa.cmp(&xb);
                }
            }
        }
    }
}

/// [`natural_cmp`] for two ASCII names: digit runs as numbers, A-Z folded, then raw.
fn ascii_cmp(a: &str, b: &str) -> Ordering {
    let (a_b, b_b) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < a_b.len() && j < b_b.len() {
        let (x, y) = (a_b[i], b_b[j]);
        if x.is_ascii_digit() && y.is_ascii_digit() {
            let run = |s: &[u8], mut k: usize| {
                while k < s.len() && s[k] == b'0' {
                    k += 1;
                }
                let start = k;
                while k < s.len() && s[k].is_ascii_digit() {
                    k += 1;
                }
                (start, k)
            };
            let ((sa, ea), (sb, eb)) = (run(a_b, i), run(b_b, j));
            // The first 38 digits, as `Unit::Num` keeps: the two ways must agree on
            // every pair, or a sort of mixed names would see two orders.
            let first = |s: usize, e: usize| s..e.min(s + 38);
            let ord = (ea - sa)
                .cmp(&(eb - sb))
                .then_with(|| a_b[first(sa, ea)].cmp(&b_b[first(sb, eb)]));
            if ord != Ordering::Equal {
                return ord;
            }
            (i, j) = (ea, eb);
        } else {
            let ord = x.to_ascii_lowercase().cmp(&y.to_ascii_lowercase());
            if ord != Ordering::Equal {
                return ord;
            }
            i += 1;
            j += 1;
        }
    }
    (a_b.len() - i).cmp(&(b_b.len() - j)).then_with(|| a.cmp(b))
}

/// One unit of a folded name: a run of digits, as a number, or a letter's first-level
/// key. Digits sort before letters, as `0`–`9` come before `a` in ASCII.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    /// Significant digits (leading zeros dropped) and the first 38 of them as a value.
    Num {
        len: usize,
        value: u128,
    },
    Char(u32),
}

impl Ord for Unit {
    fn cmp(&self, other: &Self) -> Ordering {
        let digit = key('0');
        match (self, other) {
            (Unit::Num { len: a, value: x }, Unit::Num { len: b, value: y }) => {
                a.cmp(b).then(x.cmp(y))
            }
            (Unit::Num { .. }, Unit::Char(k)) => digit.cmp(k).then(Ordering::Less),
            (Unit::Char(k), Unit::Num { .. }) => k.cmp(&digit).then(Ordering::Greater),
            (Unit::Char(x), Unit::Char(y)) => x.cmp(y),
        }
    }
}

impl PartialOrd for Unit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A character's first-level key: its lower case, spaced so a letter that is its own
/// letter after another (`đ` after `d`) can sit between them.
fn key(c: char) -> u32 {
    let lower = c.to_lowercase().next().unwrap_or(c);
    (lower as u32) << 2
}

/// A name, folded unit by unit, each with its second-level mark (0 for none): an
/// accent, a kana voicing mark. The rules:
/// - full-width ASCII is ASCII (`Ａ`, `２`), the ideographic space a space;
/// - digit runs, ASCII or full width, are numbers;
/// - Latin letters with accents (Latin-1, Extended-A, Vietnamese, `ơ` `ư`) are their
///   base letter, the accent a second-level mark; `đ`, `ð`, `ł`, `ø`, `æ`, `œ` are
///   letters of their own, after `d`, `d`, `l`, `o`, `a`, `o`;
/// - combining marks (U+0300–U+036F) are second-level marks of the letter before;
/// - kana: voiced and semi-voiced kana are their plain kana with a mark, written
///   precomposed (`が`) or not (`か` + U+3099); katakana are hiragana;
/// - Hangul jamo (macOS) are composed into syllables.
struct Fold<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl<'a> Fold<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            chars: s.chars().peekable(),
        }
    }

    fn digit(&mut self) -> Option<u32> {
        width(*self.chars.peek()?).to_digit(10)
    }
}

impl Iterator for Fold<'_> {
    type Item = (Unit, u32);

    fn next(&mut self) -> Option<(Unit, u32)> {
        if self.digit().is_some() {
            let (mut len, mut value) = (0usize, 0u128);
            while let Some(d) = self.digit() {
                self.chars.next();
                if len == 0 && d == 0 {
                    continue;
                }
                len += 1;
                if len <= 38 {
                    value = value * 10 + u128::from(d);
                }
            }
            return Some((Unit::Num { len, value }, 0));
        }
        let c = width(self.chars.next()?);
        let (mut k, mut mark) = fold_char(c);
        // Hangul as macOS writes it: a leading consonant and a vowel (and a final).
        if (0x1100..=0x1112).contains(&(c as u32)) {
            if let Some(v) = self
                .chars
                .peek()
                .map(|&v| v as u32)
                .filter(|v| (0x1161..=0x1175).contains(v))
            {
                self.chars.next();
                let t = self
                    .chars
                    .peek()
                    .map(|&t| t as u32)
                    .filter(|t| (0x11A8..=0x11C2).contains(t))
                    .map_or(0, |t| {
                        self.chars.next();
                        t - 0x11A7
                    });
                let s = 0xAC00 + ((c as u32 - 0x1100) * 21 + (v - 0x1161)) * 28 + t;
                k = s << 2;
            }
        }
        // Marks written after the letter they belong to.
        while let Some(m) = self.chars.peek().and_then(|&n| combining(n)) {
            self.chars.next();
            mark = mark.wrapping_mul(0x401).wrapping_add(m);
        }
        Some((Unit::Char(k), mark))
    }
}

/// Full width to ASCII; the ideographic space to a space.
fn width(c: char) -> char {
    match c as u32 {
        u @ 0xFF01..=0xFF5E => char::from_u32(u - 0xFEE0).unwrap_or(c),
        0x3000 => ' ',
        _ => c,
    }
}

/// A combining mark's second-level value; `None` if `c` is not one. Kana voicing
/// marks, combining or spacing, give the values precomposed kana do (1 and 2).
fn combining(c: char) -> Option<u32> {
    match c as u32 {
        0x3099 | 0x309B => Some(1),
        0x309A | 0x309C => Some(2),
        u @ 0x0300..=0x036F => Some(u),
        _ => None,
    }
}

/// Latin-1 Supplement, U+00C0–U+00FF: each letter's base, or itself.
const LATIN1: &str = "AAAAAAÆCEEEEIIIIÐNOOOOO×ØUUUUYÞsaaaaaaæceeeeiiiiðnooooo÷øuuuuyþy";
/// Latin Extended-A, U+0100–U+017F.
const LATIN_A: &str = "AaAaAaCcCcCcCcDdĐđEeEeEeEeEeGgGgGgGgHhHhIiIiIiIiIiIiJjKkkLlLlLlLlŁłNnNnNnnNnOoOoOoŒœRrRrRrSsSsSsSsTtTtTtUuUuUuUuUuUuWwYyYZzZzZzs";

static LATIN1_T: std::sync::OnceLock<Vec<char>> = std::sync::OnceLock::new();
static LATIN_A_T: std::sync::OnceLock<Vec<char>> = std::sync::OnceLock::new();

/// A table's characters, indexed.
fn table(t: &'static std::sync::OnceLock<Vec<char>>, s: &str) -> &'static [char] {
    t.get_or_init(|| s.chars().collect())
}

/// A character's first-level key and second-level mark.
fn fold_char(c: char) -> (u32, u32) {
    let u = c as u32;
    if c.is_ascii() {
        return (u32::from(c.to_ascii_lowercase()) << 2, 0);
    }
    let base = match u {
        0x00C0..=0x00FF => table(&LATIN1_T, LATIN1).get((u - 0xC0) as usize).copied(),
        0x0100..=0x017F => table(&LATIN_A_T, LATIN_A)
            .get((u - 0x100) as usize)
            .copied(),
        0x01A0 => Some('O'),
        0x01A1 => Some('o'),
        0x01AF => Some('U'),
        0x01B0 => Some('u'),
        // Vietnamese, U+1EA0–U+1EF9: capital and small in turn.
        0x1EA0..=0x1EB7 => Some('a'),
        0x1EB8..=0x1EC7 => Some('e'),
        0x1EC8..=0x1ECB => Some('i'),
        0x1ECC..=0x1EE3 => Some('o'),
        0x1EE4..=0x1EF1 => Some('u'),
        0x1EF2..=0x1EF9 => Some('y'),
        _ => None,
    };
    if let Some(b) = base {
        // Letters of their own: just after the letter they come from.
        let own = match b.to_lowercase().next().unwrap_or(b) {
            'æ' => Some('a'),
            'ð' | 'đ' => Some('d'),
            'ł' => Some('l'),
            'ø' | 'œ' => Some('o'),
            'þ' => Some('z'),
            _ => None,
        };
        if let Some(o) = own {
            return (key(o) | 1, 0);
        }
        let mark = if b == c { 0 } else { u };
        return (key(b), mark);
    }
    // Kana: voicing is second level; katakana sort as hiragana.
    if let Some((plain, mark)) = unvoice(u) {
        return (kana(plain) << 2, mark);
    }
    if (0x30A1..=0x30F6).contains(&u) {
        return (kana(u) << 2, 0);
    }
    (key(c), 0)
}

/// A voiced or semi-voiced kana as its plain kana and mark (1 voiced, 2 semi).
fn unvoice(u: u32) -> Option<(u32, u32)> {
    let hira = if (0x30A1..=0x30F6).contains(&u) {
        u - 0x60
    } else {
        u
    };
    let kata = hira != u;
    let back = |p: u32| if kata { p + 0x60 } else { p };
    let plain = match hira {
        0x3094 => (0x3046, 1),
        // か…ど: the voiced one follows its plain one.
        0x304C | 0x304E | 0x3050 | 0x3052 | 0x3054 | 0x3056 | 0x3058 | 0x305A | 0x305C | 0x305E
        | 0x3060 | 0x3062 | 0x3065 | 0x3067 | 0x3069 => (hira - 1, 1),
        // は…ぽ: plain, voiced, semi-voiced.
        0x3070 | 0x3073 | 0x3076 | 0x3079 | 0x307C => (hira - 1, 1),
        0x3071 | 0x3074 | 0x3077 | 0x307A | 0x307D => (hira - 2, 2),
        _ => return None,
    };
    Some((back(plain.0), plain.1))
}

/// Katakana as hiragana.
fn kana(u: u32) -> u32 {
    if (0x30A1..=0x30F6).contains(&u) {
        u - 0x60
    } else {
        u
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn e(name: &str, kind: Kind, size: u64) -> Entry {
        Entry {
            name: name.to_string(),
            path: PathBuf::from(name),
            kind,
            size,
            modified: None,
            hidden: false,
            symlink: false,
            error: None,
        }
    }

    fn names(v: &[Entry]) -> Vec<&str> {
        v.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn numbers_sort_as_numbers() {
        assert_eq!(natural_cmp("file2", "file10"), Ordering::Less);
        assert_eq!(natural_cmp("file10", "file9"), Ordering::Greater);
        assert_eq!(natural_cmp("a1b2", "a1b10"), Ordering::Less);
    }

    #[test]
    fn leading_zeros_do_not_change_the_value() {
        // 007 and 8 compare as 7 and 8, not as text where "0" < "8" by accident.
        assert_eq!(natural_cmp("v007", "v8"), Ordering::Less);
        assert_eq!(natural_cmp("v008", "v7"), Ordering::Greater);
        // Equal in value, so the raw string breaks the tie and the order is stable
        // rather than dependent on the order the directory happened to be read in.
        assert_eq!(natural_cmp("v007", "v7"), Ordering::Less);
        assert_eq!(natural_cmp("v7", "v007"), Ordering::Greater);
    }

    #[test]
    fn case_does_not_drive_the_ordering() {
        // "alpha" sorts next to "Alpha", not away in a separate uppercase block.
        assert_eq!(natural_cmp("alpha", "Beta"), Ordering::Less);
        assert_eq!(natural_cmp("Beta", "alpha"), Ordering::Greater);
        // Same word, different case: ordered by the raw bytes so it is deterministic.
        assert_eq!(natural_cmp("Alpha", "alpha"), Ordering::Less);
    }

    #[test]
    fn parent_is_first_and_dirs_lead_files() {
        let mut v = vec![
            e("zeta.txt", Kind::File, 1),
            e("alpha", Kind::Dir, 0),
            e("..", Kind::Parent, 0),
            e("beta.txt", Kind::File, 2),
            e("omega", Kind::Dir, 0),
        ];
        sort_entries(&mut v, Sort::default());
        assert_eq!(names(&v), ["..", "alpha", "omega", "beta.txt", "zeta.txt"]);
    }

    #[test]
    fn reverse_leaves_parent_and_dir_grouping_alone() {
        let mut v = vec![
            e("..", Kind::Parent, 0),
            e("a.txt", Kind::File, 1),
            e("b.txt", Kind::File, 2),
            e("dir", Kind::Dir, 0),
        ];
        sort_entries(
            &mut v,
            Sort {
                reverse: true,
                ..Sort::default()
            },
        );
        assert_eq!(names(&v), ["..", "dir", "b.txt", "a.txt"]);
    }

    #[test]
    fn file10_regression() {
        let mut v = vec![
            e("file10", Kind::File, 0),
            e("file9", Kind::File, 0),
            e("file1", Kind::File, 0),
        ];
        sort_entries(&mut v, Sort::default());
        assert_eq!(names(&v), ["file1", "file9", "file10"]);
    }

    /// `names` sorted by name.
    fn by_name(names: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = names.iter().map(|s| s.to_string()).collect();
        v.sort_by(|a, b| natural_cmp(a, b));
        v
    }

    #[test]
    fn the_tables_are_the_size_of_their_blocks() {
        assert_eq!(LATIN1.chars().count(), 0x40);
        assert_eq!(LATIN_A.chars().count(), 0x80);
    }

    #[test]
    fn names_sort_as_people_read_them_in_any_script() {
        let rows: &[(&[&str], &[&str])] = &[
            // Accents with their letter, after it on a tie; case folds outside A-Z.
            (
                &["zebra", "Éclair", "apple", "eclair"],
                &["apple", "eclair", "Éclair", "zebra"],
            ),
            (&["Ölfeld", "öl", "oma"], &["öl", "Ölfeld", "oma"]),
            // Vietnamese, by Unicode's default order: ơ, ư, ạ… are their letter with a
            // mark (so ước, u-o-c, before uống, u-o-n-g); đ is a letter after d. A
            // Vietnamese dictionary would put ư after u: the owner's call (Round 023).
            (
                &["đường", "dao", "ước", "uống", "em"],
                &["dao", "đường", "em", "ước", "uống"],
            ),
            // Kana: katakana with hiragana; voicing second.
            (
                &["アイ.txt", "いえ.txt", "あう.txt"],
                &["アイ.txt", "あう.txt", "いえ.txt"],
            ),
            (&["がか", "かき", "かか"], &["かか", "がか", "かき"]),
            // Full width: letters as ASCII, digits as numbers.
            (&["file10", "ｆｉｌｅ２"], &["ｆｉｌｅ２", "file10"]),
            // Numbers still numbers.
            (&["img12", "img2", "img1"], &["img1", "img2", "img12"]),
        ];
        for (input, want) in rows {
            assert_eq!(by_name(input), *want, "{input:?}");
        }
    }

    #[test]
    fn a_name_sorts_the_same_however_it_was_stored() {
        // macOS stores names decomposed: e + U+0301, か + U+3099, Hangul jamo.
        let pairs = [
            ("r\u{e9}sum\u{e9}", "re\u{301}sume\u{301}"),
            ("\u{304c}.txt", "\u{304b}\u{3099}.txt"),
            ("\u{30d1}\u{30f3}", "\u{30cf}\u{309a}\u{30f3}"),
            (
                "\u{d55c}\u{ae00}",
                "\u{1112}\u{1161}\u{11ab}\u{1100}\u{1173}\u{11af}",
            ),
        ];
        for (nfc, nfd) in pairs {
            // Nothing sorts between the two forms: they are next to each other.
            let mut v = by_name(&[
                "resume", "zzz", "aaa", "\u{304b}", "\u{304d}", nfc, nfd, "\u{d55c}", "\u{d560}",
            ]);
            let at = v.iter().position(|n| n == nfc).unwrap();
            let bt = v.iter().position(|n| n == nfd).unwrap();
            assert_eq!(at.abs_diff(bt), 1, "{nfc:?} and {nfd:?} apart in {v:?}");
            v.clear();
        }
    }

    #[test]
    fn the_order_is_total_and_does_not_depend_on_the_input_order() {
        let names = [
            "B",
            "b",
            "a",
            "A",
            "é",
            "e",
            "É",
            "ア",
            "あ",
            "が",
            "か\u{3099}",
            "１",
            "1",
            "01",
            "file2",
            "file10",
            "設計",
            "資料",
            "_x",
            "-y",
            "x y",
        ];
        let sorted = by_name(&names);
        let mut reversed = names;
        reversed.reverse();
        assert_eq!(by_name(&reversed), sorted);
        for w in sorted.windows(2) {
            assert_ne!(natural_cmp(&w[0], &w[1]), Ordering::Equal, "{w:?}");
            assert_eq!(natural_cmp(&w[1], &w[0]), Ordering::Greater, "{w:?}");
        }
    }

    #[test]
    fn the_quick_ascii_way_agrees_with_the_full_one() {
        let names = [
            "a",
            "A",
            "b",
            "a1",
            "a01",
            "a10",
            "a9",
            "a_",
            "a-",
            "a/",
            "a:",
            "a.txt",
            "a b",
            "file2",
            "file10",
            "File2",
            "x0",
            "x00",
            "",
            "9",
            "10",
            "z",
            "n12345678901234567890123456789012345678901",
            "n12345678901234567890123456789012345678902",
        ];
        for a in names {
            for b in names {
                let (mut fa, mut fb) = (Fold::new(a), Fold::new(b));
                let mut full = Ordering::Equal;
                loop {
                    match (fa.next(), fb.next()) {
                        (None, None) => break,
                        (None, Some(_)) => {
                            full = Ordering::Less;
                            break;
                        }
                        (Some(_), None) => {
                            full = Ordering::Greater;
                            break;
                        }
                        (Some((x, _)), Some((y, _))) => {
                            full = x.cmp(&y);
                            if full != Ordering::Equal {
                                break;
                            }
                        }
                    }
                }
                let full = full.then_with(|| a.cmp(b));
                assert_eq!(ascii_cmp(a, b), full, "{a:?} vs {b:?}");
            }
        }
    }
}
