pub mod files;
pub mod tabs;
pub mod term;
pub mod terms;

pub use files::FilePane;
pub use tabs::FileTabs;
pub use term::TermPane;
pub use terms::TermPanel;

/// One cell of the layout tree.
pub enum Pane {
    /// A file pane: one or more tabs, each a directory listing.
    Files(Box<FileTabs>),
    /// The terminal panel: tabs of shells, each tab with its own splits.
    Term(Box<TermPanel>),
}

impl Pane {
    pub fn files(first: FilePane) -> Self {
        Pane::Files(Box::new(FileTabs::new(first)))
    }

    /// The file pane's tab on show. Nearly everything wants this rather than the tabs.
    pub fn as_files_mut(&mut self) -> Option<&mut FilePane> {
        match self {
            Pane::Files(f) => Some(f.active_mut()),
            Pane::Term(_) => None,
        }
    }

    pub fn as_files(&self) -> Option<&FilePane> {
        match self {
            Pane::Files(f) => Some(f.active()),
            Pane::Term(_) => None,
        }
    }

    pub fn as_tabs_mut(&mut self) -> Option<&mut FileTabs> {
        match self {
            Pane::Files(f) => Some(f),
            Pane::Term(_) => None,
        }
    }

    pub fn as_tabs(&self) -> Option<&FileTabs> {
        match self {
            Pane::Files(f) => Some(f),
            Pane::Term(_) => None,
        }
    }

    pub fn is_term(&self) -> bool {
        matches!(self, Pane::Term(_))
    }
}
