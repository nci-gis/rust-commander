//! File operations: copy, move, delete, mkdir, rename, rename many, links, permissions, checksums.

use super::*;

impl App {
    /// Where a copy or move from the focused pane could go, best first: the other file
    /// panes by how recently they had focus, then any never focused, one entry per
    /// directory, and a directory equal to the source's own last. With two panes this
    /// is just "the other pane", as it always was.
    pub(super) fn transfer_targets(&self) -> Vec<PathBuf> {
        let src = self.focused_cwd();
        let mut panes: Vec<PaneId> = self
            .file_panes()
            .into_iter()
            .filter(|id| *id != self.focus)
            .collect();
        let rank = |id: &PaneId| self.mru.iter().position(|m| m == id).unwrap_or(usize::MAX);
        panes.sort_by_key(rank);
        let mut dirs: Vec<PathBuf> = Vec::new();
        for id in panes {
            // A pane browsing an archive or showing search results is not a place
            // files can be copied into.
            let into = self
                .panes
                .get(&id)
                .and_then(Pane::as_files)
                .filter(|f| f.archive.is_none() && f.results.is_none());
            if let Some(f) = into {
                if !dirs.contains(&f.cwd) {
                    dirs.push(f.cwd.clone());
                }
            }
        }
        // Stable sort: recency order is kept within each group.
        dirs.sort_by_key(|d| *d == src);
        dirs
    }

    pub(super) fn other_pane_cwd(&self) -> Option<PathBuf> {
        self.transfer_targets().into_iter().next()
    }

    /// "1 file", "3 files" — the singular matters because these strings sit in a
    /// confirmation for something irreversible, and sloppy copy reads as untrustworthy.
    pub(super) fn count_label(files: u64, bytes: u64) -> String {
        let noun = if files == 1 { "file" } else { "files" };
        if bytes < 1024 {
            format!("{files} {noun} \u{2022} {bytes} bytes")
        } else {
            format!("{files} {noun} \u{2022} {}", human_size(bytes))
        }
    }

    pub(super) fn describe(sources: &[PathBuf]) -> String {
        match sources {
            [one] => one
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| one.display().to_string()),
            many => format!("{} items", many.len()),
        }
    }

    pub(super) fn begin_transfer(&mut self, kind: ops::Kind) {
        let sources = self.selection();
        if sources.is_empty() {
            self.status = "nothing selected".into();
            return;
        }
        let choices = self.transfer_targets();
        if let Some((file, prefix)) = self
            .focused_archive()
            .map(|a| (a.file.clone(), a.prefix.clone()))
        {
            let inner = self
                .focused_archive()
                .map(|a| a.inner(&sources))
                .unwrap_or_default();
            let Some(dest) = choices.first().cloned() else {
                self.status = "extracting goes into another pane \u{2014} Alt+S splits one".into();
                return;
            };
            self.confirm_extract(file, inner, prefix, dest, choices);
            return;
        }
        let Some(dest) = choices.first().cloned() else {
            self.status = "no second pane to copy to — Alt+S splits one".into();
            return;
        };
        let request = match kind {
            ops::Kind::Copy => Request::copy(sources.clone(), dest.clone()),
            _ => Request::mv(sources.clone(), dest.clone()),
        };
        // Overwrites still raise a question mid-job; this only skips the "are you sure"
        // before starting, which is all the setting claims to do.
        if !self.settings.config.general.confirm_transfer {
            self.remember_job(&request);
            self.modal = Some(Modal::Job(Job::new(kind, ops::start(request))));
            return;
        }
        self.modal = Some(Modal::Confirm(Confirm {
            title: kind.verb().to_string(),
            body: vec![
                Self::describe(&sources),
                String::new(),
                format!("to    {}", dest.display()),
                COUNTING.into(),
            ],
            danger: false,
            request,
            buttons: Vec::new(),
            choices,
            choice: 0,
            source_dir: Some(self.focused_cwd()),
            counting: Some((ops::count(sources), 3)),
            permanent: false,
            // set_destination writes the line, after the destination's own lines.
            existing_line: Some(0),
        }));
        if let Some(Modal::Confirm(c)) = self.modal.as_mut() {
            c.set_destination(0);
        }
    }

    pub(super) fn begin_delete(&mut self, permanent: bool) {
        let targets = self.selection();
        if targets.is_empty() {
            self.status = "nothing selected".into();
            return;
        }
        let mut body = vec![Self::describe(&targets), String::new(), COUNTING.into()];
        let counting = Some((ops::count(targets.clone()), 2));
        if permanent {
            // What goes, by name: the first few, so a wrong selection shows.
            if targets.len() > 1 {
                body.push(String::new());
                for t in targets.iter().take(3) {
                    let name = t.file_name().map(|n| n.to_string_lossy().into_owned());
                    body.push(format!(
                        "  {}",
                        name.unwrap_or_else(|| t.display().to_string())
                    ));
                }
                if targets.len() > 3 {
                    body.push(format!("  \u{2026} and {} more", targets.len() - 3));
                }
            }
            body.push(String::new());
            body.push("Deleted permanently. This cannot be undone.".into());
        } else {
            body.push("Moved to the trash.".into());
        }
        // A list's names may come from any text a shell printed: say where from, so a
        // list made of the wrong output shows before anything is deleted.
        let from = self
            .panes
            .get(&self.focus)
            .and_then(Pane::as_files)
            .and_then(|f| f.results.as_ref())
            .map(|r| {
                let what = xdevcmd_core::format::elide_middle(&r.what, 50);
                if r.find {
                    format!("Found by: find {what}")
                } else {
                    format!("From the list: {what}")
                }
            });
        if let Some(from) = from {
            body.push(String::new());
            body.push(from);
        }
        self.modal = Some(Modal::Confirm(Confirm {
            title: if permanent {
                "Delete permanently".into()
            } else {
                "Delete".to_string()
            },
            body,
            danger: true,
            request: Request::delete(targets, !permanent),
            buttons: Vec::new(),
            choices: Vec::new(),
            choice: 0,
            source_dir: None,
            counting,
            permanent,
            existing_line: None,
        }));
    }

    pub(super) fn begin_mkdir(&mut self) {
        let dir = self.focused_cwd();
        self.modal = Some(Modal::Input(Input {
            title: "Create directory".into(),
            label: format!("in {}", dir.display()),
            edit: LineEdit::new(""),
            kind: InputKind::MkDir,
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    pub(super) fn begin_rename(&mut self) {
        let Some(f) = self.panes.get(&self.focus).and_then(Pane::as_files) else {
            return;
        };
        let Some(e) = f.current().filter(|e| !e.is_parent()) else {
            self.status = "nothing to rename here".into();
            return;
        };
        self.modal = Some(Modal::Input(Input {
            title: "Rename".into(),
            label: format!("in {}", f.cwd.display()),
            edit: LineEdit::for_rename(&e.name, e.is_dir()),
            kind: InputKind::Rename {
                from: e.path.clone(),
            },
            dir: f.cwd.clone(),
            buttons: Vec::new(),
            error: None,
        }));
    }

    pub(super) fn apply_input(&mut self, i: Input) {
        let text = i.edit.text();
        match &i.kind {
            InputKind::SaveAs => self.apply_save_as(i),
            InputKind::MkDir => {
                let name = text.trim();
                if name.is_empty() {
                    self.status = "no name given".into();
                    return;
                }
                let target = i.dir.join(name);
                if let Err(e) = xdevcmd_core::policy::guard_write() {
                    self.status = e.to_string();
                    return;
                }
                let first = xdevcmd_core::undo::first_missing(&target);
                // Under --root: no `..` (create_dir_all would follow it out), and the
                // directory it is made in must be inside, links resolved.
                let climbs = Path::new(name)
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir));
                let inside = first
                    .parent()
                    .is_some_and(|p| xdevcmd_core::policy::get().allows(p));
                if xdevcmd_core::policy::get().root.is_some() && (climbs || !inside) {
                    self.status = "outside the root xdevcmd is confined to".into();
                    return;
                }
                let existed = target.exists();
                match std::fs::create_dir_all(&target) {
                    Ok(()) => {
                        if !existed {
                            self.last_undo = Some(xdevcmd_core::undo::Undo::MadeDir(first));
                        }
                        self.status = format!("created {name}");
                        self.reload_all();
                    }
                    Err(e) => self.status = format!("mkdir failed: {e}"),
                }
            }
            InputKind::GoTo => {
                let typed = text.trim();
                if typed.is_empty() {
                    return;
                }
                let mut p = expand_tilde(typed);
                if p.is_relative() {
                    p = i.dir.join(p);
                }
                match std::fs::metadata(&p) {
                    Ok(m) if m.is_dir() => self.with_files(|f| f.set_cwd(&p)),
                    Ok(_) => {
                        let (Some(parent), Some(name)) = (p.parent(), p.file_name()) else {
                            return;
                        };
                        let name = name.to_os_string();
                        self.with_files(|f| {
                            f.set_cwd(parent);
                            let target = f.cwd.join(&name);
                            f.select_path(&target);
                        });
                    }
                    Err(e) => {
                        self.modal = Some(Modal::Input(Input {
                            error: Some(e.to_string()),
                            ..i
                        }));
                    }
                }
            }
            InputKind::NewFile => {
                let name = text.trim();
                if let Err(e) = xdevcmd_core::rename::validate_name(name) {
                    self.modal = Some(Modal::Input(Input {
                        error: Some(e.to_string()),
                        ..i
                    }));
                    return;
                }
                let target = i.dir.join(name);
                if let Err(e) = xdevcmd_core::policy::guard_write() {
                    self.status = e.to_string();
                    return;
                }
                // An existing file is opened, not truncated: Shift+F4 on a name that
                // is there already means "edit it".
                let made = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&target);
                match made {
                    Ok(_) => {
                        self.last_undo = Some(xdevcmd_core::undo::Undo::MadeFile(target.clone()));
                        self.status = format!("created {name}");
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        if target.is_dir() {
                            self.status = format!("{name} is a directory");
                            return;
                        }
                    }
                    Err(e) => {
                        self.status = format!("cannot create {name}: {e}");
                        return;
                    }
                }
                self.reload_all();
                self.with_files(|f| f.select_path(&target));
                if xdevcmd_core::policy::get().allows(&target) {
                    self.pending_edit = Some(target);
                } else {
                    // A name that exists as a link out of the root.
                    self.status = "that file is outside the root xdevcmd is confined to".into();
                }
            }
            InputKind::CopyAs { from } => {
                let name = text.trim();
                if let Err(e) = xdevcmd_core::rename::validate_name(name) {
                    self.modal = Some(Modal::Input(Input {
                        error: Some(e.to_string()),
                        ..i
                    }));
                    return;
                }
                let mut req = Request::copy(vec![from.clone()], i.dir.clone());
                req.rename_to = Some(name.into());
                self.remember_job(&req);
                self.modal = Some(Modal::Job(Job::new(ops::Kind::Copy, ops::start(req))));
            }
            InputKind::Permissions { targets } => {
                self.last_undo = Some(xdevcmd_core::undo::Undo::Cannot(
                    "a change of permissions".into(),
                ));
                let setting = match xdevcmd_core::attrs::parse(&text) {
                    Ok(s) => s,
                    Err(e) => {
                        self.modal = Some(Modal::Input(Input {
                            error: Some(e),
                            ..i
                        }));
                        return;
                    }
                };
                let mut done = 0;
                let mut first_err = None;
                for t in targets {
                    match xdevcmd_core::attrs::apply(t, setting) {
                        Ok(()) => done += 1,
                        Err(e) => {
                            first_err.get_or_insert_with(|| {
                                format!(
                                    "{}: {e}",
                                    t.file_name().unwrap_or_default().to_string_lossy()
                                )
                            });
                        }
                    }
                }
                self.reload_all();
                self.status = match first_err {
                    None => format!("permissions set on {done}"),
                    Some(e) => format!("set on {done}, not on the rest \u{2014} {e}"),
                };
            }
            InputKind::Pack { sources } => {
                let typed = text.trim();
                if typed.is_empty() {
                    return;
                }
                let mut dest = expand_tilde(typed);
                if dest.is_relative() {
                    dest = i.dir.join(dest);
                }
                // Under --root, as F7: no `..`, and the directory it goes in must be
                // inside, links resolved.
                let climbs = Path::new(typed)
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir));
                let inside = dest
                    .parent()
                    .is_some_and(|p| xdevcmd_core::policy::get().allows(p));
                if xdevcmd_core::policy::get().root.is_some() && (climbs || !inside) {
                    self.status = "outside the root xdevcmd is confined to".into();
                    return;
                }
                if xdevcmd_core::archive::format_of(&dest).is_none() {
                    self.modal = Some(Modal::Input(Input {
                        error: Some("the name must end in .zip, .tar, .tar.gz or .tgz".into()),
                        ..i
                    }));
                    return;
                }
                let req = Request::pack(sources.clone(), dest);
                self.remember_job(&req);
                self.modal = Some(Modal::Job(Job::new(ops::Kind::Pack, ops::start(req))));
            }
            InputKind::Pattern { mark } => {
                let mark = *mark;
                let pattern = xdevcmd_core::glob::Pattern::new(&text);
                if pattern.is_empty() {
                    self.status = "no pattern given".into();
                    return;
                }
                let n = self
                    .focused_files()
                    .map_or(0, |f| f.mark_matching(&pattern, mark));
                let verb = if mark { "marked" } else { "unmarked" };
                self.status = match n {
                    0 => format!("nothing matches {}", text.trim()),
                    n => format!("{verb} {n} matching {}", text.trim()),
                };
            }
            InputKind::Rename { from } => match xdevcmd_core::rename::rename(from, &text) {
                Ok(to) => {
                    if to != *from {
                        self.last_undo = Some(xdevcmd_core::undo::Undo::Rename(vec![(
                            from.clone(),
                            to.clone(),
                        )]));
                    }
                    let old = from.file_name().map(|s| s.to_string_lossy().into_owned());
                    self.status = format!("renamed {} \u{2192} {text}", old.unwrap_or_default());
                    self.reload_all();
                    for p in self.panes.values_mut() {
                        if let Some(f) = p.as_files_mut() {
                            f.select_path(&to);
                        }
                    }
                }
                Err(e) => {
                    // Back on screen with the reason, and the text as typed.
                    self.modal = Some(Modal::Input(Input {
                        error: Some(e.to_string()),
                        ..i
                    }));
                }
            },
        }
    }

    /// F4 in the rename dialog: `text`, one name per file, into a file for the editor.
    pub(super) fn begin_batch(&mut self, files: Vec<PathBuf>, text: &str) {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let Some(list) = crate::tempfiles::file(&format!("rename-{n}.txt")) else {
            self.status = "cannot make a temporary directory for the list of names".into();
            return;
        };
        if let Err(e) = session::write_private(&list, text.as_bytes()) {
            self.status = format!("cannot write the list of names: {e}");
            return;
        }
        self.pending_batch = Some((files, list.clone()));
        self.pending_edit = Some(list);
    }

    /// The editor has closed: read the names back and ask before renaming anything.
    pub(super) fn finish_batch(&mut self, files: Vec<PathBuf>, list: &Path) {
        let edited = std::fs::read_to_string(list);
        let _ = std::fs::remove_file(list);
        let edited = match edited {
            Ok(t) => t,
            Err(e) => {
                self.status = format!("cannot read the names back: {e}");
                return;
            }
        };
        let moves = match xdevcmd_core::batch_rename::plan(&files, &edited) {
            Ok(m) => m,
            Err(e) => {
                self.status = format!("nothing renamed \u{2014} {e}");
                return;
            }
        };
        if moves.is_empty() {
            self.status = "no names changed".into();
            return;
        }
        let name = |p: &PathBuf| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        };
        let body = moves
            .iter()
            .map(|(f, t)| format!("{}  \u{2192}  {}", name(f), name(t)))
            .collect();
        let n = moves.len();
        self.modal = Some(Modal::Ask(modal::Ask {
            title: format!("Rename {n} file{}", if n == 1 { "" } else { "s" }),
            body,
            danger: false,
            yes: Some(("rename".into(), Cmd::BatchRename(moves))),
            buttons: Vec::new(),
            also: None,
            footnote: None,
            pictures: Vec::new(),
        }));
    }

    pub(super) fn begin_copy_as(&mut self) {
        let Some(f) = self.panes.get(&self.focus).and_then(Pane::as_files) else {
            return;
        };
        let Some(e) = f.current().filter(|e| !e.is_parent()) else {
            self.status = "nothing to copy here".into();
            return;
        };
        let dir = e
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| f.cwd.clone());
        self.modal = Some(Modal::Input(Input {
            title: "Copy as".into(),
            label: format!("a second copy of {}, beside it", e.name),
            edit: LineEdit::for_rename(&e.name, e.is_dir()),
            kind: InputKind::CopyAs {
                from: e.path.clone(),
            },
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    pub(super) fn make_symlink(&mut self) {
        self.last_undo = Some(xdevcmd_core::undo::Undo::Cannot("making a link".into()));
        let Some(e) = self
            .panes
            .get(&self.focus)
            .and_then(Pane::as_files)
            .and_then(|f| f.current().cloned())
            .filter(|e| !e.is_parent())
        else {
            self.status = "nothing to link to here".into();
            return;
        };
        let Some(dest) = self.other_pane_cwd() else {
            self.status = "a link goes into the other pane \u{2014} there is none".into();
            return;
        };
        let link = dest.join(&e.name);
        match xdevcmd_core::attrs::symlink(&e.path, &link) {
            Ok(()) => {
                self.reload_all();
                self.status = format!("{} \u{2192} {}", tidy(&link), tidy(&e.path));
            }
            Err(err) => self.status = format!("no link made: {err}"),
        }
    }

    pub(super) fn begin_permissions(&mut self) {
        let targets = self.selection();
        let Some(first) = targets.first() else {
            self.status = "nothing selected".into();
            return;
        };
        let current = xdevcmd_core::attrs::current(first).unwrap_or_default();
        let many = if targets.len() > 1 {
            format!(" \u{b7} applies to all {}", targets.len())
        } else {
            String::new()
        };
        let label = format!("{}{many}", xdevcmd_core::attrs::describe(first));
        let dir = self.focused_cwd();
        self.modal = Some(Modal::Input(Input {
            title: "Permissions".into(),
            label,
            edit: LineEdit::for_rename(&current, true),
            kind: InputKind::Permissions { targets },
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    pub(super) fn begin_checksum(&mut self) {
        let Some(path) = self.current_file() else {
            self.status = "a checksum is of a file".into();
            return;
        };
        if !xdevcmd_core::policy::get().allows(&path) {
            self.status = "that file is outside the root xdevcmd is confined to".into();
            return;
        }
        // Not a FIFO or a device: reading one would never end, or never start.
        let special = std::fs::metadata(&path)
            .ok()
            .and_then(|m| xdevcmd_core::special_kind(&m.file_type()).map(str::to_string));
        if let Some(kind) = special {
            self.status = format!("that is a {kind}, not a file to checksum");
            return;
        }
        let (tx, rx) = mpsc::channel();
        let p = path.clone();
        std::thread::spawn(move || {
            let stop = std::sync::atomic::AtomicBool::new(false);
            let _ = tx.send(xdevcmd_core::sha256::file(&p, &stop));
        });
        self.status = format!(
            "working out the SHA-256 of {}\u{2026}",
            Self::describe(std::slice::from_ref(&path))
        );
        self.hashing = Some((path, rx));
    }

    pub(super) fn show_checksum(&mut self, path: &Path, result: std::io::Result<Option<String>>) {
        let name = Self::describe(&[path.to_path_buf()]);
        let hex = match result {
            Ok(Some(h)) => h,
            Ok(None) => return,
            Err(e) => {
                self.status = format!("cannot read {name}: {e}");
                return;
            }
        };
        let mut body = vec![name.clone(), String::new(), format!("SHA-256  {hex}")];
        // A published checksum beside the file, in the usual places.
        let dir = path.parent().unwrap_or(Path::new("."));
        let mut sidecars = vec![dir.join(format!("{name}.sha256"))];
        sidecars.extend(["SHA256SUMS", "sha256sums.txt", "SHA256SUMS.txt"].map(|f| dir.join(f)));
        for side in sidecars {
            let Ok(text) = std::fs::read_to_string(&side) else {
                continue;
            };
            let listed = text.lines().find(|l| {
                l.split_whitespace()
                    .nth(1)
                    .is_none_or(|n| n.trim_start_matches('*') == name)
            });
            let Some(line) = listed else { continue };
            let want = line
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            let side = side
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            body.push(String::new());
            body.push(if want == hex {
                format!("\u{2714} matches {side}")
            } else {
                format!("\u{2718} DOES NOT MATCH {side}: {want}")
            });
            break;
        }
        let bad = body.iter().any(|l| l.contains("DOES NOT MATCH"));
        self.status.clear();
        self.modal = Some(Modal::Ask(modal::Ask {
            title: "Checksum".into(),
            body,
            danger: bad,
            yes: Some(("copy to clipboard".into(), Cmd::CopyText(hex))),
            buttons: Vec::new(),
            also: None,
            footnote: None,
            pictures: Vec::new(),
        }));
    }
}
