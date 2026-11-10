//! Pictures: quick view, terminal graphics (Kitty, iTerm2, sixel) and cell sizes.

use super::*;

impl App {
    /// Pictures may be drawn by the terminal itself: worth asking it, at start-up, what
    /// it draws and how big its cells are (also when a protocol is chosen by hand, for
    /// the cell size).
    #[cfg(unix)]
    pub fn settings_images_probe(&self) -> bool {
        self.settings.config.viewer.images != "blocks"
    }

    /// Draw the quick view into `area`: the file under the cursor of the focused pane.
    pub(super) fn draw_quick_view(&mut self, frame: &mut Frame, area: Rect) {
        const LIMIT: u64 = 64 << 20;
        let cur = self
            .panes
            .get(&self.focus)
            .and_then(Pane::as_files)
            .and_then(|f| f.current().cloned());
        let Some((_, shown)) = self.quick_view.as_mut() else {
            return;
        };
        let note = |frame: &mut Frame, lines: Vec<String>| {
            let inner = modal::frame_box(frame, area, "quick view", theme::t().accent);
            for (i, l) in lines.iter().enumerate() {
                modal::put(
                    frame,
                    inner,
                    i as u16,
                    l,
                    Style::new().fg(theme::t().fg_dim),
                );
            }
        };
        let Some(e) = cur.filter(|e| !e.is_parent()) else {
            *shown = None;
            return note(frame, vec!["nothing under the cursor".into()]);
        };
        if e.is_dir() {
            *shown = None;
            return note(
                frame,
                vec![
                    e.name.clone(),
                    "a directory \u{2014} Space works out its size".into(),
                ],
            );
        }
        if e.size > LIMIT {
            *shown = None;
            return note(
                frame,
                vec![
                    e.name.clone(),
                    format!(
                        "{} \u{2014} too big to preview, F3 views it",
                        human_size(e.size)
                    ),
                ],
            );
        }
        if !xdevcmd_core::policy::get().allows(&e.path) {
            *shown = None;
            return note(
                frame,
                vec!["outside the root xdevcmd is confined to".into()],
            );
        }
        if xdevcmd_core::image::is_image(&e.path) {
            *shown = None;
            return self.draw_picture(frame, area, &e.path);
        }
        if shown.as_ref().is_none_or(|(p, _)| *p != e.path) {
            *shown = match Document::open(&e.path) {
                Ok(doc) => {
                    let v = &self.settings.config.viewer;
                    Some((
                        e.path.clone(),
                        Box::new(Viewer::open(doc, v.wrap, v.tab_width)),
                    ))
                }
                Err(err) => {
                    return note(frame, vec![e.name.clone(), err.to_string()]);
                }
            };
        }
        if let Some((_, v)) = shown.as_mut() {
            v.render(frame, area);
        }
    }

    /// A picture in half blocks: each cell is two pixels, the upper as the
    /// foreground of `▀` and the lower as its background.
    pub(super) fn draw_picture(&mut self, frame: &mut Frame, area: Rect, path: &Path) {
        let th = theme::t();
        let inner = Rect::new(
            area.x + 1,
            area.y + 1,
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        );
        let bg = match th.bg_panel {
            ratatui::style::Color::Rgb(r, g, b) => [r, g, b],
            _ => [0, 0, 0],
        };
        // The terminal's own pictures are decoded at its resolution, a cell's pixels each;
        // half blocks at two pixels a cell.
        let own = self.graphics != crate::graphics::Protocol::Blocks && self.interactive;
        // Sixel is drawn pixel for pixel, so at the cell's own size. Kitty and iTerm2 fit
        // the picture to its cells themselves: sent at twice the density, it stays sharp
        // on a high-density screen and where the cell's size could only be guessed.
        let density = if self.graphics == crate::graphics::Protocol::Sixel {
            1
        } else {
            2
        };
        let (cw, ch) = (self.cell_px.0 * density, self.cell_px.1 * density);
        let size = if own {
            // At most 2048 pixels a side: what goes through SSH stays reasonable.
            let (w, h) = (
                inner.width.saturating_mul(cw).min(2048),
                inner.height.saturating_mul(ch).min(2048),
            );
            (w, h)
        } else {
            (inner.width, inner.height)
        };
        let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        let key = PicKey {
            path: path.to_path_buf(),
            px: size,
            mtime,
            inner: (inner.width, inner.height),
            protocol: if own {
                self.graphics
            } else {
                crate::graphics::Protocol::Blocks
            },
        };
        let stale = self.quick_image.as_ref().is_none_or(|(k, _)| *k != key);
        if stale {
            let (w, h) = if own {
                (size.0 as usize, size.1 as usize)
            } else {
                (inner.width as usize, inner.height as usize * 2)
            };
            if !self.interactive {
                // A screenshot has no later frame for the picture to arrive in.
                let pic = xdevcmd_core::image::load(path, w, h, bg).map_err(|e| e.to_string());
                let prepared = Prepared {
                    pic,
                    cells: (0, 0),
                    bytes: Vec::new(),
                };
                self.quick_image = Some((key.clone(), prepared));
            } else if !self.decoding.as_ref().is_some_and(|(k, _)| *k == key) {
                // Only once the cursor has stayed: a quick run down a directory of
                // photos decodes and sends none of those passed over.
                let since = match &self.settle {
                    Some((p, t)) if p == path => *t,
                    _ => {
                        let now = Instant::now();
                        self.settle = Some((path.to_path_buf(), now));
                        now
                    }
                };
                if since.elapsed() < SETTLE {
                    self.settling = true;
                } else {
                    self.settle = None;
                    let (tx, rx) = std::sync::mpsc::channel();
                    let job = key.clone();
                    let cell = (cw, ch);
                    std::thread::spawn(move || {
                        let _ = tx.send(prepare(&job, w, h, bg, cell));
                    });
                    self.decoding = Some((key.clone(), rx));
                }
            }
        }
        let current = self.quick_image.as_ref().filter(|(k, _)| *k == key);
        let Some((_, Prepared { pic, cells, .. })) = current else {
            let inner = modal::frame_box(frame, area, "image", th.accent);
            modal::put(
                frame,
                inner,
                0,
                "decoding\u{2026}",
                Style::new().fg(th.fg_dim),
            );
            return;
        };
        let cells = *cells;
        let title = match pic {
            Ok(p) => format!("image {}\u{d7}{}", p.source.0, p.source.1),
            Err(_) => "image".into(),
        };
        let inner = modal::frame_box(frame, area, &title, th.accent);
        let pic = match pic {
            Ok(p) => p,
            Err(e) => {
                modal::put(frame, inner, 0, e, Style::new().fg(th.err));
                return;
            }
        };
        if own {
            // The terminal draws it, after this frame (sync_graphics); not under a dialog,
            // where it would be drawn over the dialog, nor in a box too small to hold it.
            let (cols, rows) = cells;
            if self.modal.is_none()
                && cols > 0
                && rows > 0
                && cols <= inner.width
                && rows <= inner.height
            {
                self.graphic_want = Some(Graphic {
                    rect: Rect::new(
                        inner.x + (inner.width - cols) / 2,
                        inner.y + (inner.height - rows) / 2,
                        cols,
                        rows,
                    ),
                    key,
                });
            }
            return;
        }
        let rows = pic.height.div_ceil(2);
        let x0 = inner.x + (inner.width.saturating_sub(pic.width as u16)) / 2;
        let y0 = inner.y + (inner.height.saturating_sub(rows as u16)) / 2;
        modal::paint_picture(frame, x0, y0, pic, bg);
    }

    /// A cell's size in pixels, from the terminal, for decoding pictures at its own
    /// resolution. Terminals that do not say (Windows) keep the last guess.
    pub(super) fn measure_cells(&mut self, terminal: &mut Term) {
        use ratatui::backend::Backend;
        if let Ok(w) = terminal.backend_mut().window_size() {
            let (c, p) = (w.columns_rows, w.pixels);
            if c.width > 0 && c.height > 0 && p.width >= c.width && p.height >= c.height {
                self.cell_px = (p.width / c.width, p.height / c.height);
                self.cell_known = true;
            }
        }
        self.graphics = crate::graphics::effective(self.graphics_found, self.cell_known);
    }

    /// After a frame: put the picture it wants on screen in the terminal's own protocol,
    /// once, and take an old one away. The bytes were made with the decode
    /// ([`prepare`]): a frame only writes them. A Kitty picture is sent once and kept by
    /// the terminal: under a dialog only its placement goes, and it is placed again
    /// after; it is freed when another picture or none replaces it. An iTerm2 or sixel
    /// picture is cells: the screen is repainted over it, and it is written again.
    pub(super) fn sync_graphics(&mut self, terminal: &mut Term) -> Result<()> {
        use crate::graphics::{kitty_delete, kitty_place, kitty_unplace, Protocol};
        // Kept under a dialog, and the dialog has closed with nothing to show (quick
        // view closed meanwhile): freed, not left in the terminal.
        if self.graphic_want.is_none()
            && self.graphic_shown.is_none()
            && self.modal.is_none()
            && self.kitty_held.take().is_some()
            && self.graphics == Protocol::Kitty
        {
            terminal.backend_mut().raw(&kitty_delete(KITTY_ID))?;
        }
        if self.graphic_want == self.graphic_shown {
            return Ok(());
        }
        if let Some(old) = self.graphic_shown.take() {
            if self.graphics == Protocol::Kitty {
                // Kept while a dialog covers it, or when only its place changes.
                let same = self.graphic_want.as_ref().is_some_and(|w| w.key == old.key);
                if same || (self.graphic_want.is_none() && self.modal.is_some()) {
                    terminal.backend_mut().raw(&kitty_unplace(KITTY_ID))?;
                } else {
                    terminal.backend_mut().raw(&kitty_delete(KITTY_ID))?;
                    self.kitty_held = None;
                }
            } else {
                // An iTerm2 or sixel picture is cells: write again, through the backend,
                // what this frame has in the cells it covered (draw() kept them), which
                // ratatui's diff thinks are already on screen.
                use ratatui::backend::Backend;
                let under = std::mem::take(&mut self.under_graphic);
                terminal
                    .backend_mut()
                    .draw(under.iter().map(|(x, y, c)| (*x, *y, c)))?;
            }
        }
        let Some(want) = self.graphic_want.clone() else {
            return Ok(());
        };
        let Some((_, prepared)) = self
            .quick_image
            .as_ref()
            .filter(|(k, p)| *k == want.key && p.pic.is_ok() && !p.bytes.is_empty())
        else {
            return Ok(());
        };
        let r = want.rect;
        let mut bytes = format!("\x1b7\x1b[{};{}H", r.y + 1, r.x + 1).into_bytes();
        if self.graphics == Protocol::Kitty && self.kitty_held.as_ref() == Some(&want.key) {
            bytes.extend(kitty_place(KITTY_ID, r.width, r.height));
        } else {
            bytes.extend_from_slice(&prepared.bytes);
            if self.graphics == Protocol::Kitty {
                self.kitty_held = Some(want.key.clone());
            }
        }
        bytes.extend_from_slice(b"\x1b8");
        terminal.backend_mut().raw(&bytes)?;
        self.graphic_shown = Some(want);
        Ok(())
    }
}
