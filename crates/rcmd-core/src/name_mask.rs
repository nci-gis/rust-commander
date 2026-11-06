//! New names from a pattern, for renaming many files at once (Shift+F2).
//!
//! Two masks build the name and the extension; then find-and-replace runs on the whole
//! name, then the case changes. Pure: nothing here touches the disk. The result goes
//! through [`crate::batch_rename::plan`], the same check the editor route uses.
//!
//! | In a mask       | Becomes                                                     |
//! |-----------------|-------------------------------------------------------------|
//! | `[N]`           | the name without its extension                              |
//! | `[N2-5]`        | its 2nd to 5th characters; `[N3-]` from the 3rd; `[N1]` one |
//! | `[E]`           | the extension, without the dot                              |
//! | `[P]`           | the name of the folder the file is in                       |
//! | `[C]`           | a counter from 1, zero-padded to the widest number          |
//! | `[C10+5:3]`     | start at 10, step 5, three digits; every part is optional   |
//! | `[[`            | a `[`                                                       |

use std::path::Path;

use crate::entry::split_name;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Case {
    #[default]
    Keep,
    Lower,
    Upper,
    /// First letter of each word in capitals, the rest small; the extension is left.
    Title,
}

impl Case {
    pub fn label(self) -> &'static str {
        match self {
            Case::Keep => "unchanged",
            Case::Lower => "lower case",
            Case::Upper => "UPPER CASE",
            Case::Title => "Title Case",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Case::Keep => Case::Lower,
            Case::Lower => Case::Upper,
            Case::Upper => Case::Title,
            Case::Title => Case::Keep,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Case::Keep => Case::Title,
            Case::Lower => Case::Keep,
            Case::Upper => Case::Lower,
            Case::Title => Case::Upper,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mask {
    pub name: String,
    pub ext: String,
    pub find: String,
    pub replace: String,
    pub case: Case,
}

impl Default for Mask {
    /// Changes nothing: every name comes out as it went in.
    fn default() -> Self {
        Self {
            name: "[N]".into(),
            ext: "[E]".into(),
            find: String::new(),
            replace: String::new(),
            case: Case::Keep,
        }
    }
}

enum Part {
    Text(String),
    Name(usize, Option<usize>),
    Ext,
    Parent,
    Counter {
        start: Option<u64>,
        step: Option<u64>,
        width: Option<usize>,
    },
}

const MAX_WIDTH: usize = 20;

fn parse(mask: &str) -> Result<Vec<Part>, String> {
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut rest = mask;
    while let Some(i) = rest.find('[') {
        text.push_str(&rest[..i]);
        rest = &rest[i + 1..];
        if let Some(r) = rest.strip_prefix('[') {
            text.push('[');
            rest = r;
            continue;
        }
        let end = rest
            .find(']')
            .ok_or_else(|| format!("[{rest} has no closing ]"))?;
        let token = &rest[..end];
        rest = &rest[end + 1..];
        if !text.is_empty() {
            parts.push(Part::Text(std::mem::take(&mut text)));
        }
        parts.push(token_part(token)?);
    }
    text.push_str(rest);
    if !text.is_empty() {
        parts.push(Part::Text(text));
    }
    Ok(parts)
}

fn token_part(token: &str) -> Result<Part, String> {
    let bad = || format!("[{token}] is not something a mask knows");
    let number = |s: &str| -> Result<Option<u64>, String> {
        if s.is_empty() {
            Ok(None)
        } else {
            s.parse().map(Some).map_err(|_| bad())
        }
    };
    match token.as_bytes().first() {
        Some(b'E') if token == "E" => Ok(Part::Ext),
        Some(b'P') if token == "P" => Ok(Part::Parent),
        Some(b'N') => {
            let range = &token[1..];
            if range.is_empty() {
                return Ok(Part::Name(1, None));
            }
            let (from, to) = match range.split_once('-') {
                Some((a, b)) => (number(a)?.unwrap_or(1), number(b)?),
                None => {
                    let n = number(range)?.ok_or_else(bad)?;
                    (n, Some(n))
                }
            };
            if from == 0 || to.is_some_and(|t| t < from) {
                return Err(format!("[{token}]: characters count from 1, first to last"));
            }
            Ok(Part::Name(from as usize, to.map(|t| t as usize)))
        }
        Some(b'C') => {
            let spec = &token[1..];
            let (spec, width) = match spec.split_once(':') {
                Some((s, w)) => (s, number(w)?.map(|w| w as usize)),
                None => (spec, None),
            };
            if width.is_some_and(|w| w > MAX_WIDTH) {
                return Err(format!("[{token}]: at most {MAX_WIDTH} digits"));
            }
            let (start, step) = match spec.split_once('+') {
                Some((s, st)) => (number(s)?, number(st)?),
                None => (number(spec)?, None),
            };
            Ok(Part::Counter { start, step, width })
        }
        _ => Err(bad()),
    }
}

fn expand(
    parts: &[Part],
    stem: &str,
    ext: &str,
    parent: &str,
    i: u64,
    auto_width: &dyn Fn(u64, u64) -> usize,
) -> Result<String, String> {
    let mut out = String::new();
    for p in parts {
        match p {
            Part::Text(t) => out.push_str(t),
            Part::Ext => out.push_str(ext),
            Part::Parent => out.push_str(parent),
            Part::Name(from, to) => {
                let take = to.map(|t| t + 1 - from).unwrap_or(usize::MAX);
                out.extend(stem.chars().skip(from - 1).take(take));
            }
            Part::Counter { start, step, width } => {
                let (start, step) = (start.unwrap_or(1), step.unwrap_or(1));
                let n = i
                    .checked_mul(step)
                    .and_then(|d| d.checked_add(start))
                    .ok_or("the counter runs past what a number can hold")?;
                let w = width.unwrap_or_else(|| auto_width(start, step));
                out.push_str(&format!("{n:0w$}"));
            }
        }
    }
    Ok(out)
}

fn title(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut start = true;
    for c in s.chars() {
        if c.is_alphanumeric() {
            if start {
                out.extend(c.to_uppercase());
            } else {
                out.extend(c.to_lowercase());
            }
            start = false;
        } else {
            out.push(c);
            start = true;
        }
    }
    out
}

impl Mask {
    /// The new name for each path, in order. An error names the first thing wrong
    /// with the masks; whether the names themselves are allowed is the plan's job.
    pub fn apply(&self, paths: &[impl AsRef<Path>]) -> Result<Vec<String>, String> {
        let name = parse(&self.name)?;
        let ext = parse(&self.ext)?;
        let count = paths.len() as u64;
        // Wide enough for the last number, so the names sort in counter order.
        let auto_width = |start: u64, step: u64| {
            let last = start.saturating_add(step.saturating_mul(count.saturating_sub(1)));
            last.to_string().len()
        };
        let mut out = Vec::with_capacity(paths.len());
        for (i, p) in paths.iter().enumerate() {
            let p = p.as_ref();
            let file = p.file_name().unwrap_or_default().to_string_lossy();
            let (stem, old_ext) = split_name(&file);
            let parent = p
                .parent()
                .and_then(Path::file_name)
                .unwrap_or_default()
                .to_string_lossy();
            let i = i as u64;
            let n = expand(&name, stem, old_ext, &parent, i, &auto_width)?;
            let e = expand(&ext, stem, old_ext, &parent, i, &auto_width)?;
            let mut whole = if e.is_empty() { n } else { format!("{n}.{e}") };
            if !self.find.is_empty() {
                whole = whole.replace(&self.find, &self.replace);
            }
            whole = match self.case {
                Case::Keep => whole,
                Case::Lower => whole.to_lowercase(),
                Case::Upper => whole.to_uppercase(),
                Case::Title => {
                    let (s, x) = split_name(&whole);
                    if x.is_empty() {
                        title(s)
                    } else {
                        format!("{}.{x}", title(s))
                    }
                }
            };
            out.push(whole);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names
            .iter()
            .map(|n| Path::new("/photos/trip").join(n))
            .collect()
    }

    fn run(m: &Mask, names: &[&str]) -> Vec<String> {
        m.apply(&paths(names)).unwrap()
    }

    fn mask(name: &str, ext: &str) -> Mask {
        Mask {
            name: name.into(),
            ext: ext.into(),
            ..Mask::default()
        }
    }

    #[test]
    fn the_default_changes_nothing() {
        let names = ["a.txt", "archive.tar.gz", ".bashrc", "README", "dir.d"];
        assert_eq!(run(&Mask::default(), &names), names);
    }

    #[test]
    fn a_counter_is_padded_to_the_widest_number() {
        let names: Vec<String> = (0..12).map(|i| format!("IMG_{i}.JPG")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let out = run(&mask("[P]_[C]", "[E]"), &names);
        assert_eq!(out[0], "trip_01.JPG");
        assert_eq!(out[11], "trip_12.JPG");
    }

    #[test]
    fn a_counter_takes_a_start_a_step_and_a_width() {
        let out = run(&mask("[C10+5:3]", "[E]"), &["a.x", "b.x", "c.x"]);
        assert_eq!(out, ["010.x", "015.x", "020.x"]);
        let out = run(&mask("[C:2]-[N]", ""), &["a.x"]);
        assert_eq!(out, ["01-a"], "an empty extension mask drops the dot");
        let out = run(&mask("[C0]", "[E]"), &["a.x", "b.x"]);
        assert_eq!(out, ["0.x", "1.x"]);
    }

    #[test]
    fn character_ranges_count_characters_from_one() {
        let out = run(&mask("[N1-3]", "[E]"), &["báo cáo.txt"]);
        assert_eq!(out, ["báo.txt"], "characters, not bytes");
        assert_eq!(run(&mask("[N5-]", "[E]"), &["báo cáo.txt"]), ["cáo.txt"]);
        assert_eq!(run(&mask("[N2]", "[E]"), &["abc.txt"]), ["b.txt"]);
        assert_eq!(
            run(&mask("[N2-99]", "[E]"), &["abc.txt"]),
            ["bc.txt"],
            "past the end is just the end"
        );
    }

    #[test]
    fn find_and_replace_then_case() {
        let m = Mask {
            find: "_".into(),
            replace: " ".into(),
            case: Case::Title,
            ..Mask::default()
        };
        assert_eq!(run(&m, &["my_HOLIDAY_photo.JPG"]), ["My Holiday Photo.JPG"]);
        let m = Mask {
            case: Case::Lower,
            ..Mask::default()
        };
        assert_eq!(run(&m, &["IMG_1.JPG"]), ["img_1.jpg"]);
    }

    #[test]
    fn a_bracket_can_be_written_and_mistakes_are_named() {
        assert_eq!(run(&mask("[[[N]]", "[E]"), &["a.x"]), ["[a].x"]);
        for bad in ["[X]", "[N", "[N0-2]", "[N3-1]", "[Cx]", "[C:99]"] {
            let err = mask(bad, "[E]").apply(&paths(&["a.x"])).unwrap_err();
            assert!(!err.is_empty(), "{bad}");
        }
    }

    #[test]
    fn a_counter_that_overflows_is_an_error_not_a_wrap() {
        let m = mask(&format!("[C{}]", u64::MAX), "");
        assert!(m.apply(&paths(&["a", "b"])).is_err());
    }
}
