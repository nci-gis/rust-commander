//! Filesystem engine for xdevcmd.
//!
//! Deliberately knows nothing about terminals, ratatui or rendering: everything here is
//! testable without a TTY, and a future GUI would reuse this crate unchanged.

// No unsafe here, and the compiler keeps it that way. Any unsafe code xdevcmd runs is in
// a dependency, where cargo-deny's advisory check is watching.
#![forbid(unsafe_code)]

pub mod archive;
pub mod attrs;
pub mod batch_rename;
pub mod color;
pub mod compare;
pub mod debuglog;
pub mod document;
pub mod entry;
pub mod fastcopy;
#[doc(hidden)]
pub mod faults;
pub mod find;
pub mod format;
pub mod found;
pub mod fuzzy;
pub mod glob;
pub mod image;
pub mod journal;
pub mod listing;
pub mod name_mask;
pub mod noreplace;
pub mod ops;
pub mod paths;
pub mod plan;
pub mod policy;
pub mod rename;
pub mod sha256;
pub mod size;
pub mod sort;
pub mod syntax;
pub mod termlog;
pub mod trashed;
pub mod undo;

pub use document::{Document, Mode};
pub use entry::{special_kind, split_name, Entry, Kind};
pub use listing::{entry_at, read_dir, read_dir_counted};
pub use ops::{Answer, Question, Request, Summary};
pub use sort::{sort_entries, Sort, SortKey};
