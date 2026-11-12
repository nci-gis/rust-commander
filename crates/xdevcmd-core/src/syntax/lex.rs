//! The lexers: one for the C-shaped and script languages, driven by a table per
//! language ([`Spec`]), and one each for the line-shaped formats (Markdown, diff, logs,
//! INI, TOML, YAML) and markup.
//!
//! Each works on one line from a [`State`] and returns the state the next line starts
//! in. Spans start and end only at ASCII bytes or at the line's end, so they are always
//! on character boundaries.

use super::{Kind, Lang, Span, State};

/// Lex `line` from `state`: its coloured pieces into `spans` (when given), and the state
/// the next line starts in. Without `spans` only the state is worked out, which is what
/// a scan of a whole file needs.
pub fn lex_line(lang: Lang, state: State, line: &str, spans: Option<&mut Vec<Span>>) -> State {
    let mut out = Out { spans };
    let b = line.as_bytes();
    match lang {
        Lang::Markdown => markdown(state, b, &mut out),
        Lang::Diff => {
            diff(b, &mut out);
            State::Normal
        }
        Lang::Log => {
            log(b, &mut out);
            State::Normal
        }
        Lang::Ini => {
            ini(b, &mut out);
            State::Normal
        }
        Lang::Toml => toml(state, b, &mut out),
        Lang::Yaml => yaml(b, &mut out),
        Lang::Markup => markup(state, b, &mut out),
        _ => code(spec(lang), state, b, 0, &mut out),
    }
}

struct Out<'a> {
    spans: Option<&'a mut Vec<Span>>,
}

impl Out<'_> {
    fn push(&mut self, start: usize, end: usize, kind: Kind) {
        if end > start {
            if let Some(v) = self.spans.as_mut() {
                v.push(Span { start, end, kind });
            }
        }
    }
}

/// How many bytes the character starting with `c` takes.
fn char_len(c: u8) -> usize {
    match c {
        0xF0.. => 4,
        0xE0.. => 3,
        0xC0.. => 2,
        _ => 1,
    }
}

fn ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c >= 0x80
}

fn ident_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c >= 0x80
}

fn skip_blank(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    i
}

// ── the table-driven lexer ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Meta {
    None,
    /// `#include`: a `#` that starts the line (C, C#).
    Hash,
    /// `@decorator`, `@Override`, `@media`.
    At,
    /// `#[derive(…)]`, `#![…]`.
    RustAttr,
    /// YAML's `&anchor`, `*alias`, `!tag`.
    Anchor,
}

struct Spec {
    line: &'static [&'static str],
    block: Option<(&'static str, &'static str)>,
    nest: bool,
    quotes: &'static [u8],
    /// Quotes whose strings go on past the line's end.
    multi: &'static [u8],
    /// Quotes with no escapes inside (shell's `'`, Go's `` ` ``).
    raw: &'static [u8],
    /// `"""` and `'''`.
    triple: bool,
    /// Rust's raw strings, `b"…"`, chars against lifetimes, `name!`.
    rust: bool,
    /// Python's `f"…"`, `rb'…'`.
    prefixes: bool,
    /// `#` starts a comment only at a word's start (shell: `a#b` is a word).
    hash_word: bool,
    meta: Meta,
    /// Shell's `$name`, `${…}`.
    sigil: bool,
    /// `Capitalised` names are types, `ALL_CAPS` ones constants.
    caps: bool,
    /// Names may hold `-` (CSS).
    dash: bool,
    /// Keywords whatever their case (SQL).
    fold: bool,
    /// A string followed by `:` is a key (JSON).
    keys: bool,
    /// A name followed by `:` is a property (CSS).
    css: bool,
    keywords: &'static [&'static str],
    types: &'static [&'static str],
    consts: &'static [&'static str],
    /// Keywords after which a name is a function's.
    defs: &'static [&'static str],
    /// Keywords after which a name is a type's.
    type_defs: &'static [&'static str],
}

const BASE: Spec = Spec {
    line: &[],
    block: None,
    nest: false,
    quotes: &[],
    multi: &[],
    raw: &[],
    triple: false,
    rust: false,
    prefixes: false,
    hash_word: false,
    meta: Meta::None,
    sigil: false,
    caps: false,
    dash: false,
    fold: false,
    keys: false,
    css: false,
    keywords: &[],
    types: &[],
    consts: &[],
    defs: &[],
    type_defs: &[],
};

const C_COMMENTS: Spec = Spec {
    line: &["//"],
    block: Some(("/*", "*/")),
    quotes: b"\"'",
    ..BASE
};

static RUST: Spec = Spec {
    nest: true,
    multi: b"\"",
    rust: true,
    meta: Meta::RustAttr,
    caps: true,
    keywords: &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut",
        "pub", "ref", "return", "self", "static", "struct", "super", "trait", "type", "union",
        "unsafe", "use", "where", "while", "yield",
    ],
    types: &[
        "bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32",
        "i64", "i128", "isize", "f32", "f64",
    ],
    consts: &["true", "false"],
    defs: &["fn"],
    type_defs: &["struct", "enum", "trait", "type", "union"],
    ..C_COMMENTS
};

static PYTHON: Spec = Spec {
    line: &["#"],
    quotes: b"\"'",
    triple: true,
    prefixes: true,
    meta: Meta::At,
    keywords: &[
        "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
        "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is",
        "lambda", "match", "case", "nonlocal", "not", "or", "pass", "raise", "return", "try",
        "while", "with", "yield", "self",
    ],
    types: &[
        "int",
        "float",
        "str",
        "bytes",
        "bool",
        "list",
        "dict",
        "set",
        "tuple",
        "object",
        "type",
        "complex",
        "frozenset",
        "bytearray",
    ],
    consts: &["True", "False", "None", "NotImplemented", "Ellipsis"],
    defs: &["def"],
    type_defs: &["class"],
    ..BASE
};

static JAVASCRIPT: Spec = Spec {
    quotes: b"\"'`",
    multi: b"`",
    meta: Meta::At,
    caps: true,
    keywords: &[
        "abstract",
        "as",
        "async",
        "await",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "declare",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "export",
        "extends",
        "finally",
        "for",
        "from",
        "function",
        "get",
        "if",
        "implements",
        "import",
        "in",
        "instanceof",
        "interface",
        "keyof",
        "let",
        "namespace",
        "new",
        "of",
        "private",
        "protected",
        "public",
        "readonly",
        "return",
        "satisfies",
        "set",
        "static",
        "super",
        "switch",
        "this",
        "throw",
        "try",
        "type",
        "typeof",
        "var",
        "void",
        "while",
        "with",
        "yield",
    ],
    types: &[
        "string", "number", "boolean", "any", "unknown", "never", "object", "bigint", "symbol",
    ],
    consts: &["true", "false", "null", "undefined", "NaN", "Infinity"],
    defs: &["function"],
    type_defs: &["class", "interface", "type", "enum"],
    ..C_COMMENTS
};

static JSON: Spec = Spec {
    quotes: b"\"",
    keys: true,
    consts: &["true", "false", "null"],
    ..C_COMMENTS
};

static SHELL: Spec = Spec {
    line: &["#"],
    quotes: b"\"'",
    multi: b"\"'",
    raw: b"'",
    hash_word: true,
    sigil: true,
    keywords: &[
        "if",
        "then",
        "else",
        "elif",
        "fi",
        "for",
        "while",
        "until",
        "do",
        "done",
        "case",
        "esac",
        "in",
        "function",
        "select",
        "return",
        "local",
        "export",
        "readonly",
        "declare",
        "typeset",
        "break",
        "continue",
        "exit",
        "set",
        "unset",
        "shift",
        "source",
        "alias",
        "eval",
        "exec",
        "trap",
        "time",
        // Dockerfiles, read as shell
        "FROM",
        "RUN",
        "CMD",
        "COPY",
        "ADD",
        "ENV",
        "ARG",
        "WORKDIR",
        "ENTRYPOINT",
        "EXPOSE",
        "USER",
        "VOLUME",
        "LABEL",
        "HEALTHCHECK",
        "SHELL",
        "ONBUILD",
        "STOPSIGNAL",
    ],
    types: &[
        "echo", "printf", "cd", "test", "read", "pwd", "true", "false", "exit", "wait", "kill",
        "getopts", "command", "builtin", "type", "ulimit", "umask",
    ],
    ..BASE
};

static C: Spec = Spec {
    meta: Meta::Hash,
    keywords: &[
        "auto",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "constexpr",
        "continue",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "explicit",
        "extern",
        "for",
        "friend",
        "goto",
        "if",
        "inline",
        "namespace",
        "new",
        "noexcept",
        "operator",
        "private",
        "protected",
        "public",
        "register",
        "return",
        "sizeof",
        "static",
        "static_assert",
        "struct",
        "switch",
        "template",
        "this",
        "throw",
        "try",
        "typedef",
        "typename",
        "union",
        "using",
        "virtual",
        "volatile",
        "while",
        "override",
        "final",
        "co_await",
        "co_return",
        "co_yield",
        "consteval",
        "constinit",
        "decltype",
        "mutable",
    ],
    types: &[
        "void",
        "char",
        "short",
        "int",
        "long",
        "float",
        "double",
        "signed",
        "unsigned",
        "bool",
        "size_t",
        "ssize_t",
        "int8_t",
        "int16_t",
        "int32_t",
        "int64_t",
        "uint8_t",
        "uint16_t",
        "uint32_t",
        "uint64_t",
        "wchar_t",
        "char8_t",
        "char16_t",
        "char32_t",
        "auto",
        "ptrdiff_t",
        "intptr_t",
        "uintptr_t",
    ],
    consts: &["true", "false", "NULL", "nullptr"],
    type_defs: &["struct", "class", "enum", "union", "typedef"],
    ..C_COMMENTS
};

static GO: Spec = Spec {
    quotes: b"\"'`",
    multi: b"`",
    raw: b"`",
    keywords: &[
        "break",
        "case",
        "chan",
        "const",
        "continue",
        "default",
        "defer",
        "else",
        "fallthrough",
        "for",
        "func",
        "go",
        "goto",
        "if",
        "import",
        "interface",
        "map",
        "package",
        "range",
        "return",
        "select",
        "struct",
        "switch",
        "type",
        "var",
    ],
    types: &[
        "bool",
        "byte",
        "complex64",
        "complex128",
        "error",
        "float32",
        "float64",
        "int",
        "int8",
        "int16",
        "int32",
        "int64",
        "rune",
        "string",
        "uint",
        "uint8",
        "uint16",
        "uint32",
        "uint64",
        "uintptr",
        "any",
    ],
    consts: &["true", "false", "nil", "iota"],
    defs: &["func"],
    type_defs: &["type"],
    ..C_COMMENTS
};

static JAVA: Spec = Spec {
    triple: true,
    meta: Meta::At,
    caps: true,
    keywords: &[
        "abstract",
        "assert",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "default",
        "do",
        "else",
        "enum",
        "extends",
        "final",
        "finally",
        "for",
        "goto",
        "if",
        "implements",
        "import",
        "instanceof",
        "interface",
        "native",
        "new",
        "package",
        "permits",
        "private",
        "protected",
        "public",
        "record",
        "return",
        "sealed",
        "static",
        "strictfp",
        "super",
        "switch",
        "synchronized",
        "this",
        "throw",
        "throws",
        "transient",
        "try",
        "var",
        "volatile",
        "while",
        "yield",
        "def",
    ],
    types: &[
        "boolean", "byte", "char", "double", "float", "int", "long", "short", "void",
    ],
    consts: &["true", "false", "null"],
    type_defs: &["class", "interface", "enum", "record"],
    ..C_COMMENTS
};

static KOTLIN: Spec = Spec {
    keywords: &[
        "as",
        "break",
        "class",
        "continue",
        "do",
        "else",
        "for",
        "fun",
        "if",
        "in",
        "interface",
        "is",
        "object",
        "package",
        "return",
        "super",
        "this",
        "throw",
        "try",
        "typealias",
        "val",
        "var",
        "when",
        "while",
        "by",
        "catch",
        "constructor",
        "finally",
        "get",
        "import",
        "init",
        "set",
        "where",
        "abstract",
        "annotation",
        "companion",
        "data",
        "enum",
        "final",
        "inline",
        "inner",
        "internal",
        "lateinit",
        "open",
        "override",
        "private",
        "protected",
        "public",
        "sealed",
        "suspend",
        "vararg",
    ],
    types: &[],
    consts: &["true", "false", "null"],
    defs: &["fun"],
    type_defs: &["class", "interface", "object", "typealias"],
    ..JAVA
};

static CSHARP: Spec = Spec {
    triple: false,
    meta: Meta::Hash,
    keywords: &[
        "abstract",
        "as",
        "async",
        "await",
        "base",
        "break",
        "case",
        "catch",
        "checked",
        "class",
        "const",
        "continue",
        "default",
        "delegate",
        "do",
        "else",
        "enum",
        "event",
        "explicit",
        "extern",
        "finally",
        "fixed",
        "for",
        "foreach",
        "get",
        "goto",
        "if",
        "implicit",
        "in",
        "init",
        "interface",
        "internal",
        "is",
        "lock",
        "namespace",
        "new",
        "operator",
        "out",
        "override",
        "params",
        "private",
        "protected",
        "public",
        "readonly",
        "record",
        "ref",
        "return",
        "sealed",
        "set",
        "sizeof",
        "stackalloc",
        "static",
        "struct",
        "switch",
        "this",
        "throw",
        "try",
        "typeof",
        "unchecked",
        "unsafe",
        "using",
        "var",
        "virtual",
        "volatile",
        "when",
        "where",
        "while",
        "yield",
    ],
    types: &[
        "bool", "byte", "char", "decimal", "double", "float", "int", "long", "object", "sbyte",
        "short", "string", "uint", "ulong", "ushort", "void", "dynamic", "nint", "nuint",
    ],
    consts: &["true", "false", "null"],
    type_defs: &["class", "interface", "enum", "struct", "record"],
    ..JAVA
};

static CSS: Spec = Spec {
    line: &[],
    meta: Meta::At,
    dash: true,
    css: true,
    keywords: &["important"],
    ..C_COMMENTS
};

static SQL: Spec = Spec {
    line: &["--"],
    multi: b"'",
    fold: true,
    keywords: &[
        "add",
        "all",
        "alter",
        "and",
        "as",
        "asc",
        "begin",
        "between",
        "by",
        "case",
        "check",
        "column",
        "commit",
        "constraint",
        "create",
        "cross",
        "database",
        "default",
        "delete",
        "desc",
        "distinct",
        "drop",
        "else",
        "end",
        "exists",
        "foreign",
        "from",
        "full",
        "group",
        "having",
        "if",
        "in",
        "index",
        "inner",
        "insert",
        "into",
        "is",
        "join",
        "key",
        "left",
        "like",
        "limit",
        "not",
        "offset",
        "on",
        "or",
        "order",
        "outer",
        "primary",
        "references",
        "returning",
        "right",
        "rollback",
        "select",
        "set",
        "table",
        "then",
        "transaction",
        "union",
        "unique",
        "update",
        "values",
        "view",
        "when",
        "where",
        "with",
    ],
    types: &[
        "int",
        "integer",
        "bigint",
        "smallint",
        "text",
        "varchar",
        "char",
        "boolean",
        "bool",
        "date",
        "time",
        "timestamp",
        "real",
        "float",
        "double",
        "decimal",
        "numeric",
        "blob",
        "serial",
        "json",
        "jsonb",
        "uuid",
    ],
    consts: &["null", "true", "false"],
    ..C_COMMENTS
};

/// What follows `key =` in TOML.
static TOML_VALUE: Spec = Spec {
    line: &["#"],
    quotes: b"\"'",
    raw: b"'",
    triple: true,
    consts: &["true", "false", "inf", "nan"],
    ..BASE
};

/// What follows `key:` in YAML, or a list's `- `.
static YAML_VALUE: Spec = Spec {
    line: &["#"],
    quotes: b"\"'",
    raw: b"'",
    hash_word: true,
    meta: Meta::Anchor,
    consts: &[
        "true", "false", "null", "yes", "no", "on", "off", "True", "False", "Null", "TRUE",
        "FALSE", "NULL", "~",
    ],
    ..BASE
};

/// What follows `key =` in INI: quoted strings, numbers, booleans; the rest plain.
static INI_VALUE: Spec = Spec {
    quotes: b"\"",
    consts: &["true", "false", "yes", "no", "on", "off"],
    ..BASE
};

fn spec(lang: Lang) -> &'static Spec {
    match lang {
        Lang::Rust => &RUST,
        Lang::Python => &PYTHON,
        Lang::JavaScript => &JAVASCRIPT,
        Lang::Json => &JSON,
        Lang::Shell => &SHELL,
        Lang::C => &C,
        Lang::Go => &GO,
        Lang::Java => &JAVA,
        Lang::Kotlin => &KOTLIN,
        Lang::CSharp => &CSHARP,
        Lang::Css => &CSS,
        Lang::Sql => &SQL,
        // The others have lexers of their own; this is not reached for them.
        _ => &BASE,
    }
}

fn name_end(spec: &Spec, b: &[u8], mut i: usize) -> usize {
    while i < b.len() {
        if ident_char(b[i])
            || (spec.dash && b[i] == b'-' && b.get(i + 1).is_some_and(|&c| ident_char(c)))
        {
            i += 1;
        } else {
            break;
        }
    }
    i
}

/// The end of a block comment from `i`, `depth` deep: where it closes and `Normal`, or
/// the line's end and the depth still open.
fn block_comment(spec: &Spec, b: &[u8], mut i: usize, mut depth: u8) -> (usize, State) {
    let Some((open, close)) = spec.block else {
        return (b.len(), State::Normal);
    };
    while i < b.len() {
        if spec.nest && b[i..].starts_with(open.as_bytes()) {
            depth = depth.saturating_add(1);
            i += open.len();
        } else if b[i..].starts_with(close.as_bytes()) {
            depth = depth.saturating_sub(1);
            i += close.len();
            if depth == 0 {
                return (i, State::Normal);
            }
        } else {
            i += 1;
        }
    }
    (b.len(), State::Comment(depth.max(1)))
}

/// The body of a string from `i`: where it closes (after its quote), or `None` at the
/// line's end.
fn string_body(spec: &Spec, b: &[u8], mut i: usize, quote: u8, triple: bool) -> Option<usize> {
    let escapes = !spec.raw.contains(&quote);
    while i < b.len() {
        if escapes && b[i] == b'\\' {
            i += 2;
            continue;
        }
        if triple {
            if b[i..].starts_with(&[quote; 3]) {
                return Some(i + 3);
            }
        } else if b[i] == quote {
            return Some(i + 1);
        }
        i += 1;
    }
    None
}

/// Rust's raw string body from `i`, closed by `"` and `hashes` `#`.
fn raw_body(b: &[u8], mut i: usize, hashes: u8) -> Option<usize> {
    let h = usize::from(hashes);
    while i < b.len() {
        if b[i] == b'"' && b.len() >= i + 1 + h && b[i + 1..i + 1 + h].iter().all(|&c| c == b'#') {
            return Some(i + 1 + h);
        }
        i += 1;
    }
    None
}

/// A string whose text starts at `start` and whose quote is at `q`. `Ok`: where lexing
/// goes on; `Err`: the string is still open at the line's end, in this state.
fn string_at(spec: &Spec, b: &[u8], start: usize, q: usize, out: &mut Out) -> Result<usize, State> {
    let quote = b[q];
    let triple = spec.triple && b[q..].starts_with(&[quote; 3]);
    let body = q + if triple { 3 } else { 1 };
    match string_body(spec, b, body, quote, triple) {
        Some(end) => {
            let end = end.min(b.len());
            let after = skip_blank(b, end);
            let kind = if spec.keys && b.get(after) == Some(&b':') {
                Kind::Key
            } else {
                Kind::String
            };
            out.push(start, end, kind);
            Ok(end)
        }
        None => {
            out.push(start, b.len(), Kind::String);
            if triple || spec.multi.contains(&quote) {
                Err(State::Str { quote, triple })
            } else {
                Ok(b.len())
            }
        }
    }
}

/// The table-driven lexer, from `from` on the line.
fn code(spec: &Spec, state: State, b: &[u8], from: usize, out: &mut Out) -> State {
    let n = b.len();
    let mut i = from;
    match state {
        State::Comment(depth) => {
            let (end, st) = block_comment(spec, b, i, depth);
            out.push(i, end, Kind::Comment);
            if st != State::Normal {
                return st;
            }
            i = end;
        }
        State::Str { quote, triple } => match string_body(spec, b, i, quote, triple) {
            Some(end) => {
                let end = end.min(n);
                out.push(i, end, Kind::String);
                i = end;
            }
            None => {
                out.push(i, n, Kind::String);
                return state;
            }
        },
        State::RawStr(h) => match raw_body(b, i, h) {
            Some(end) => {
                out.push(i, end, Kind::String);
                i = end;
            }
            None => {
                out.push(i, n, Kind::String);
                return state;
            }
        },
        _ => {}
    }
    let mut first = true;
    let mut next_is: Option<Kind> = None;
    while i < n {
        let c = b[i];
        if c == b' ' || c == b'\t' {
            i += 1;
            continue;
        }
        let word_start = i == 0 || matches!(b[i - 1], b' ' | b'\t' | b';' | b'|' | b'&' | b'(');
        if spec.line.iter().any(|p| b[i..].starts_with(p.as_bytes()))
            && !(spec.hash_word && c == b'#' && !word_start)
        {
            out.push(i, n, Kind::Comment);
            return State::Normal;
        }
        if let Some((open, _)) = spec.block {
            if b[i..].starts_with(open.as_bytes()) {
                let (end, st) = block_comment(spec, b, i + open.len(), 1);
                out.push(i, end, Kind::Comment);
                if st != State::Normal {
                    return st;
                }
                i = end;
                continue;
            }
        }
        let was_first = std::mem::replace(&mut first, false);
        let next = b.get(i + 1).copied();
        match spec.meta {
            Meta::Hash if was_first && c == b'#' => {
                let end = name_end(spec, b, skip_blank(b, i + 1));
                out.push(i, end, Kind::Meta);
                i = end;
                continue;
            }
            Meta::At if c == b'@' && next.is_some_and(ident_start) => {
                let mut end = name_end(spec, b, i + 1);
                while b.get(end) == Some(&b'.') && b.get(end + 1).is_some_and(|&c| ident_start(c)) {
                    end = name_end(spec, b, end + 1);
                }
                out.push(i, end, Kind::Meta);
                i = end;
                continue;
            }
            Meta::RustAttr
                if c == b'#'
                    && (next == Some(b'[')
                        || (next == Some(b'!') && b.get(i + 2) == Some(&b'['))) =>
            {
                let mut depth = 0usize;
                let mut j = i;
                while j < n {
                    match b[j] {
                        b'[' => depth += 1,
                        b']' => {
                            depth -= 1;
                            if depth == 0 {
                                j += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }
                out.push(i, j, Kind::Meta);
                i = j;
                continue;
            }
            Meta::Anchor if word_start && matches!(c, b'&' | b'*' | b'!') => {
                let mut j = i + 1;
                while j < n && !matches!(b[j], b' ' | b'\t' | b',' | b']' | b'}') {
                    j += 1;
                }
                if j > i + 1 {
                    out.push(i, j, Kind::Meta);
                    i = j;
                    continue;
                }
            }
            _ => {}
        }
        // Rust: raw strings (r"…", r#"…"#, br"…") and byte strings (b"…").
        if spec.rust && (c == b'r' || c == b'b') && (i == 0 || !ident_char(b[i - 1])) {
            let mut j = i + usize::from(c == b'b');
            if b.get(j) == Some(&b'r') {
                j += 1;
                let mut h = 0usize;
                while b.get(j + h) == Some(&b'#') {
                    h += 1;
                }
                if b.get(j + h) == Some(&b'"') {
                    let h8 = u8::try_from(h).unwrap_or(u8::MAX);
                    match raw_body(b, j + h + 1, h8) {
                        Some(end) => {
                            out.push(i, end, Kind::String);
                            i = end;
                            continue;
                        }
                        None => {
                            out.push(i, n, Kind::String);
                            return State::RawStr(h8);
                        }
                    }
                }
            } else if c == b'b' && matches!(b.get(j), Some(b'"') | Some(b'\'')) {
                match string_at(spec, b, i, j, out) {
                    Ok(end) => {
                        i = end;
                        continue;
                    }
                    Err(st) => return st,
                }
            }
        }
        // Rust: 'x' and '\n' are characters, 'a a lifetime.
        if spec.rust && c == b'\'' {
            let close = if next == Some(b'\\') {
                (i + 2..n.min(i + 12)).find(|&j| b[j] == b'\'')
            } else {
                next.map(|c| i + 1 + char_len(c))
                    .filter(|&j| b.get(j) == Some(&b'\''))
            };
            match close {
                Some(j) => out.push(i, j + 1, Kind::String),
                None if next.is_some_and(ident_start) => {
                    let end = name_end(spec, b, i + 1);
                    out.push(i, end, Kind::Type);
                    i = end;
                    continue;
                }
                None => {
                    i += 1;
                    continue;
                }
            }
            i = close.map_or(i + 1, |j| j + 1);
            continue;
        }
        if spec.quotes.contains(&c) {
            match string_at(spec, b, i, i, out) {
                Ok(end) => {
                    i = end;
                    continue;
                }
                Err(st) => return st,
            }
        }
        if c.is_ascii_digit() || (c == b'.' && next.is_some_and(|d| d.is_ascii_digit())) {
            let hex = c == b'0' && matches!(next, Some(b'x' | b'X'));
            let mut j = i + 1;
            while j < n {
                let d = b[j];
                if d.is_ascii_alphanumeric()
                    || d == b'_'
                    || d == b'.'
                    || ((d == b'+' || d == b'-') && !hex && matches!(b[j - 1], b'e' | b'E'))
                {
                    j += 1;
                } else {
                    break;
                }
            }
            out.push(i, j, Kind::Number);
            i = j;
            continue;
        }
        if spec.sigil && c == b'$' {
            let end = match next {
                Some(b'{') => (i + 2..n).find(|&j| b[j] == b'}').map_or(n, |j| j + 1),
                Some(d) if ident_start(d) => name_end(spec, b, i + 1),
                Some(d) if d.is_ascii_digit() || b"?@#$!*-".contains(&d) => i + 2,
                _ => i + 1,
            };
            if end > i + 1 {
                out.push(i, end, Kind::Key);
            }
            i = end;
            continue;
        }
        if spec.css && c == b'#' && next.is_some_and(|d| ident_char(d) || d == b'-') {
            let end = name_end(spec, b, i + 1);
            out.push(i, end, Kind::Number);
            i = end;
            continue;
        }
        if ident_start(c) || (spec.dash && c == b'-' && next.is_some_and(ident_start)) {
            let end = name_end(spec, b, i + 1);
            let word = &b[i..end];
            // Python's string prefixes: f"…", rb'…'.
            if spec.prefixes
                && word.len() <= 2
                && word.iter().all(|c| b"rRbBfFuU".contains(c))
                && b.get(end).is_some_and(|q| spec.quotes.contains(q))
            {
                match string_at(spec, b, i, end, out) {
                    Ok(e) => {
                        i = e;
                        continue;
                    }
                    Err(st) => return st,
                }
            }
            let text = std::str::from_utf8(word).unwrap_or("");
            let is = |list: &[&str]| {
                if spec.fold {
                    list.iter().any(|k| k.eq_ignore_ascii_case(text))
                } else {
                    list.contains(&text)
                }
            };
            let after = b.get(end).copied();
            let pending = next_is.take();
            let kind = if is(spec.keywords) {
                if is(spec.defs) {
                    next_is = Some(Kind::Function);
                } else if is(spec.type_defs) {
                    next_is = Some(Kind::Type);
                }
                Some(Kind::Keyword)
            } else if let Some(k) = pending {
                Some(k)
            } else if is(spec.consts) {
                Some(Kind::Number)
            } else if is(spec.types) {
                Some(Kind::Type)
            } else if spec.css
                && after == Some(b':')
                && b.get(end + 1).is_none_or(|d| !d.is_ascii_alphabetic())
            {
                Some(Kind::Key)
            } else if spec.rust && after == Some(b'!') && b.get(end + 1) != Some(&b'=') {
                out.push(i, end + 1, Kind::Function);
                i = end + 1;
                continue;
            } else if after == Some(b'(') {
                Some(Kind::Function)
            } else if spec.caps && word[0].is_ascii_uppercase() {
                if word.len() > 1 && !word.iter().any(|c| c.is_ascii_lowercase()) {
                    Some(Kind::Number)
                } else {
                    Some(Kind::Type)
                }
            } else {
                None
            };
            if let Some(k) = kind {
                out.push(i, end, k);
            }
            i = end;
            continue;
        }
        // Anything else between `fn` and a name means it was not the name (Go's
        // `func (s *S) Run()`).
        next_is = None;
        i = (i + char_len(c)).min(n);
    }
    State::Normal
}

// ── the line-shaped formats ───────────────────────────────────────────────────────────

fn markdown(state: State, b: &[u8], out: &mut Out) -> State {
    let n = b.len();
    let t = skip_blank(b, 0);
    let fence = |at: usize| -> Option<(u8, usize)> {
        let ch = *b.get(at)?;
        if ch != b'`' && ch != b'~' {
            return None;
        }
        let len = b[at..].iter().take_while(|&&c| c == ch).count();
        (len >= 3).then_some((ch, len))
    };
    if let State::Fence { ch, len } = state {
        if let Some((c, l)) = fence(t) {
            if c == ch
                && l >= usize::from(len)
                && b[t + l..].iter().all(|c| c.is_ascii_whitespace())
            {
                out.push(0, n, Kind::Meta);
                return State::Normal;
            }
        }
        out.push(0, n, Kind::String);
        return state;
    }
    if t >= n {
        return State::Normal;
    }
    if let Some((ch, len)) = fence(t) {
        out.push(0, n, Kind::Meta);
        return State::Fence {
            ch,
            len: u8::try_from(len).unwrap_or(u8::MAX),
        };
    }
    let hashes = b[t..].iter().take_while(|&&c| c == b'#').count();
    if (1..=6).contains(&hashes) && matches!(b.get(t + hashes), None | Some(b' ') | Some(b'\t')) {
        out.push(0, n, Kind::Heading);
        return State::Normal;
    }
    if b[t] == b'>' {
        out.push(t, n, Kind::Comment);
        return State::Normal;
    }
    // A rule, or a setext heading's underline.
    if b[t..].len() >= 3
        && b[t..].iter().all(|&c| c == b[t] || c == b' ')
        && matches!(b[t], b'-' | b'*' | b'_' | b'=')
    {
        out.push(t, n, Kind::Meta);
        return State::Normal;
    }
    let mut i = t;
    // A list's marker: -, *, +, or 1. / 1)
    if matches!(b[t], b'-' | b'*' | b'+') && b.get(t + 1) == Some(&b' ') {
        out.push(t, t + 1, Kind::Keyword);
        i = t + 2;
    } else {
        let digits = b[t..].iter().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0
            && matches!(b.get(t + digits), Some(b'.') | Some(b')'))
            && b.get(t + digits + 1) == Some(&b' ')
        {
            out.push(t, t + digits + 1, Kind::Keyword);
            i = t + digits + 2;
        }
    }
    // Inline: `code` and [text](link).
    while i < n {
        match b[i] {
            b'`' => {
                let ticks = b[i..].iter().take_while(|&&c| c == b'`').count();
                let open = i + ticks;
                let close = (open..n).find(|&j| {
                    b[j..].starts_with(&vec![b'`'; ticks]) && b.get(j + ticks) != Some(&b'`')
                });
                match close {
                    Some(j) => {
                        out.push(i, j + ticks, Kind::String);
                        i = j + ticks;
                    }
                    None => i = open,
                }
            }
            b']' if b.get(i + 1) == Some(&b'(') => {
                let end = (i + 2..n).find(|&j| b[j] == b')').map_or(n, |j| j + 1);
                out.push(i + 1, end, Kind::Key);
                i = end;
            }
            c => i += char_len(c),
        }
    }
    State::Normal
}

fn diff(b: &[u8], out: &mut Out) {
    let n = b.len();
    let kind = if b.starts_with(b"diff ")
        || b.starts_with(b"index ")
        || b.starts_with(b"+++ ")
        || b.starts_with(b"--- ")
        || b.starts_with(b"new file")
        || b.starts_with(b"deleted file")
        || b.starts_with(b"similarity")
        || b.starts_with(b"rename ")
    {
        Kind::Heading
    } else if b.starts_with(b"@@") {
        Kind::Meta
    } else if b.starts_with(b"+") {
        Kind::Added
    } else if b.starts_with(b"-") {
        Kind::Removed
    } else {
        return;
    };
    out.push(0, n, kind);
}

fn log(b: &[u8], out: &mut Out) {
    let n = b.len();
    let mut i = 0;
    // A timestamp at the start: digits and the separators dates and times use.
    let t = skip_blank(b, 0);
    let stamp = b[t..]
        .iter()
        .take_while(|c| c.is_ascii_digit() || b"-:/.,TZ+ ".contains(c))
        .count();
    let digits = b[t..t + stamp]
        .iter()
        .filter(|c| c.is_ascii_digit())
        .count();
    if digits >= 6 && b[t..t + stamp].contains(&b':') {
        let mut end = t + stamp;
        while end > t && b[end - 1] == b' ' {
            end -= 1;
        }
        out.push(t, end, Kind::Comment);
        i = end;
    }
    while i < n {
        let c = b[i];
        if c == b'"' {
            let end = (i + 1..n).find(|&j| b[j] == b'"').map_or(n, |j| j + 1);
            out.push(i, end, Kind::String);
            i = end;
            continue;
        }
        if c.is_ascii_alphabetic() && (i == 0 || !ident_char(b[i - 1])) {
            let mut end = i;
            while end < n && ident_char(b[end]) {
                end += 1;
            }
            let w = &b[i..end];
            let colon = b.get(end) == Some(&b':');
            let lower = w.to_ascii_lowercase();
            let upper = w.iter().all(|c| !c.is_ascii_lowercase());
            let level = |words: &[&[u8]]| words.contains(&lower.as_slice()) && (upper || colon);
            let kind = if level(&[
                b"error",
                b"err",
                b"fatal",
                b"crit",
                b"critical",
                b"panic",
                b"severe",
                b"emerg",
                b"alert",
                b"failed",
                b"fail",
            ]) {
                Some(Kind::Error)
            } else if level(&[b"warn", b"warning"]) {
                Some(Kind::Warning)
            } else if level(&[b"info", b"notice"]) {
                Some(Kind::Function)
            } else if level(&[b"debug", b"trace", b"verbose"]) {
                Some(Kind::Comment)
            } else {
                None
            };
            if let Some(k) = kind {
                out.push(i, end, k);
            }
            i = end;
            continue;
        }
        i += char_len(c);
    }
}

/// `[section]` at `t`: its end, after the last `]` (`[[array]]` included).
fn section(b: &[u8], t: usize) -> Option<usize> {
    if b.get(t) != Some(&b'[') {
        return None;
    }
    let close = b.iter().rposition(|&c| c == b']')?;
    (close > t).then_some(close + 1)
}

fn ini(b: &[u8], out: &mut Out) {
    let n = b.len();
    let t = skip_blank(b, 0);
    if t >= n {
        return;
    }
    if b[t] == b';' || b[t] == b'#' {
        out.push(t, n, Kind::Comment);
        return;
    }
    if let Some(end) = section(b, t) {
        out.push(t, end, Kind::Meta);
        return;
    }
    if let Some(eq) = b.iter().position(|&c| c == b'=' || c == b':') {
        let mut k = eq;
        while k > t && (b[k - 1] == b' ' || b[k - 1] == b'\t') {
            k -= 1;
        }
        out.push(t, k, Kind::Key);
        code(&INI_VALUE, State::Normal, b, eq + 1, out);
    }
}

fn toml(state: State, b: &[u8], out: &mut Out) -> State {
    if state != State::Normal {
        return code(&TOML_VALUE, state, b, 0, out);
    }
    let n = b.len();
    let t = skip_blank(b, 0);
    if t >= n {
        return State::Normal;
    }
    if let Some(end) = section(b, t) {
        // `[a] # comment`: the comment after the header is still one.
        let hash = b[t..end].iter().position(|&c| c == b'#').map(|p| t + p);
        let end = hash.map_or(end, |h| {
            b[t..h]
                .iter()
                .rposition(|&c| c == b']')
                .map_or(h, |p| t + p + 1)
        });
        out.push(t, end, Kind::Meta);
        return code(&TOML_VALUE, State::Normal, b, end, out);
    }
    // key = …, the key bare, quoted or dotted.
    let mut j = t;
    while j < n {
        match b[j] {
            b'"' | b'\'' => {
                let q = b[j];
                j = (j + 1..n).find(|&k| b[k] == q).map_or(n, |k| k + 1);
            }
            c if c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.' | b' ' | b'\t') => {
                j += 1
            }
            _ => break,
        }
    }
    if b.get(j) == Some(&b'=') && j > t {
        let mut k = j;
        while k > t && (b[k - 1] == b' ' || b[k - 1] == b'\t') {
            k -= 1;
        }
        out.push(t, k, Kind::Key);
        return code(&TOML_VALUE, State::Normal, b, j + 1, out);
    }
    code(&TOML_VALUE, State::Normal, b, t, out)
}

fn yaml(b: &[u8], out: &mut Out) -> State {
    let n = b.len();
    let mut t = skip_blank(b, 0);
    if t >= n {
        return State::Normal;
    }
    if t == 0 && (b.starts_with(b"---") || b.starts_with(b"...")) {
        out.push(0, 3, Kind::Meta);
        code(&YAML_VALUE, State::Normal, b, 3, out);
        return State::Normal;
    }
    while b.get(t) == Some(&b'-') && matches!(b.get(t + 1), None | Some(b' ')) {
        out.push(t, t + 1, Kind::Keyword);
        t = skip_blank(b, t + 1);
    }
    if t >= n || b[t] == b'#' {
        code(&YAML_VALUE, State::Normal, b, t, out);
        return State::Normal;
    }
    // key: — quoted, or up to ": " (or a ':' that ends the line).
    let key_end = if b[t] == b'"' || b[t] == b'\'' {
        let q = b[t];
        (t + 1..n).find(|&k| b[k] == q).map(|k| k + 1)
    } else if !b"[{&*!|>%@`".contains(&b[t]) {
        (t..n)
            .take_while(|&k| !(b[k] == b'#' && k > t && b[k - 1] == b' '))
            .find(|&k| b[k] == b':' && matches!(b.get(k + 1), None | Some(b' ') | Some(b'\t')))
    } else {
        None
    };
    if let Some(k) = key_end {
        let after = skip_blank(b, k);
        if b.get(after) == Some(&b':') {
            out.push(t, k, Kind::Key);
            code(&YAML_VALUE, State::Normal, b, after + 1, out);
            return State::Normal;
        }
    }
    code(&YAML_VALUE, State::Normal, b, t, out);
    State::Normal
}

fn markup(state: State, b: &[u8], out: &mut Out) -> State {
    let n = b.len();
    let mut i = 0;
    let comment = |from: usize, out: &mut Out| -> Option<usize> {
        match (from..n).find(|&j| b[j..].starts_with(b"-->")) {
            Some(j) => {
                out.push(from, j + 3, Kind::Comment);
                Some(j + 3)
            }
            None => {
                out.push(from, n, Kind::Comment);
                None
            }
        }
    };
    match state {
        State::MarkupComment => match comment(0, out) {
            Some(j) => i = j,
            None => return state,
        },
        State::InTag => match attributes(b, 0, out) {
            Some(j) => i = j,
            None => return state,
        },
        _ => {}
    }
    while i < n {
        let c = b[i];
        if c == b'<' {
            if b[i..].starts_with(b"<!--") {
                match comment(i, out) {
                    Some(j) => {
                        i = j;
                        continue;
                    }
                    None => return State::MarkupComment,
                }
            }
            if matches!(b.get(i + 1), Some(b'!') | Some(b'?')) {
                let end = (i..n).find(|&j| b[j] == b'>').map_or(n, |j| j + 1);
                out.push(i, end, Kind::Meta);
                i = end;
                continue;
            }
            let start = i + 1 + usize::from(b.get(i + 1) == Some(&b'/'));
            if b.get(start).is_some_and(|&d| ident_start(d)) {
                let mut end = start;
                while end < n && (ident_char(b[end]) || matches!(b[end], b'-' | b':' | b'.')) {
                    end += 1;
                }
                out.push(i, end, Kind::Tag);
                match attributes(b, end, out) {
                    Some(j) => {
                        i = j;
                        continue;
                    }
                    None => return State::InTag,
                }
            }
        }
        if c == b'&' {
            if let Some(semi) = (i + 1..n.min(i + 12)).find(|&j| b[j] == b';') {
                if b[i + 1..semi]
                    .iter()
                    .all(|d| d.is_ascii_alphanumeric() || *d == b'#')
                    && semi > i + 1
                {
                    out.push(i, semi + 1, Kind::Number);
                    i = semi + 1;
                    continue;
                }
            }
        }
        i += char_len(c);
    }
    State::Normal
}

/// A tag's attributes from `i`, to its `>`: where it closes, or `None` at the line's end.
fn attributes(b: &[u8], mut i: usize, out: &mut Out) -> Option<usize> {
    let n = b.len();
    while i < n {
        let c = b[i];
        if c == b'>' {
            out.push(i, i + 1, Kind::Tag);
            return Some(i + 1);
        }
        if (c == b'/' || c == b'?') && b.get(i + 1) == Some(&b'>') {
            out.push(i, i + 2, Kind::Tag);
            return Some(i + 2);
        }
        if c == b'"' || c == b'\'' {
            let end = (i + 1..n).find(|&j| b[j] == c).map_or(n, |j| j + 1);
            out.push(i, end, Kind::String);
            i = end;
            continue;
        }
        if ident_start(c) {
            let mut end = i;
            while end < n && (ident_char(b[end]) || matches!(b[end], b'-' | b':' | b'.')) {
                end += 1;
            }
            out.push(i, end, Kind::Key);
            i = end;
            continue;
        }
        i += char_len(c);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pieces of `line` as (text, kind), and the state after it.
    fn pieces(lang: Lang, state: State, line: &str) -> (Vec<(&str, Kind)>, State) {
        let mut v = Vec::new();
        let st = lex_line(lang, state, line, Some(&mut v));
        let mut last = 0;
        for s in &v {
            assert!(
                s.start >= last && s.end <= line.len() && s.start < s.end,
                "{v:?} in {line:?}"
            );
            assert!(line.is_char_boundary(s.start) && line.is_char_boundary(s.end));
            last = s.end;
        }
        (
            v.iter().map(|s| (&line[s.start..s.end], s.kind)).collect(),
            st,
        )
    }

    fn kinds(lang: Lang, line: &str) -> Vec<(&str, Kind)> {
        pieces(lang, State::Normal, line).0
    }

    #[test]
    fn rust_keywords_strings_comments_and_attributes() {
        use Kind::*;
        assert_eq!(
            kinds(
                Lang::Rust,
                "pub fn main() -> Result<u8> { let s = \"a\\\"b\"; // done"
            ),
            vec![
                ("pub", Keyword),
                ("fn", Keyword),
                ("main", Function),
                ("Result", Type),
                ("u8", Type),
                ("let", Keyword),
                ("\"a\\\"b\"", String),
                ("// done", Comment)
            ]
        );
        assert_eq!(
            kinds(
                Lang::Rust,
                "#[derive(Debug)] struct A<'a> { c: 'x', n: 0x1F_u8, m: MAX_N }"
            ),
            vec![
                ("#[derive(Debug)]", Meta),
                ("struct", Keyword),
                ("A", Type),
                ("'a", Type),
                ("'x'", String),
                ("0x1F_u8", Number),
                ("MAX_N", Number)
            ]
        );
        assert_eq!(
            kinds(Lang::Rust, "println!(r#\"a \"q\" b\"#, '\\n', b\"x\");"),
            vec![
                ("println!", Function),
                ("r#\"a \"q\" b\"#", String),
                ("'\\n'", String),
                ("b\"x\"", String)
            ]
        );
    }

    #[test]
    fn block_comments_and_strings_carry_over_lines() {
        let (p, st) = pieces(Lang::Rust, State::Normal, "let a = 1; /* one /* two */");
        assert_eq!(p.last().unwrap(), &("/* one /* two */", Kind::Comment));
        assert_eq!(st, State::Comment(1), "Rust's comments nest");
        let (p, st) = pieces(Lang::Rust, st, "still */ fn f() {}");
        assert_eq!(p[0], ("still */", Kind::Comment));
        assert_eq!(p[1], ("fn", Kind::Keyword));
        assert_eq!(st, State::Normal);

        let (_, st) = pieces(Lang::C, State::Normal, "/* a /* b */ int x;");
        assert_eq!(st, State::Normal, "C's do not");

        let (p, st) = pieces(Lang::Python, State::Normal, "doc = \"\"\"first");
        assert_eq!(p, vec![("\"\"\"first", Kind::String)]);
        assert_eq!(
            st,
            State::Str {
                quote: b'"',
                triple: true
            }
        );
        let (p, st) = pieces(Lang::Python, st, "end\"\"\" + 'x'");
        assert_eq!(p, vec![("end\"\"\"", Kind::String), ("'x'", Kind::String)]);
        assert_eq!(st, State::Normal);

        let (_, st) = pieces(Lang::Rust, State::Normal, "let s = r##\"open");
        assert_eq!(st, State::RawStr(2));
        let (p, st) = pieces(Lang::Rust, st, "x\"# still\"## ;");
        assert_eq!(p, vec![("x\"# still\"##", Kind::String)]);
        assert_eq!(st, State::Normal);

        let (_, st) = pieces(Lang::JavaScript, State::Normal, "const t = `a ${b}");
        assert_eq!(
            st,
            State::Str {
                quote: b'`',
                triple: false
            }
        );
        // A plain JS string unclosed ends with its line.
        let (_, st) = pieces(Lang::JavaScript, State::Normal, "const t = 'a");
        assert_eq!(st, State::Normal);
    }

    #[test]
    fn python_decorators_prefixes_and_definitions() {
        use Kind::*;
        assert_eq!(
            kinds(Lang::Python, "@app.route('/x')"),
            vec![("@app.route", Meta), ("'/x'", String)]
        );
        assert_eq!(
            kinds(Lang::Python, "def go(n=None): return f\"{n}\" # why"),
            vec![
                ("def", Keyword),
                ("go", Function),
                ("None", Number),
                ("return", Keyword),
                ("f\"{n}\"", String),
                ("# why", Comment)
            ]
        );
        assert_eq!(kinds(Lang::Python, "class A(B):")[1], ("A", Type));
    }

    #[test]
    fn shell_comments_only_start_words() {
        use Kind::*;
        assert_eq!(
            kinds(Lang::Shell, "echo a#b $HOME ${X:-1} 'it''s' # end"),
            vec![
                ("echo", Type),
                ("$HOME", Key),
                ("${X:-1}", Key),
                ("'it'", String),
                ("'s'", String),
                ("# end", Comment)
            ]
        );
        let (_, st) = pieces(Lang::Shell, State::Normal, "msg='two");
        assert_eq!(
            st,
            State::Str {
                quote: b'\'',
                triple: false
            }
        );
    }

    #[test]
    fn json_keys_against_values() {
        use Kind::*;
        assert_eq!(
            kinds(Lang::Json, r#"{"a": "b", "n" : -1.5e+3, "t": true}"#),
            vec![
                ("\"a\"", Key),
                ("\"b\"", String),
                ("\"n\"", Key),
                ("1.5e+3", Number),
                ("\"t\"", Key),
                ("true", Number)
            ]
        );
    }

    #[test]
    fn config_formats() {
        use Kind::*;
        assert_eq!(
            kinds(Lang::Toml, "[package.metadata] # c"),
            vec![("[package.metadata]", Meta), ("# c", Comment)]
        );
        assert_eq!(
            kinds(Lang::Toml, "rust-version = \"1.88\"   # why"),
            vec![
                ("rust-version", Key),
                ("\"1.88\"", String),
                ("# why", Comment)
            ]
        );
        assert_eq!(
            kinds(Lang::Toml, "\"a b\".c = [1, true]"),
            vec![("\"a b\".c", Key), ("1", Number), ("true", Number)]
        );
        let (_, st) = pieces(Lang::Toml, State::Normal, "s = '''");
        assert_eq!(
            st,
            State::Str {
                quote: b'\'',
                triple: true
            }
        );
        let (p, st) = pieces(Lang::Toml, st, "a = 1 '''");
        assert_eq!(
            p,
            vec![("a = 1 '''", String)],
            "inside the string, not a key"
        );
        assert_eq!(st, State::Normal);

        assert_eq!(
            kinds(Lang::Yaml, "- name: \"Build\" # step"),
            vec![
                ("-", Keyword),
                ("name", Key),
                ("\"Build\"", String),
                ("# step", Comment)
            ]
        );
        assert_eq!(
            kinds(Lang::Yaml, "base: &b {x: 1}"),
            vec![("base", Key), ("&b", Meta), ("1", Number)]
        );
        assert_eq!(kinds(Lang::Yaml, "url: http://x.y/z#q")[0], ("url", Key));
        assert_eq!(
            kinds(Lang::Yaml, "url: http://x.y/z#q").len(),
            1,
            "not a comment"
        );

        assert_eq!(kinds(Lang::Ini, "[core]"), vec![("[core]", Meta)]);
        assert_eq!(kinds(Lang::Ini, "  editor = vim"), vec![("editor", Key)]);
        assert_eq!(kinds(Lang::Ini, "; note"), vec![("; note", Comment)]);
    }

    #[test]
    fn markdown_fences_headings_and_code() {
        use Kind::*;
        assert_eq!(
            kinds(Lang::Markdown, "## Title"),
            vec![("## Title", Heading)]
        );
        assert_eq!(kinds(Lang::Markdown, "#hashtag"), vec![]);
        assert_eq!(
            kinds(Lang::Markdown, "- use `cargo test` and [docs](x.md)"),
            vec![("-", Keyword), ("`cargo test`", String), ("(x.md)", Key)]
        );
        let (_, st) = pieces(Lang::Markdown, State::Normal, "```rust");
        assert_eq!(st, State::Fence { ch: b'`', len: 3 });
        let (p, st) = pieces(Lang::Markdown, st, "# not a heading");
        assert_eq!(p, vec![("# not a heading", String)]);
        let (_, st) = pieces(Lang::Markdown, st, "``");
        assert_ne!(st, State::Normal, "a shorter fence does not close it");
        let (_, st) = pieces(Lang::Markdown, st, "```");
        assert_eq!(st, State::Normal);
    }

    #[test]
    fn diff_and_log_lines() {
        use Kind::*;
        assert_eq!(kinds(Lang::Diff, "+added"), vec![("+added", Added)]);
        assert_eq!(kinds(Lang::Diff, "-gone"), vec![("-gone", Removed)]);
        assert_eq!(kinds(Lang::Diff, "--- a/x"), vec![("--- a/x", Heading)]);
        assert_eq!(
            kinds(Lang::Diff, "@@ -1 +1 @@"),
            vec![("@@ -1 +1 @@", Meta)]
        );
        assert_eq!(kinds(Lang::Diff, " same"), vec![]);
        assert_eq!(
            kinds(
                Lang::Log,
                "2026-11-12T10:00:01.5Z ERROR db: \"down\" (warning: none)"
            ),
            vec![
                ("2026-11-12T10:00:01.5Z", Comment),
                ("ERROR", Error),
                ("\"down\"", String),
                ("warning", Warning)
            ]
        );
        assert_eq!(
            kinds(Lang::Log, "an error happened"),
            vec![],
            "prose is not a level"
        );
        assert_eq!(kinds(Lang::Log, "[INFO] ok")[0], ("INFO", Function));
    }

    #[test]
    fn markup_tags_attributes_comments() {
        use Kind::*;
        assert_eq!(
            kinds(Lang::Markup, "<a href=\"/x\">A &amp; B</a><!-- c -->"),
            vec![
                ("<a", Tag),
                ("href", Key),
                ("\"/x\"", String),
                (">", Tag),
                ("&amp;", Number),
                ("</a", Tag),
                (">", Tag),
                ("<!-- c -->", Comment)
            ]
        );
        let (_, st) = pieces(Lang::Markup, State::Normal, "<div class=\"a\"");
        assert_eq!(st, State::InTag);
        let (p, st) = pieces(Lang::Markup, st, "  id=\"b\">text");
        assert_eq!(p, vec![("id", Key), ("\"b\"", String), (">", Tag)]);
        assert_eq!(st, State::Normal);
        let (_, st) = pieces(Lang::Markup, State::Normal, "<!-- open");
        assert_eq!(st, State::MarkupComment);
    }

    #[test]
    fn other_tables() {
        use Kind::*;
        assert_eq!(kinds(Lang::C, "#include <stdio.h>")[0], ("#include", Meta));
        assert_eq!(kinds(Lang::C, "  #  define X 1")[0], ("#  define", Meta));
        assert_eq!(
            kinds(Lang::Go, "func (s *S) Run() error { return nil }"),
            vec![
                ("func", Keyword),
                ("Run", Function),
                ("error", Type),
                ("return", Keyword),
                ("nil", Number)
            ]
        );
        assert_eq!(
            kinds(Lang::Sql, "SELECT id FROM t -- x")[0],
            ("SELECT", Keyword)
        );
        assert_eq!(
            kinds(Lang::Css, "a:hover { color: #fff; margin: 0 10px; }"),
            vec![
                ("color", Key),
                ("#fff", Number),
                ("margin", Key),
                ("0", Number),
                ("10px", Number)
            ]
        );
        assert_eq!(
            kinds(Lang::Java, "@Override public void x() {}")[0],
            ("@Override", Meta)
        );
        assert_eq!(kinds(Lang::Kotlin, "fun main() {}")[1], ("main", Function));
        assert_eq!(kinds(Lang::CSharp, "#region X")[0], ("#region", Meta));
    }

    #[test]
    fn nothing_breaks_on_odd_input() {
        let odd = [
            "",
            "\"",
            "'",
            "r#",
            "r#\"",
            "#[",
            "#![",
            "/*",
            "*/",
            "\\",
            "\"\\",
            "'\\",
            "$",
            "${",
            "<",
            "</",
            "<a",
            "&",
            "`",
            "```",
            "@",
            "0x",
            "1e",
            "\u{fffd}\u{fffd}é漢字🎉'é'\"漢\"",
            "'é",
            "'\u{1F389}'",
        ];
        let langs = [
            Lang::Rust,
            Lang::Python,
            Lang::JavaScript,
            Lang::Json,
            Lang::Toml,
            Lang::Yaml,
            Lang::Markdown,
            Lang::Shell,
            Lang::C,
            Lang::Go,
            Lang::Java,
            Lang::Kotlin,
            Lang::CSharp,
            Lang::Markup,
            Lang::Css,
            Lang::Diff,
            Lang::Ini,
            Lang::Sql,
            Lang::Log,
        ];
        let states = [
            State::Normal,
            State::Comment(3),
            State::Str {
                quote: b'"',
                triple: true,
            },
            State::Str {
                quote: b'\'',
                triple: false,
            },
            State::RawStr(255),
            State::Fence { ch: b'~', len: 255 },
            State::MarkupComment,
            State::InTag,
        ];
        for lang in langs {
            for st in states {
                for line in odd {
                    pieces(lang, st, line);
                    pieces(lang, st, &line.repeat(50));
                }
                // A long line: linear, and it ends.
                let long = "a \"b\" /* c */ 'd' 1.5 #x ".repeat(3000);
                pieces(lang, st, &long);
            }
        }
    }
}
