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
pub mod controls;
pub mod debuglog;
pub mod dirs;
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
pub mod os;
pub mod paths;
pub mod plan;
pub mod policy;
pub mod protect;
pub mod rename;
pub mod sha256;
pub mod size;
pub mod sort;
pub mod syntax;
pub mod task;
pub mod termlog;
pub mod trashed;
pub mod undo;

pub use document::{Document, Mode};
pub use entry::{special_kind, split_name, Entry, Kind};
pub use listing::{entry_at, read_dir, read_dir_counted};
pub use ops::{Answer, Question, Request, Summary};
pub use sort::{sort_entries, Sort, SortKey};

/// The stack of a thread that walks a tree, one call per level: planning, copying,
/// counting. A thread's default (2 MiB) runs out near 1,500 levels, and running out is
/// an abort that ends the whole program. 256 MiB is address space reserved, not memory
/// used, and walks deeper than any path a system lets a program name (Windows' 32,767
/// characters is 16,000 levels of one letter and a separator).
pub const WALK_STACK: usize = 256 << 20;

/// A thread for work that walks a tree; see [`WALK_STACK`].
pub fn walk_thread<F>(f: F) -> std::thread::JoinHandle<()>
where
    F: FnOnce() + Send + 'static,
{
    std::thread::Builder::new()
        .stack_size(WALK_STACK)
        .spawn(f)
        .expect("a thread to walk a tree")
}
