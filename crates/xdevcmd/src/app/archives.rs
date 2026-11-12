//! Archives: what is in focus, extract, pack, and viewing a file inside one.

use super::*;

impl App {
    pub(super) fn focused_archive(&self) -> Option<&crate::pane::files::ArchiveView> {
        self.panes
            .get(&self.focus)
            .and_then(Pane::as_files)
            .and_then(|f| f.archive.as_ref())
    }

    pub(super) fn confirm_extract(
        &mut self,
        file: PathBuf,
        inner: Vec<String>,
        prefix: String,
        dest: PathBuf,
        choices: Vec<PathBuf>,
    ) {
        if inner.is_empty() {
            self.status = "nothing selected".into();
            return;
        }
        // Browsing it, the listing is in memory already; from outside (Alt+F9) it is
        // counted on a thread, as F5's count is, so a big .tar.gz does not freeze.
        let open = self.focused_archive().filter(|a| a.file == file).map(|a| {
            xdevcmd_core::archive::expand(&a.listing().items, &inner)
                .iter()
                .filter(|i| i.kind == xdevcmd_core::archive::ItemKind::File)
                .fold((0u64, 0u64), |(n, b), i| (n + 1, b + i.size))
        });
        let (count_line, counting) = match open {
            Some((n, bytes)) => (Self::count_label(n, bytes), None),
            None => {
                let (f, chosen) = (file.clone(), inner.clone());
                let work = move |stop: &std::sync::atomic::AtomicBool| {
                    xdevcmd_core::archive::count_until(&f, &chosen, stop)
                };
                (COUNTING.to_string(), Some((ops::Count::spawn(work), 3)))
            }
        };
        let what = match inner.as_slice() {
            [all] if all.is_empty() => "everything".to_string(),
            [one] => one.rsplit('/').next().unwrap_or(one).to_string(),
            many => format!("{} items", many.len()),
        };
        let name = file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let request = Request::extract(file, inner, prefix, dest.clone());
        self.modal = Some(Modal::Confirm(Confirm {
            title: "Extract".into(),
            body: vec![
                format!("{what}  from {name}"),
                String::new(),
                format!("to    {}", dest.display()),
                count_line,
            ],
            danger: false,
            request,
            buttons: Vec::new(),
            choices,
            choice: 0,
            source_dir: None,
            counting,
            permanent: false,
            existing_line: None,
        }));
        if let Some(Modal::Confirm(c)) = self.modal.as_mut() {
            c.set_destination(0);
        }
    }

    /// Alt+F9 on an archive: everything in it, into the other pane.
    pub(super) fn begin_unpack(&mut self) {
        let Some(file) = self
            .current_file()
            .filter(|p| xdevcmd_core::archive::format_of(p).is_some())
        else {
            self.status =
                "Alt+F9 unpacks a .zip, .tar or .tar.gz \u{2014} the cursor is not on one".into();
            return;
        };
        if !xdevcmd_core::policy::get().allows(&file) {
            self.status = "that archive is outside the root xdevcmd is confined to".into();
            return;
        }
        // The whole archive, named by the empty path: listing it to find its top level
        // would be the freeze this avoids.
        let top = vec![String::new()];
        let choices = self.transfer_targets();
        let Some(dest) = choices.first().cloned() else {
            self.status = "unpacking goes into another pane \u{2014} Alt+S splits one".into();
            return;
        };
        self.confirm_extract(file, top, String::new(), dest, choices);
    }

    /// Alt+F5: the selection into a new archive, in the other pane by default.
    pub(super) fn begin_pack(&mut self) {
        let sources = self.selection();
        if sources.is_empty() {
            self.status = "nothing selected".into();
            return;
        }
        let dir = self.other_pane_cwd().unwrap_or_else(|| self.focused_cwd());
        let stem = match sources.as_slice() {
            [one] => one
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            _ => self
                .focused_cwd()
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "archive".into()),
        };
        // The name selected, so typing replaces it; the extension is the format.
        let edit = LineEdit::for_rename(&format!("{stem}.zip"), false);
        self.modal = Some(Modal::Input(Input {
            title: "Pack".into(),
            label: format!(
                "{} into {} \u{b7} .zip .tar .tar.gz",
                Self::describe(&sources),
                dir.display()
            ),
            edit,
            kind: InputKind::Pack { sources },
            dir,
            buttons: Vec::new(),
            error: None,
        }));
    }

    /// F3 inside an archive: the one entry extracted to a private temporary directory,
    /// viewed, and removed when the viewer closes.
    pub(super) fn view_in_archive(&mut self) {
        const LIMIT: u64 = 256 << 20;
        let Some(a) = self.focused_archive() else {
            return;
        };
        let (file, prefix) = (a.file.clone(), a.prefix.clone());
        let Some(e) = self
            .panes
            .get(&self.focus)
            .and_then(Pane::as_files)
            .and_then(|f| f.current().cloned())
            .filter(|e| !e.is_dir())
        else {
            self.status = "F3 views a file, not a directory".into();
            return;
        };
        if e.size > LIMIT {
            let k = self.footer_key(Action::Copy).unwrap_or_default();
            self.status = format!(
                "{} is too big to view from inside \u{2014} {k} extracts it",
                e.name
            );
            return;
        }
        let inner = self
            .focused_archive()
            .map(|a| a.inner(std::slice::from_ref(&e.path)))
            .unwrap_or_default();
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let Some(tmp) = crate::tempfiles::file(&format!("view-{n}")) else {
            self.status = "cannot make a temporary directory".into();
            return;
        };
        let made = {
            // Mutable only where there is a mode to set.
            #[cfg_attr(not(unix), allow(unused_mut))]
            let mut b = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                b.mode(0o700);
            }
            b.create(&tmp)
        };
        if let Err(err) = made {
            self.status = format!("cannot make a temporary directory: {err}");
            return;
        }
        // The guard on writes is the engine's; viewing is reading, so it is allowed
        // in read-only mode — into a directory xdevcmd made for it and removes after.
        // A small archive is read at once; a big one on a thread, since reaching one
        // entry of a .tar.gz means decompressing everything before it.
        let small = std::fs::metadata(&file).map_or(true, |m| m.len() < (8 << 20));
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let name = e.name.clone();
        if small {
            let out =
                xdevcmd_core::archive::extract(&file, &inner, &prefix, &tmp, &mut ViewHooks(stop));
            return self.show_extracted(out, tmp, name);
        }
        let (tx, rx) = mpsc::channel();
        let (dir, flag) = (tmp.clone(), stop.clone());
        std::thread::spawn(move || {
            let out = xdevcmd_core::archive::extract(
                &file,
                &inner,
                &prefix,
                &dir,
                &mut ViewHooks(flag.clone()),
            );
            // Given up on: nobody will view it, so nothing is left behind either.
            if flag.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = std::fs::remove_dir_all(&dir);
            }
            let _ = tx.send(out);
        });
        self.status = format!("extracting {name} to view it\u{2026} Esc stops it");
        self.pending_view = Some((rx, tmp, name, stop));
    }

    /// Open the viewer on an entry extracted for it, or say why not.
    pub(super) fn show_extracted(&mut self, out: std::io::Result<()>, tmp: PathBuf, name: String) {
        let path = tmp.join(&name);
        match out.and_then(|_| Document::open(&path)) {
            Ok(doc) => {
                let v = &self.settings.config.viewer;
                let viewer = Viewer::open(doc, v.wrap, v.tab_width).with_syntax(
                    crate::viewer::colours_on(v.highlight),
                    crate::viewer::COLOUR_MAX,
                    true,
                );
                self.modal = Some(Modal::View(Box::new(viewer)));
                self.view_temp = Some(tmp);
                self.status.clear();
            }
            Err(err) => {
                let _ = std::fs::remove_dir_all(&tmp);
                self.status = format!("cannot view {name}: {err}");
            }
        }
    }
}
