//! How deep a tree walk may go (beta.4, the owner, 2026-11-26: a hard limit per system,
//! measured, not a setting). Planning, copying, counting and moving walk a tree one call
//! per level, and a stack that runs out is an abort that ends the whole program. So a
//! walk asks [`deeper`] before it goes down a level; at the limit it does not, and says
//! the folder is too deep instead. The rest of the job goes on.
//!
//! The limit is per thread: a [`crate::walk_thread`] with its big stack takes
//! [`MAX_DEPTH`]; one that had to make do with the system's default stack (it would
//! not reserve the big one) takes [`FALLBACK_DEPTH`], and [`fell_back`] says so.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

/// The deepest a walk goes on a [`crate::walk_thread`]: above the deepest path each
/// system lets a program name with one-letter folders (Linux 4,096 bytes ≈ 2,048 levels,
/// macOS 1,024 ≈ 512, Windows 32,767 characters ≈ 16,383), so a real tree never meets
/// it. Stack per level, measured with `tests/deep_tree.rs` `measure_stack_per_level`
/// on Linux x86_64 (2026-11-26): planning 1,476 bytes (release) and 3,718 (debug),
/// copying 664 and 2,421. At the worst, 16,400 levels need 61 MB of
/// [`crate::WALK_STACK`]'s 256 MiB: over 4 times the room (11 in a release build).
#[cfg(target_os = "windows")]
pub const MAX_DEPTH: usize = 16_400;
#[cfg(target_os = "macos")]
pub const MAX_DEPTH: usize = 1_100;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub const MAX_DEPTH: usize = 4_100;

/// The deepest a walk goes on a default stack (2 MiB for a spawned thread): planning
/// runs out at 710 levels on 1 MiB in a release build, 282 in a debug one.
pub const FALLBACK_DEPTH: usize = 150;

thread_local! {
    static DEPTH: Cell<usize> = const { Cell::new(0) };
    static LIMIT: Cell<usize> = const { Cell::new(FALLBACK_DEPTH) };
}

static FELL_BACK: AtomicBool = AtomicBool::new(false);

/// Whether a walk thread had to run on the default stack, so its limit is
/// [`FALLBACK_DEPTH`]: the app says so once.
pub fn fell_back() -> bool {
    FELL_BACK.load(Ordering::Relaxed)
}

pub(crate) fn note_fell_back() {
    FELL_BACK.store(true, Ordering::Relaxed);
}

/// This thread's limit.
pub fn limit() -> usize {
    LIMIT.with(Cell::get)
}

pub(crate) fn set_limit(n: usize) {
    LIMIT.with(|l| l.set(n));
}

/// One level down, while held. `None` at the limit: do not go down.
#[must_use = "the level is left when this is dropped"]
pub struct Deeper(());

impl Drop for Deeper {
    fn drop(&mut self) {
        DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

/// Go one level down, or `None` if this thread is at its limit.
pub fn deeper() -> Option<Deeper> {
    DEPTH.with(|d| {
        if d.get() >= limit() {
            None
        } else {
            d.set(d.get() + 1);
            Some(Deeper(()))
        }
    })
}

/// What a walk says of a folder it did not go into.
pub fn too_deep() -> std::io::Error {
    std::io::Error::other(format!("too deep: more than {} levels", limit()))
}

/// Tests only: walk threads get this stack, and no limit, to measure where a walk
/// runs out (`tests/deep_tree.rs`). 0: as usual.
#[doc(hidden)]
pub static STACK_FOR_MEASURING: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Tests only: walk threads take this limit, to meet it without a deep tree. 0: as
/// usual.
#[doc(hidden)]
pub static LIMIT_FOR_TESTING: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_walk_goes_down_to_its_limit_and_no_further() {
        set_limit(3);
        let a = deeper().unwrap();
        let b = deeper().unwrap();
        let c = deeper().unwrap();
        assert!(deeper().is_none(), "at 3");
        drop(c);
        let c2 = deeper().expect("one level up again");
        drop((a, b, c2));
        assert!(deeper().is_some());
        set_limit(FALLBACK_DEPTH);
    }
}
