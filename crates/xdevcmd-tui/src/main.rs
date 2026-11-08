//! xdevcmd — a two-pane file manager for the terminal.

// No unsafe here, and the compiler keeps it that way. Any unsafe code xdevcmd runs is in
// a dependency, where cargo-deny's advisory check is watching.
#![forbid(unsafe_code)]

mod app;
mod bookmarks;
mod cli_docs;
mod config;
mod config_edit;
mod crash;
mod find_dialog;
mod git;
mod graphics;
mod help;
mod keyfix;
mod keys;
mod layout;
mod line_edit;
mod look;
mod menu;
mod modal;
mod opener;
mod pane;
mod quiet;
mod rename_dialog;
mod safety;
mod scrollbar;
mod search;
mod session;
mod settings_dialog;
mod tempfiles;
mod theme;
mod usage;
mod usermenu;
mod viewer;
mod watch;

use std::io::stdout;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use ratatui::crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, KeyEvent, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::{Position, Rect};

use crate::app::App;
use crate::config::{Mode, Outcome};
use xdevcmd_core::policy::{self, Policy};

/// Below the first line of --help, which comes from Cargo.toml.
pub(crate) const USAGE: &str = "\
Usage:
  xdevcmd [LEFT] [RIGHT]       start with these directories in the two panes
  xdevcmd --no-restore         start fresh instead of reopening the last session
  xdevcmd --read-only          xdevcmd writes nothing: no copy, move, delete, rename, edit
  xdevcmd --restricted         read-only, and no shell, no editor, no clipboard
  xdevcmd --root DIR           never navigate above DIR
                               These only tighten [security] in the config, never loosen it.
  xdevcmd --cwd-file PATH      on quit, write the directory you were in to PATH, so a
                               shell function can cd there (Q quits without it)
  xdevcmd --config PATH        use this config file instead of the default one
  xdevcmd --print-config       print every setting at its default, ready to save
  xdevcmd --print-theme NAME   print a built-in theme as a file, to start your own
  xdevcmd --print-keys         print every key as Markdown (as configured)
  xdevcmd --print-completions SHELL   completions for bash, zsh, fish or powershell
  xdevcmd --print-man          print the manual page (roff): xdevcmd --print-man | man -l -
  xdevcmd --check-config       validate the config file; exit 1 if it is wrong
  xdevcmd --show-keys          print each key, click and scroll as xdevcmd receives it, and
                               what it would do (for a key that does nothing)
  xdevcmd --screenshot[=WxH]   render one frame as plain text and exit
          --keys=F8,Enter      with --screenshot: type these keys first; mouse too:
                               click:X:Y rclick:X:Y ctrl-click:X:Y drag:X1:Y1:X2:Y2
                               wheel:X:Y:N move:X:Y
  xdevcmd --version            print the version and exit
  xdevcmd --help               print this and exit

Press F1 inside xdevcmd for every key and setting as currently configured.

Config file, first match wins:
  $XDEVCMD_CONFIG
  $XDG_CONFIG_HOME/xdevcmd/config.toml
  %APPDATA%\\xdevcmd\\config.toml            (Windows)
  ~/.config/xdevcmd/config.toml

  mkdir -p ~/.config/xdevcmd && xdevcmd --print-config > ~/.config/xdevcmd/config.toml

Your own theme starts from a built-in one (dark, light, ansi, blue, high-contrast):
  mkdir -p ~/.config/xdevcmd/themes
  xdevcmd --print-theme dark > ~/.config/xdevcmd/themes/mine.toml
";

#[derive(Default)]
struct Cli {
    paths: Vec<PathBuf>,
    config: Option<PathBuf>,
    screenshot: Option<(u16, u16)>,
    keys: String,
    print_config: bool,
    print_theme: Option<String>,
    print_completions: Option<String>,
    print_man: bool,
    print_keys: bool,
    show_keys: bool,
    check_config: bool,
    no_restore: bool,
    read_only: bool,
    restricted: bool,
    root: Option<PathBuf>,
    cwd_file: Option<PathBuf>,
    help: bool,
    version: bool,
}

fn parse_cli() -> std::result::Result<Cli, String> {
    let mut cli = Cli::default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => cli.help = true,
            "-V" | "--version" => cli.version = true,
            "--print-config" => cli.print_config = true,
            "--print-man" => cli.print_man = true,
            "--print-completions" => {
                cli.print_completions =
                    Some(args.next().ok_or(
                        "--print-completions needs a shell: bash, zsh, fish or powershell",
                    )?);
            }
            s if s.starts_with("--print-completions=") => {
                cli.print_completions = Some(s["--print-completions=".len()..].to_string());
            }
            "--print-theme" => {
                cli.print_theme = Some(args.next().ok_or("--print-theme needs a theme name")?);
            }
            s if s.starts_with("--print-theme=") => {
                cli.print_theme = Some(s["--print-theme=".len()..].to_string());
            }
            "--print-keys" => cli.print_keys = true,
            "--show-keys" => cli.show_keys = true,
            "--check-config" => cli.check_config = true,
            "--no-restore" => cli.no_restore = true,
            "--read-only" => cli.read_only = true,
            "--restricted" => cli.restricted = true,
            "--root" => {
                cli.root = Some(args.next().ok_or("--root needs a directory")?.into());
            }
            s if s.starts_with("--root=") => cli.root = Some(s["--root=".len()..].into()),
            "--cwd-file" => {
                cli.cwd_file = Some(args.next().ok_or("--cwd-file needs a path")?.into());
            }
            s if s.starts_with("--cwd-file=") => {
                cli.cwd_file = Some(s["--cwd-file=".len()..].into());
            }
            "--config" => {
                cli.config = Some(args.next().ok_or("--config needs a path")?.into());
            }
            s if s.starts_with("--config=") => cli.config = Some(s["--config=".len()..].into()),
            s if s.starts_with("--keys=") => cli.keys = s["--keys=".len()..].to_string(),
            s if s.starts_with("--screenshot") => {
                cli.screenshot = Some(
                    s.split_once('=')
                        .and_then(|(_, wh)| wh.split_once('x'))
                        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                        .unwrap_or((120, 36)),
                );
            }
            s if s.starts_with('-') && s.len() > 1 => {
                return Err(format!("unknown option {s} (try --help)"));
            }
            _ => cli.paths.push(a.into()),
        }
    }
    Ok(cli)
}

fn main() -> Result<()> {
    let cli = match parse_cli() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("xdevcmd: {e}");
            std::process::exit(2);
        }
    };
    if cli.help {
        emit(&format!(
            "xdevcmd {} \u{2014} {}\n\n{USAGE}",
            env!("CARGO_PKG_VERSION"),
            env!("CARGO_PKG_DESCRIPTION")
        ));
        return Ok(());
    }
    if cli.version {
        emit(&format!(
            "xdevcmd {} \u{b7} {} \u{b7} {}\n",
            env!("CARGO_PKG_VERSION"),
            env!("CARGO_PKG_LICENSE"),
            env!("CARGO_PKG_AUTHORS").replace(':', ", ")
        ));
        return Ok(());
    }
    if cli.print_config {
        emit(&config::render_default());
        return Ok(());
    }
    if cli.print_man {
        emit(&cli_docs::man());
        return Ok(());
    }
    if let Some(shell) = &cli.print_completions {
        match cli_docs::completions(shell) {
            Ok(text) => emit(&text),
            Err(e) => {
                eprintln!("xdevcmd: {e}");
                std::process::exit(2);
            }
        }
        return Ok(());
    }
    if let Some(name) = &cli.print_theme {
        match config::render_theme(name) {
            Ok(text) => emit(&text),
            Err(e) => {
                eprintln!("xdevcmd: {e}");
                std::process::exit(2);
            }
        }
        return Ok(());
    }

    // What earlier runs left in the temporary directory, if they ended badly.
    tempfiles::sweep();

    let (mut settings, outcome) = config::load(cli.config.as_deref());
    if cli.check_config {
        match &outcome {
            Outcome::Loaded(p) => emit(&format!("ok: {}\n", p.display())),
            Outcome::NoFile => match config::default_path() {
                Some(p) => emit(&format!("no config at {} — defaults in use\n", p.display())),
                None => emit("no config location — defaults in use\n"),
            },
            Outcome::Rejected { .. } => {
                eprintln!("{}", outcome.message().unwrap_or_default());
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    if cli.print_keys {
        emit(&keys::markdown(&settings.keymap));
        return Ok(());
    }
    // How many colours this terminal shows: XDEVCMD_COLORS, then [theme] colors, then
    // what the environment says. The 16-colour theme is a palette of its own, not the
    // 24-bit one squeezed.
    let depth = match std::env::var("XDEVCMD_COLORS")
        .ok()
        .filter(|v| !v.trim().is_empty())
    {
        Some(v) => match xdevcmd_core::color::Depth::parse(&v) {
            Ok(d) => d,
            Err(e) => fail(&format!("XDEVCMD_COLORS={v}: {e}")),
        },
        None => settings.colors,
    }
    .unwrap_or_else(|| xdevcmd_core::color::detect(|k| std::env::var(k).ok(), cfg!(windows)));
    settings.depth = depth;
    theme::init(if depth == xdevcmd_core::color::Depth::Ansi16 {
        settings.theme16
    } else {
        settings.theme
    });
    if cli.show_keys {
        return show_keys(&settings.keymap, depth);
    }

    // The command line may only tighten what the config says. A wrapper that sets
    // restricted mode cannot be escaped by adding a flag.
    let cli_mode = if cli.restricted {
        Mode::Restricted
    } else if cli.read_only {
        Mode::ReadOnly
    } else {
        Mode::Normal
    };
    settings.mode = settings.mode.max(cli_mode);
    if let Some(r) = &cli.root {
        let r = match xdevcmd_core::paths::canonical(r) {
            Ok(p) if p.is_dir() => p,
            Ok(_) => fail(&format!("--root {}: not a directory", r.display())),
            Err(e) => fail(&format!("--root {}: {e}", r.display())),
        };
        if let Some(c) = &settings.root {
            if !r.starts_with(c) {
                fail(&format!(
                    "--root must be inside the configured root {}",
                    c.display()
                ));
            }
        }
        settings.root = Some(r);
    }
    let files = &settings.config.files;
    crate::pane::files::set_fuzzy(files.filter == "fuzzy");
    xdevcmd_core::ops::set_safe(settings.config.general.safe_operations);
    // Safe operations' journal sits beside the session, in the state directory.
    if let Some(dir) = session::default_path()
        .as_deref()
        .and_then(std::path::Path::parent)
    {
        xdevcmd_core::journal::init(dir.join("journal"));
    }
    look::init(look::Look {
        ls: files
            .ls_colors
            .then(|| std::env::var("LS_COLORS").ok())
            .flatten()
            .map(|s| look::LsColors::parse(&s)),
        icons: files.icons,
        git: files.git_status && settings.mode != Mode::Restricted,
    });
    policy::install(Policy {
        read_only: settings.mode.read_only(),
        root: settings.root.clone(),
    });

    // A start directory outside the root opens at the root instead.
    // Directories named relative to where xdevcmd was started are made absolute first:
    // the title, its clickable path and Backspace all need the whole path.
    let cwd = std::env::current_dir()?;
    let absolute = |p: &PathBuf| std::path::absolute(p).unwrap_or_else(|_| cwd.join(p));
    let left = policy::get().clamp(
        &cli.paths
            .first()
            .map(absolute)
            .unwrap_or_else(|| cwd.clone()),
    );
    let right = policy::get().clamp(
        &cli.paths
            .get(1)
            .map(absolute)
            .unwrap_or_else(|| cwd.clone()),
    );

    // The last session, unless directories were named or it was declined. Anything
    // wrong with it means a fresh start, never a failure to open.
    let restore =
        cli.paths.is_empty() && !cli.no_restore && settings.config.general.restore_session;
    let restored = if restore {
        session::default_path()
            .and_then(|p| session::load(&p))
            .and_then(|s| App::from_session(&s, settings.clone(), outcome.clone()))
    } else {
        None
    };
    let mut app = match restored {
        Some(app) => app,
        None => App::new(&left, &right, settings, outcome),
    };
    app.cwd_file = cli.cwd_file.clone();

    if let Some((w, h)) = cli.screenshot {
        return screenshot(app, w, h, &cli.keys);
    }

    // init() for what it sets up (raw mode, the alternate screen, a panic hook that
    // undoes them); then our own Terminal, whose backend stays quiet when idle.
    drop(ratatui::init());
    // What the terminal can draw, asked before anything reads its replies; over SSH the
    // environment rarely says. Not inside tmux, which answers for itself.
    #[cfg(unix)]
    if std::env::var_os("TMUX").is_none() && app.settings_images_probe() {
        // Ends as soon as the device attributes come, which every terminal answers;
        // long enough that a slow SSH link's replies are not left for crossterm to read
        // as keys.
        app.probe = graphics::probe(Duration::from_millis(1500));
    }
    let mut terminal = ratatui::Terminal::new(
        quiet::Quiet::new(ratatui::backend::CrosstermBackend::new(stdout())).with_depth(depth),
    )?;
    execute!(stdout(), EnableMouseCapture)?;

    // ratatui::init() installs a hook that restores the terminal; chain onto it so a
    // panic also turns mouse reporting back off, which otherwise leaves the user's shell
    // spraying escape codes on every click.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(stdout(), DisableMouseCapture);
        let log = crash::write(info);
        previous(info);
        // After the previous hook: the terminal is itself again, so this can be read.
        crash::tell(log.as_deref());
    }));

    let result = app.run(&mut terminal);

    let _ = execute!(stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

/// Render a single frame into an off-screen buffer and print it as text.
///
/// This is the only way to look at the whole layout without a terminal attached, which
/// makes it the thing to reach for when a pane is the wrong size or a column is off by
/// one — and it is checkable in a test, unlike a screenshot of a screen.
fn screenshot(mut app: App, w: u16, h: u16, keys: &str) -> Result<()> {
    let backend = ratatui::backend::TestBackend::new(w, h);
    let mut terminal = ratatui::Terminal::new(backend)?;
    app.boot(Rect::new(0, 0, w, h));
    // Give the shell a moment to print its prompt, or the pane renders empty and looks
    // broken when it is merely early.
    // XDEVCMD_SCREENSHOT_WAIT_MS=0 skips it: a sweep of hundreds of frames that only
    // checks layout should not spend most of its time waiting for prompts.
    let wait = std::env::var("XDEVCMD_SCREENSHOT_WAIT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(900);
    std::thread::sleep(Duration::from_millis(wait));
    // Let background work that started with the panes (git status, sizes) report in.
    app.tick();
    // Draw once before any input: clicks hit-test against where things were drawn.
    terminal.draw(|f| app.draw(f))?;
    for input in parse_inputs(keys) {
        match input {
            Input::Key(k) => app.feed_key(k),
            Input::Mouse(m) => app.feed_mouse(m),
        }
        // Let a job that an input started make progress, then redraw so the next
        // click lands on what is now on screen.
        std::thread::sleep(Duration::from_millis(30));
        app.tick();
        terminal.draw(|f| app.draw(f))?;
    }
    // A job the last key started finishes before the frame is taken (a slow machine
    // took longer than the pause above), unless it waits on a question to show.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while app.job_working() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
        app.tick();
    }
    terminal.draw(|f| app.draw(f))?;

    let buf = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..h {
        let mut line = String::new();
        for x in 0..w {
            line.push_str(buf.cell(Position::new(x, y)).map_or(" ", |c| c.symbol()));
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    emit(&out);
    Ok(())
}

/// Write to stdout. A reader that stops early (`xdevcmd --screenshot | head`) ends xdevcmd
/// quietly with success, as it ends `cat`; `println!` panics there instead, once the
/// output is larger than the pipe holds.
fn emit(text: &str) {
    use std::io::Write;
    let mut out = stdout().lock();
    if let Err(e) = out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        eprintln!("xdevcmd: {e}");
        std::process::exit(1);
    }
}

enum Input {
    Key(KeyEvent),
    Mouse(MouseEvent),
}

/// `F5`, `S-F8`, `A-s`, `PageDown`, `Enter`, `x` — the same names the config accepts,
/// plus `S-` / `C-` / `A-` shorthands — and mouse tokens: `click:X:Y`, `rclick:X:Y`,
/// `ctrl-click:X:Y`, `shift-click:X:Y`, `drag:X1:Y1:X2:Y2`, `wheel:X:Y:N`, `move:X:Y`.
fn parse_inputs(spec: &str) -> Vec<Input> {
    let mut out = Vec::new();
    for tok in spec.split(',').filter(|s| !s.is_empty()) {
        if let Some(events) = parse_mouse(tok) {
            out.extend(events.into_iter().map(Input::Mouse));
            continue;
        }
        let long = if let Some(r) = tok.strip_prefix("S-") {
            format!("Shift+{r}")
        } else if let Some(r) = tok.strip_prefix("C-") {
            format!("Ctrl+{r}")
        } else if let Some(r) = tok.strip_prefix("A-") {
            format!("Alt+{r}")
        } else {
            tok.to_string()
        };
        if let Ok((code, mods)) = keys::parse_key(&long) {
            out.push(Input::Key(KeyEvent::new(code, mods)));
        }
    }
    out
}

fn parse_mouse(tok: &str) -> Option<Vec<MouseEvent>> {
    let (what, rest) = tok.split_once(':')?;
    let n: Vec<i32> = rest
        .split(':')
        .map(|s| s.parse().ok())
        .collect::<Option<_>>()?;
    let ev = |kind, x: i32, y: i32, modifiers| MouseEvent {
        kind,
        column: x.max(0) as u16,
        row: y.max(0) as u16,
        modifiers,
    };
    let none = KeyModifiers::NONE;
    let click = |x, y, b, m| {
        vec![
            ev(MouseEventKind::Down(b), x, y, m),
            ev(MouseEventKind::Up(b), x, y, m),
        ]
    };
    Some(match (what, n.as_slice()) {
        ("click", [x, y]) => click(*x, *y, MouseButton::Left, none),
        ("ctrl-click", [x, y]) => click(*x, *y, MouseButton::Left, KeyModifiers::CONTROL),
        ("shift-click", [x, y]) => click(*x, *y, MouseButton::Left, KeyModifiers::SHIFT),
        ("rclick", [x, y]) => click(*x, *y, MouseButton::Right, none),
        ("move", [x, y]) => vec![ev(MouseEventKind::Moved, *x, *y, none)],
        ("wheel", [x, y, d]) => {
            let kind = if *d > 0 {
                MouseEventKind::ScrollDown
            } else {
                MouseEventKind::ScrollUp
            };
            (0..d.abs()).map(|_| ev(kind, *x, *y, none)).collect()
        }
        ("drag", [x1, y1, x2, y2]) => {
            // A real drag reports every cell it crosses; six steps is enough to pass
            // through any pane boundary in between.
            let mut v = vec![ev(MouseEventKind::Down(MouseButton::Left), *x1, *y1, none)];
            for s in 1..=6 {
                let x = x1 + (x2 - x1) * s / 6;
                let y = y1 + (y2 - y1) * s / 6;
                v.push(ev(MouseEventKind::Drag(MouseButton::Left), x, y, none));
            }
            v.push(ev(MouseEventKind::Up(MouseButton::Left), *x2, *y2, none));
            v
        }
        _ => return None,
    })
}

/// `--show-keys`: what the terminal actually sends, one line per event, and what xdevcmd
/// would do with it. For "key X does nothing" reports: the answer is usually that the
/// terminal never sent X, and this shows it.
fn show_keys(keymap: &keys::Keymap, depth: xdevcmd_core::color::Depth) -> Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseEventKind};
    use ratatui::crossterm::terminal;
    use std::io::Write;

    let var = |k: &str| std::env::var(k).unwrap_or_default();
    println!("xdevcmd --show-keys: press keys, click, scroll. Esc twice, or Ctrl+C, quits.");
    println!(
        "terminal: TERM={} TERM_PROGRAM={} {} COLORTERM={} -> colours: {}",
        var("TERM"),
        var("TERM_PROGRAM"),
        var("TERM_PROGRAM_VERSION"),
        var("COLORTERM"),
        depth.label()
    );
    println!();
    let name_of = |a: keys::Action| {
        keys::BINDINGS
            .iter()
            .find(|b| b.action == a)
            .map_or("?", |b| b.name)
    };
    let does = |k: KeyEvent| {
        let k = keyfix::shifted_f(k, |x| keymap.action_for(x).is_some());
        keymap.action_for(k).map_or("-", name_of)
    };

    terminal::enable_raw_mode()?;
    execute!(stdout(), EnableMouseCapture)?;
    let mut out = stdout();
    let mut last_esc = false;
    let mut moved = false;
    let run = (|| -> Result<()> {
        loop {
            let line = match event::read()? {
                Event::Key(k) if k.kind != KeyEventKind::Release => {
                    let quit = (k.code == KeyCode::Esc && last_esc)
                        || (k.code == KeyCode::Char('c')
                            && k.modifiers.contains(KeyModifiers::CONTROL));
                    last_esc = k.code == KeyCode::Esc;
                    let line = format!(
                        "key    {:<18} {}",
                        keys::key_name(k.code, k.modifiers),
                        does(k)
                    );
                    if quit {
                        write!(out, "{line}\r\n")?;
                        out.flush()?;
                        return Ok(());
                    }
                    line
                }
                Event::Key(_) => continue,
                Event::Mouse(m) if m.kind == MouseEventKind::Moved => {
                    // Reported on every cell crossed: say so once.
                    if moved {
                        continue;
                    }
                    moved = true;
                    "mouse  movement is reported (shown once)".to_string()
                }
                Event::Mouse(m) => format!(
                    "mouse  {:?} at column {}, row {}{}",
                    m.kind,
                    m.column,
                    m.row,
                    if m.modifiers.is_empty() {
                        String::new()
                    } else {
                        format!(" with {:?}", m.modifiers)
                    }
                ),
                Event::Resize(w, h) => format!("resize {w}x{h}"),
                Event::Paste(p) => format!("paste  {} characters", p.chars().count()),
                Event::FocusGained => "focus  gained".into(),
                Event::FocusLost => "focus  lost".into(),
            };
            write!(out, "{line}\r\n")?;
            out.flush()?;
        }
    })();
    let _ = execute!(stdout(), DisableMouseCapture);
    let _ = terminal::disable_raw_mode();
    run
}

fn fail(msg: &str) -> ! {
    eprintln!("xdevcmd: {msg}");
    std::process::exit(2);
}
