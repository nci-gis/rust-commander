//! A backend that says nothing when nothing changed.
//!
//! ratatui draws a frame every tick, and when the frame is identical to the last one the
//! diff is empty — but the crossterm backend still writes a colour reset after it, and
//! the terminal still hides or moves the cursor. Measured at about 600 bytes a second
//! with rcmd sitting idle: harmless to a fast terminal, wasted work for one that redraws
//! on every byte, and noise over SSH. This wrapper drops an empty draw, and a cursor
//! hide, show or move that changes nothing.

use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};

pub struct Quiet<B> {
    inner: B,
    /// What the terminal was last told; `None` when it is not known.
    shown: Option<bool>,
    at: Option<Position>,
}

impl<B> Quiet<B> {
    pub fn new(inner: B) -> Self {
        Self {
            inner,
            shown: None,
            at: None,
        }
    }

    /// The terminal was used behind our back (an editor ran): assume nothing about it.
    pub fn forget(&mut self) {
        self.shown = None;
        self.at = None;
    }
}

impl<B: Backend> Backend for Quiet<B> {
    type Error = B::Error;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let mut content = content.peekable();
        if content.peek().is_none() {
            return Ok(());
        }
        // Writing cells moves the terminal's cursor.
        self.at = None;
        self.inner.draw(content)
    }

    fn append_lines(&mut self, n: u16) -> Result<(), Self::Error> {
        self.at = None;
        self.inner.append_lines(n)
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        if self.shown != Some(false) {
            self.inner.hide_cursor()?;
            self.shown = Some(false);
        }
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        if self.shown != Some(true) {
            self.inner.show_cursor()?;
            self.shown = Some(true);
        }
        Ok(())
    }

    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        let p = self.inner.get_cursor_position()?;
        self.at = Some(p);
        Ok(p)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error> {
        let p = position.into();
        if self.at != Some(p) {
            self.inner.set_cursor_position(p)?;
            self.at = Some(p);
        }
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.at = None;
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        self.at = None;
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> Result<Size, Self::Error> {
        self.inner.size()
    }

    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::CrosstermBackend;
    use ratatui::layout::Rect;
    use ratatui::text::Line;
    use ratatui::{Terminal, TerminalOptions, Viewport};

    /// Counts what the backend writes; ratatui keeps the writer itself private.
    #[derive(Clone, Default)]
    struct Count(std::rc::Rc<std::cell::Cell<usize>>);

    impl std::io::Write for Count {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.set(self.0.get() + b.len());
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Bytes written per frame, through the real crossterm backend.
    fn frames(draw: &[&str], cursor: Option<(u16, u16)>) -> Vec<usize> {
        // A fixed viewport never asks the (absent) terminal for its size.
        let options = TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, 20, 1)),
        };
        let count = Count::default();
        let backend = Quiet::new(CrosstermBackend::new(count.clone()));
        let mut t = Terminal::with_options(backend, options).unwrap();
        let mut sizes = Vec::new();
        let mut before = 0;
        for text in draw {
            t.draw(|f| {
                f.render_widget(Line::raw(*text), f.area());
                if let Some(c) = cursor {
                    f.set_cursor_position(c);
                }
            })
            .unwrap();
            let now = count.0.get();
            sizes.push(now - before);
            before = now;
        }
        sizes
    }

    #[test]
    fn an_unchanged_frame_writes_nothing() {
        let sizes = frames(&["hello", "hello", "hello", "bye"], None);
        assert!(sizes[0] > 0 && sizes[3] > 0, "{sizes:?}");
        assert_eq!(&sizes[1..3], &[0, 0], "{sizes:?}");
    }

    #[test]
    fn a_shown_cursor_that_stays_put_is_not_moved_again() {
        let sizes = frames(&["$ ", "$ ", "$ x"], Some((2, 0)));
        assert_eq!(sizes[1], 0, "{sizes:?}");
        assert!(
            sizes[2] > 0,
            "a change redraws and puts the cursor back: {sizes:?}"
        );
    }
}
