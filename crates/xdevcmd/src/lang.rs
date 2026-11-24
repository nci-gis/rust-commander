//! The words on screen in another language, when someone wants them (beta.4, Round 037;
//! the owner, 2026-11-24: a small engine, a bonus, under the user's control).
//!
//! Every text the UI shows is written in the code in English, with a key:
//! `tr!("job.copied", "copied {n} files", n = count)`. That English is what shows unless a
//! language file gives the key another text. A language file is TOML, one key per line,
//! named for its language: `lang/vi.toml`, `lang/ja.toml`, `lang/xx.toml` (any name: a
//! pseudo-language of one's own is fine). xdevcmd looks in two folders, the user's last
//! so it wins: `lang/` beside the program (what a release ships), then `lang/` in the
//! config folder (what the user adds or changes).
//!
//! ```toml
//! "job.copied" = "đã chép {n} tệp"
//! ```
//!
//! A language is named by its file: `vi.toml` is the language `vi`. Two keys may say
//! otherwise (the owner, 2026-11-24), at most 32 characters each, cut if longer:
//! `"$__code"`, the name `[general] language` picks it by (`vietnamese.toml` with
//! `"$__code" = "vi"` is `vi`), and `"$__name"`, the name a user reads ("Tiếng Việt").
//! Without `$__name`, a common ISO 639-1 code has its name ([`iso_name`]), else the code.
//! Several files of one code merge, the config folder's last.
//!
//! What can go wrong, and what happens (the English is never lost):
//! - No file, a key not in it, a value that is not text: the English.
//! - A file that is not valid TOML: the English for every key, and [`init`] says why.
//! - A translation naming a `{param}` the call does not give, or with an unclosed `{`:
//!   the English for that key (a `{n}` left on screen helps nobody).
//! - A translation leaving out a `{param}` the call gives: allowed, it shows less.
//! - `{{` and `}}` are a brace.
//!
//! Docs, code, comments, config keys, session and journal words stay English: they are
//! not text for a user, and old files must read the same in any language.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

/// The longest `$__code` or `$__name`, in characters; longer is cut.
pub const MAX_ID: usize = 32;

/// The names of common ISO 639-1 languages, in their own language, for a file with no
/// `$__name`.
const ISO: &[(&str, &str)] = &[
    ("ar", "العربية"),
    ("cs", "Čeština"),
    ("de", "Deutsch"),
    ("en", "English"),
    ("es", "Español"),
    ("fr", "Français"),
    ("hi", "हिन्दी"),
    ("id", "Bahasa Indonesia"),
    ("it", "Italiano"),
    ("ja", "日本語"),
    ("km", "ភាសាខ្មែរ"),
    ("ko", "한국어"),
    ("lo", "ພາສາລາວ"),
    ("ms", "Bahasa Melayu"),
    ("nl", "Nederlands"),
    ("pl", "Polski"),
    ("pt", "Português"),
    ("ru", "Русский"),
    ("sv", "Svenska"),
    ("th", "ไทย"),
    ("tr", "Türkçe"),
    ("uk", "Українська"),
    ("vi", "Tiếng Việt"),
    ("zh", "中文"),
];

/// A common language's name in itself, by its ISO 639-1 code (case ignored).
pub fn iso_name(code: &str) -> Option<&'static str> {
    ISO.iter()
        .find(|(c, _)| c.eq_ignore_ascii_case(code))
        .map(|(_, n)| *n)
}

/// `s` trimmed and cut to [`MAX_ID`] characters; `None` if nothing is left.
fn id(s: &str) -> Option<String> {
    let s: String = s.trim().chars().take(MAX_ID).collect();
    (!s.is_empty()).then_some(s)
}

/// A language found in the `lang/` folders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language {
    /// What `[general] language` picks it by: `$__code`, else the file's name.
    pub code: String,
    /// What a user reads: `$__name`, else the ISO name of the code, else the code.
    pub name: String,
    /// Its files, in the order they load (the config folder's last).
    pub files: Vec<PathBuf>,
}

/// Every language in `folders`, by code: each `*.toml`, its `$__code` and `$__name` read.
/// A file that does not parse counts under its file name, so loading it can say why.
pub fn languages(folders: &[PathBuf]) -> Vec<Language> {
    let mut out: Vec<Language> = Vec::new();
    for dir in folders {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut files: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "toml"))
            .collect();
        files.sort();
        for file in files {
            let Some(stem) = file.file_stem().and_then(|s| s.to_str()).and_then(id) else {
                continue;
            };
            let table = std::fs::read_to_string(&file)
                .ok()
                .and_then(|t| t.parse::<toml::Table>().ok());
            let meta = |k: &str| {
                table
                    .as_ref()
                    .and_then(|t| t.get(k))
                    .and_then(|v| v.as_str())
                    .and_then(id)
            };
            let code = meta("$__code").unwrap_or(stem);
            let own_name = meta("$__name");
            match out.iter_mut().find(|l| l.code == code) {
                Some(l) => {
                    l.files.push(file);
                    if let Some(n) = own_name {
                        l.name = n;
                    }
                }
                None => out.push(Language {
                    name: own_name
                        .or_else(|| iso_name(&code).map(str::to_string))
                        .unwrap_or_else(|| code.clone()),
                    code,
                    files: vec![file],
                }),
            }
        }
    }
    out.sort_by(|a, b| a.code.cmp(&b.code));
    out
}

/// The texts of one language, by key.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Catalog {
    texts: HashMap<String, String>,
}

impl Catalog {
    /// Read a language file's text. A key is written whole (`"keybar.copy" = …`), dotted
    /// (`keybar.copy = …`) or in a section (`[keybar]` then `copy = …`): all the same. A
    /// value that is not a string is left out.
    pub fn parse(toml_text: &str) -> Result<Catalog, String> {
        let table: toml::Table = toml_text
            .parse()
            .map_err(|e: toml::de::Error| e.to_string())?;
        let mut texts = HashMap::new();
        flatten("", table, &mut texts);
        Ok(Catalog { texts })
    }

    /// `other`'s texts over this one's.
    pub fn merge(&mut self, other: Catalog) {
        self.texts.extend(other.texts);
    }

    pub fn is_empty(&self) -> bool {
        self.texts.is_empty()
    }

    /// The text for `key`: the translation, else `english`.
    pub fn text<'a>(&'a self, key: &str, english: &'a str) -> &'a str {
        self.texts.get(key).map_or(english, String::as_str)
    }

    /// The text for `key` with its `{params}` filled in (`tr!` with params; its first
    /// callers come as texts are given keys, W4): the translation if it can be
    /// filled from `args`, else `english`.
    #[allow(dead_code)] // by tr! with params; its first callers come with W4
    pub fn fill(
        &self,
        key: &str,
        english: &str,
        args: &[(&str, &dyn std::fmt::Display)],
    ) -> String {
        if let Some(t) = self.texts.get(key) {
            if let Some(s) = substitute(t, args) {
                return s;
            }
        }
        substitute(english, args).unwrap_or_else(|| english.to_string())
    }
}

/// A TOML table's texts under dotted keys.
fn flatten(prefix: &str, table: toml::Table, out: &mut HashMap<String, String>) {
    for (k, v) in table {
        let key = if prefix.is_empty() {
            k
        } else {
            format!("{prefix}.{k}")
        };
        match v {
            // `$__code`, `$__name`: about the file, not texts.
            toml::Value::String(_) if key.starts_with("$__") => {}
            toml::Value::String(s) => {
                out.insert(key, s);
            }
            toml::Value::Table(t) => flatten(&key, t, out),
            _ => {}
        }
    }
}

/// `template` with each `{name}` replaced by its argument; `None` if it names one not in
/// `args` or a brace is not closed.
pub fn substitute(template: &str, args: &[(&str, &dyn std::fmt::Display)]) -> Option<String> {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(i) = rest.find(['{', '}']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if let Some(t) = tail.strip_prefix("{{") {
            out.push('{');
            rest = t;
        } else if let Some(t) = tail.strip_prefix("}}") {
            out.push('}');
            rest = t;
        } else {
            // A `{` opens a name; a lone `}` is malformed.
            let t = tail.strip_prefix('{')?;
            let end = t.find('}')?;
            let name = &t[..end];
            let (_, value) = args.iter().find(|(n, _)| *n == name)?;
            out.push_str(&value.to_string());
            rest = &t[end + 1..];
        }
    }
    out.push_str(rest);
    Some(out)
}

/// The language a run asks for: `setting` unless it is `auto` (or empty), else the
/// first of `LC_ALL`, `LC_MESSAGES`, `LANG` that names one (`vi_VN.UTF-8` → `vi`). `None`
/// for English, `C` or `POSIX`.
pub fn wanted(setting: &str, get: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let code = |s: &str| {
        let c = s
            .split(['_', '.', '@', '-'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        (!c.is_empty() && c != "c" && c != "posix" && c != "en").then_some(c)
    };
    let setting = setting.trim();
    if !setting.is_empty() && !setting.eq_ignore_ascii_case("auto") {
        return code(setting)
            .or_else(|| (!setting.eq_ignore_ascii_case("en")).then(|| setting.to_string()));
    }
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|k| get(k))
        .find(|v| !v.trim().is_empty())
        .and_then(|v| code(&v))
}

/// The folders language files are read from, the later winning: `lang/` beside the
/// program, then `lang/` in the config folder.
pub fn folders() -> Vec<PathBuf> {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("lang")));
    let own = xdevcmd_core::dirs::config_dir().map(|d| d.join("lang"));
    beside.into_iter().chain(own).collect()
}

/// The catalogue for `code` from `folders`: each file of that code ([`languages`]), the
/// later over the earlier. A file that does not parse is skipped and named in the error,
/// the others still load.
pub fn load(code: &str, folders: &[PathBuf]) -> (Catalog, Vec<String>) {
    let mut cat = Catalog::default();
    let mut problems = Vec::new();
    let files = languages(folders)
        .into_iter()
        .find(|l| l.code == code)
        .map(|l| l.files)
        .unwrap_or_default();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        match Catalog::parse(&text) {
            Ok(c) => cat.merge(c),
            Err(e) => problems.push(format!("{}: {}", file.display(), first_line(&e))),
        }
    }
    (cat, problems)
}

fn first_line(s: &str) -> &str {
    s.lines().find(|l| !l.trim().is_empty()).unwrap_or(s).trim()
}

static CURRENT: OnceLock<Catalog> = OnceLock::new();
static CHOSEN: OnceLock<Option<Language>> = OnceLock::new();

/// The language in use, if not English: its code, name and files.
pub fn chosen() -> Option<&'static Language> {
    CHOSEN.get().and_then(Option::as_ref)
}

/// Choose the run's language once, at start: `setting` is the config's
/// `[general] language`. Returns what went wrong, to tell the user; the English shows
/// wherever a text is missing. Later calls change nothing.
pub fn init(setting: &str) -> Vec<String> {
    let get = |k: &str| std::env::var(k).ok();
    let Some(code) = wanted(setting, &get) else {
        let _ = CURRENT.set(Catalog::default());
        let _ = CHOSEN.set(None);
        return Vec::new();
    };
    let dirs = folders();
    let (cat, mut problems) = load(&code, &dirs);
    let _ = CHOSEN.set(languages(&dirs).into_iter().find(|l| l.code == code));
    // Asked for by name and found nowhere: say so (auto from the system stays quiet).
    let named = !setting.trim().is_empty() && !setting.trim().eq_ignore_ascii_case("auto");
    if cat.is_empty() && problems.is_empty() && named {
        problems.push(format!(
            "language {code:?}: no lang file names it ({code}.toml or \"$__code\" = \"{code}\") in {}",
            dirs.iter()
                .map(|d| d.display().to_string())
                .collect::<Vec<_>>()
                .join(" or ")
        ));
    }
    let _ = CURRENT.set(cat);
    problems
}

/// The run's catalogue; empty (all English) before [`init`].
pub fn current() -> &'static Catalog {
    CURRENT.get_or_init(Catalog::default)
}

/// The text for `key` in the run's language, else `english`.
pub fn text(key: &str, english: &'static str) -> Cow<'static, str> {
    Cow::Borrowed(current().text(key, english))
}

/// A text kept in a table (the key bar, the menu bar): its key and its English, read in
/// the run's language when drawn. `Text::new("key", "English")`, found by `xtask lang`
/// as `tr!` is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Text {
    pub key: &'static str,
    pub en: &'static str,
}

impl Text {
    pub const fn new(key: &'static str, en: &'static str) -> Text {
        Text { key, en }
    }

    /// In the run's language.
    pub fn get(&self) -> Cow<'static, str> {
        text(self.key, self.en)
    }
}

/// `tr!("key", "English")` → the text; `tr!("key", "English {n}", n = value)` → the text
/// with `{n}` filled in. The English is what shows without a translation.
#[macro_export]
macro_rules! tr {
    ($key:literal, $english:literal) => {
        $crate::lang::text($key, $english)
    };
    ($key:literal, $english:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::lang::current().fill(
            $key,
            $english,
            &[$((stringify!($name), &$value as &dyn ::std::fmt::Display)),+],
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat(toml: &str) -> Catalog {
        Catalog::parse(toml).unwrap()
    }

    #[test]
    fn a_key_shows_its_translation_or_the_english() {
        let c = cat(r#""menu.file" = "Tệp""#);
        assert_eq!(c.text("menu.file", "File"), "Tệp");
        assert_eq!(c.text("menu.view", "View"), "View");
        assert_eq!(Catalog::default().text("menu.file", "File"), "File");
    }

    #[test]
    fn a_key_may_be_written_whole_dotted_or_in_a_section() {
        let c = cat("\"menu.file\" = \"A\"\nkeybar.copy = \"B\"\n[status]\ndone = \"C\"\n");
        assert_eq!(c.text("menu.file", ""), "A");
        assert_eq!(c.text("keybar.copy", ""), "B");
        assert_eq!(c.text("status.done", ""), "C");
    }

    #[test]
    fn a_file_names_its_language_by_its_own_keys_else_its_file_name() {
        let d = std::env::temp_dir().join(format!("xdevcmd-lang-id-{}", std::process::id()));
        let (shipped, own) = (d.join("shipped"), d.join("own"));
        std::fs::create_dir_all(&shipped).unwrap();
        std::fs::create_dir_all(&own).unwrap();
        std::fs::write(shipped.join("vi.toml"), "\"a\" = \"một\"\n").unwrap();
        std::fs::write(shipped.join("ja.toml"), "\"a\" = \"一\"\n").unwrap();
        // Another file of code vi, with a name of its own (cut to 32 characters).
        let long = "Tiếng Việt của tôi, rất dài, dài hơn ba mươi hai";
        std::fs::write(
            own.join("mine.toml"),
            format!("\"$__code\" = \"vi\"\n\"$__name\" = \"{long}\"\n\"b\" = \"hai\"\n"),
        )
        .unwrap();
        std::fs::write(own.join("xx.toml"), "\"a\" = \"[ońe]\"\n").unwrap();
        let langs = languages(&[shipped.clone(), own.clone()]);
        let got: Vec<(&str, &str, usize)> = langs
            .iter()
            .map(|l| (l.code.as_str(), l.name.as_str(), l.files.len()))
            .collect();
        let cut: String = long.chars().take(MAX_ID).collect();
        assert_eq!(
            got,
            [
                ("ja", "日本語", 1),
                ("vi", cut.as_str(), 2),
                ("xx", "xx", 1)
            ]
        );
        let (c, problems) = load("vi", &[shipped, own]);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!((c.text("a", "one"), c.text("b", "two")), ("một", "hai"));
        assert_eq!(c.text("$__name", "-"), "-", "not a text");
        assert_eq!(iso_name("VI"), Some("Tiếng Việt"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn params_fill_the_translation_and_a_bad_one_falls_back_to_the_english() {
        let n = 3;
        let args: [(&str, &dyn std::fmt::Display); 2] = [("n", &n), ("dir", &"b")];
        let c = cat(r#"
            "ok" = "đã chép {n} tệp vào {dir}"
            "fewer" = "đã chép xong"
            "unknown" = "đã chép {count} tệp"
            "open" = "đã chép {n tệp"
            "lone" = "đã chép } tệp"
            "braces" = "{{{n}}} tệp"
        "#);
        let en = "copied {n} files to {dir}";
        assert_eq!(c.fill("ok", en, &args), "đã chép 3 tệp vào b");
        // Leaving a param out is allowed: it shows less.
        assert_eq!(c.fill("fewer", en, &args), "đã chép xong");
        // A param the call does not give, or a brace not closed: the English.
        assert_eq!(c.fill("unknown", en, &args), "copied 3 files to b");
        assert_eq!(c.fill("open", en, &args), "copied 3 files to b");
        assert_eq!(c.fill("lone", en, &args), "copied 3 files to b");
        assert_eq!(c.fill("braces", en, &args), "{3} tệp");
        assert_eq!(c.fill("missing", en, &args), "copied 3 files to b");
    }

    #[test]
    fn the_language_comes_from_the_setting_else_the_system() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        let none = env(&[]);
        assert_eq!(wanted("vi", &none).as_deref(), Some("vi"));
        assert_eq!(wanted("ja_JP.UTF-8", &none).as_deref(), Some("ja"));
        assert_eq!(wanted("en", &none), None);
        assert_eq!(wanted("auto", &none), None);
        assert_eq!(
            wanted("", &env(&[("LANG", "vi_VN.UTF-8")])).as_deref(),
            Some("vi")
        );
        assert_eq!(
            wanted(
                "auto",
                &env(&[("LC_ALL", "ja_JP.UTF-8"), ("LANG", "vi_VN")])
            )
            .as_deref(),
            Some("ja")
        );
        assert_eq!(wanted("auto", &env(&[("LANG", "C.UTF-8")])), None);
        assert_eq!(
            wanted("auto", &env(&[("LC_ALL", ""), ("LANG", "POSIX")])),
            None
        );
        // A name of one's own: a pseudo-language.
        assert_eq!(wanted("xx", &none).as_deref(), Some("xx"));
    }

    #[test]
    fn the_users_file_wins_and_a_broken_file_is_named_not_fatal() {
        let d = std::env::temp_dir().join(format!("xdevcmd-lang-{}", std::process::id()));
        let (shipped, own) = (d.join("shipped"), d.join("own"));
        std::fs::create_dir_all(&shipped).unwrap();
        std::fs::create_dir_all(&own).unwrap();
        std::fs::write(
            shipped.join("vi.toml"),
            "\"a\" = \"một\"\n\"b\" = \"hai\"\n",
        )
        .unwrap();
        std::fs::write(own.join("vi.toml"), "\"b\" = \"HAI\"\n").unwrap();
        let (c, problems) = load("vi", &[shipped.clone(), own.clone()]);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!((c.text("a", "one"), c.text("b", "two")), ("một", "HAI"));

        std::fs::write(own.join("vi.toml"), "\"b\" = \n").unwrap();
        let (c, problems) = load("vi", &[shipped.clone(), own.clone()]);
        assert_eq!(c.text("a", "one"), "một", "the other file still loads");
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("vi.toml"), "{problems:?}");

        // No file: English, nothing to say.
        let (c, problems) = load("ja", &[shipped, own]);
        assert!(c.is_empty() && problems.is_empty());
        // A code names a language found, never a path.
        let (c, problems) = load("../vi", std::slice::from_ref(&d));
        assert!(c.is_empty() && problems.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }
}
