//! Syntax colouring for the viewer: which language a file is in, a small lexer per
//! family of languages, and the checkpoints that let a line be coloured without reading
//! the file from its start.
//!
//! Not a parser. Each line is lexed on its own, from a [`State`] that says what the line
//! starts inside of (a block comment, a multi-line string, a fenced block), and gives the
//! state the next line starts in. The state is a few bytes, so keeping one every
//! hundred lines of a big file costs nothing, and a line anywhere is coloured
//! by lexing at most that many lines before it.
//!
//! Why our own and not syntect or tree-sitter: Round 028 measured both against the
//! 7 MB budget; syntect's regex engine alone is 2 MB, tree-sitter is more with two
//! languages and C on every target.

mod lex;

pub use lex::lex_line;

/// The languages coloured. Anything else is drawn plain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    Rust,
    Python,
    /// JavaScript and TypeScript, one table.
    JavaScript,
    Json,
    Toml,
    Yaml,
    Markdown,
    /// sh, bash, zsh; also Dockerfiles and Makefiles, near enough.
    Shell,
    /// C and C++.
    C,
    Go,
    Java,
    Kotlin,
    CSharp,
    /// HTML and XML.
    Markup,
    Css,
    Diff,
    Ini,
    Sql,
    Log,
}

impl Lang {
    /// The name shown on the viewer's title.
    pub fn name(self) -> &'static str {
        match self {
            Lang::Rust => "Rust",
            Lang::Python => "Python",
            Lang::JavaScript => "JavaScript",
            Lang::Json => "JSON",
            Lang::Toml => "TOML",
            Lang::Yaml => "YAML",
            Lang::Markdown => "Markdown",
            Lang::Shell => "shell",
            Lang::C => "C/C++",
            Lang::Go => "Go",
            Lang::Java => "Java",
            Lang::Kotlin => "Kotlin",
            Lang::CSharp => "C#",
            Lang::Markup => "HTML/XML",
            Lang::Css => "CSS",
            Lang::Diff => "diff",
            Lang::Ini => "INI",
            Lang::Sql => "SQL",
            Lang::Log => "log",
        }
    }
}

/// What a piece of a line is. The viewer gives each a colour from the theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Comment,
    String,
    /// Numbers, and constants such as `true`, `None`, `null`.
    Number,
    Keyword,
    Type,
    Function,
    /// Attributes, decorators, the preprocessor, section headers, anchors.
    Meta,
    /// Keys in JSON, TOML, YAML and INI; attribute names in markup; variables in shell.
    Key,
    /// Tag names in markup.
    Tag,
    Heading,
    /// A diff's added and removed lines.
    Added,
    Removed,
    /// A log's error and warning levels.
    Error,
    Warning,
}

/// A coloured piece of a line, in bytes of the line's text. Always on character
/// boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub kind: Kind,
}

/// What a line starts inside of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum State {
    #[default]
    Normal,
    /// A block comment, this deep (Rust's nest).
    Comment(u8),
    /// A string that goes on past the line's end: its quote, and whether tripled.
    Str { quote: u8, triple: bool },
    /// Rust's `r#"…"#`, with this many `#`.
    RawStr(u8),
    /// A Markdown fence: its character (`` ` `` or `~`) and length.
    Fence { ch: u8, len: u8 },
    /// Markup: inside `<!-- … -->`.
    MarkupComment,
    /// Markup: inside a tag, between its name and its `>`.
    InTag,
}

/// The language of a file, from its name, else from its first line. `None`: plain.
pub fn detect(name: &str, first_line: &str) -> Option<Lang> {
    by_name(name).or_else(|| by_first_line(first_line))
}

fn by_name(name: &str) -> Option<Lang> {
    let lower = name.to_ascii_lowercase();
    // Whole names first: they have no extension, or one that says something else.
    let whole = match lower.as_str() {
        "makefile" | "gnumakefile" | "dockerfile" | "containerfile" | "pkgbuild" | "apkbuild"
        | ".bashrc" | ".bash_profile" | ".bash_aliases" | ".bash_logout" | ".profile"
        | ".zshrc" | ".zprofile" | ".zshenv" | ".envrc" => Some(Lang::Shell),
        "cargo.lock" | "pipfile" | "poetry.lock" | "uv.lock" => Some(Lang::Toml),
        ".gitconfig" | ".gitmodules" | ".editorconfig" | ".npmrc" | ".pylintrc" | "setup.cfg"
        | ".flake8" => Some(Lang::Ini),
        "cmakelists.txt" => Some(Lang::Shell),
        _ => None,
    };
    if whole.is_some() {
        return whole;
    }
    if lower.starts_with("dockerfile.") || lower.ends_with(".dockerfile") {
        return Some(Lang::Shell);
    }
    let ext = lower.rsplit_once('.').map(|(_, e)| e)?;
    Some(match ext {
        "rs" => Lang::Rust,
        "py" | "pyw" | "pyi" => Lang::Python,
        "js" | "mjs" | "cjs" | "jsx" | "ts" | "mts" | "cts" | "tsx" => Lang::JavaScript,
        "json" | "jsonc" | "json5" | "geojson" | "ipynb" => Lang::Json,
        "toml" => Lang::Toml,
        "yaml" | "yml" => Lang::Yaml,
        "md" | "markdown" | "mdown" | "mkd" => Lang::Markdown,
        "sh" | "bash" | "zsh" | "ksh" | "ash" | "dash" | "mk" | "mak" | "ebuild" => Lang::Shell,
        "c" | "h" | "cc" | "cpp" | "cxx" | "c++" | "hh" | "hpp" | "hxx" | "h++" | "ino" | "m"
        | "mm" => Lang::C,
        "go" => Lang::Go,
        "java" | "groovy" | "gradle" | "scala" => Lang::Java,
        "kt" | "kts" => Lang::Kotlin,
        "cs" => Lang::CSharp,
        "html" | "htm" | "xhtml" | "xml" | "xsd" | "xsl" | "xslt" | "svg" | "plist" | "csproj"
        | "vcxproj" | "props" | "targets" | "pom" | "vue" => Lang::Markup,
        "css" | "scss" | "less" => Lang::Css,
        "diff" | "patch" => Lang::Diff,
        "ini" | "cfg" | "conf" | "properties" | "desktop" | "service" | "inf" | "reg" => Lang::Ini,
        "sql" => Lang::Sql,
        "log" => Lang::Log,
        _ => return None,
    })
}

fn by_first_line(line: &str) -> Option<Lang> {
    let l = line.trim_start_matches('\u{feff}');
    if let Some(rest) = l.strip_prefix("#!") {
        // `#!/usr/bin/env -S python3 -u`: the program is the first word after env and
        // its options.
        let mut words = rest.split_whitespace();
        let mut prog = words.next().unwrap_or("");
        if prog.ends_with("/env") || prog == "env" {
            prog = words.find(|w| !w.starts_with('-')).unwrap_or("");
        }
        let prog = prog.rsplit('/').next().unwrap_or(prog);
        let prog = prog.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
        return match prog {
            "sh" | "bash" | "zsh" | "ksh" | "dash" | "ash" | "busybox" => Some(Lang::Shell),
            "python" | "pypy" => Some(Lang::Python),
            "node" | "deno" | "bun" | "ts-node" => Some(Lang::JavaScript),
            _ => None,
        };
    }
    let lower = l.trim_start().to_ascii_lowercase();
    if lower.starts_with("<?xml")
        || lower.starts_with("<!doctype html")
        || lower.starts_with("<html")
    {
        return Some(Lang::Markup);
    }
    if lower.starts_with("diff --git ") || lower.starts_with("--- a/") {
        return Some(Lang::Diff);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn by_extension_whatever_its_case() {
        assert_eq!(detect("main.rs", ""), Some(Lang::Rust));
        assert_eq!(detect("APP.PY", ""), Some(Lang::Python));
        assert_eq!(detect("index.tsx", ""), Some(Lang::JavaScript));
        assert_eq!(detect("a.b.json", ""), Some(Lang::Json));
        assert_eq!(detect("Cargo.toml", ""), Some(Lang::Toml));
        assert_eq!(detect("ci.yml", ""), Some(Lang::Yaml));
        assert_eq!(detect("README.md", ""), Some(Lang::Markdown));
        assert_eq!(detect("x.hpp", ""), Some(Lang::C));
        assert_eq!(detect("fix.patch", ""), Some(Lang::Diff));
        assert_eq!(detect("app.log", ""), Some(Lang::Log));
        assert_eq!(detect("q.SQL", ""), Some(Lang::Sql));
        assert_eq!(detect("notes.txt", ""), None);
        assert_eq!(detect("noext", ""), None);
        assert_eq!(detect(".hidden", ""), None);
    }

    #[test]
    fn by_whole_name_before_extension() {
        assert_eq!(detect("Makefile", ""), Some(Lang::Shell));
        assert_eq!(detect("Dockerfile", ""), Some(Lang::Shell));
        assert_eq!(detect("Dockerfile.dev", ""), Some(Lang::Shell));
        assert_eq!(detect("Cargo.lock", ""), Some(Lang::Toml));
        assert_eq!(detect(".gitconfig", ""), Some(Lang::Ini));
        assert_eq!(detect(".bashrc", ""), Some(Lang::Shell));
        assert_eq!(detect("CMakeLists.txt", ""), Some(Lang::Shell));
    }

    #[test]
    fn by_shebang_and_first_line_when_the_name_says_nothing() {
        assert_eq!(detect("run", "#!/bin/sh"), Some(Lang::Shell));
        assert_eq!(detect("run", "#!/usr/bin/env bash"), Some(Lang::Shell));
        assert_eq!(detect("tool", "#!/usr/bin/env python3"), Some(Lang::Python));
        assert_eq!(
            detect("tool", "#!/usr/bin/env -S python3.12 -u"),
            Some(Lang::Python)
        );
        assert_eq!(
            detect("tool", "#!/usr/local/bin/node"),
            Some(Lang::JavaScript)
        );
        assert_eq!(detect("tool", "#!/usr/bin/perl"), None);
        assert_eq!(
            detect("data", "<?xml version=\"1.0\"?>"),
            Some(Lang::Markup)
        );
        assert_eq!(detect("page", "<!DOCTYPE html>"), Some(Lang::Markup));
        assert_eq!(detect("change", "diff --git a/x b/x"), Some(Lang::Diff));
        assert_eq!(detect("change", "hello"), None);
        // The name wins over the first line.
        assert_eq!(detect("x.py", "#!/bin/sh"), Some(Lang::Python));
    }
}
