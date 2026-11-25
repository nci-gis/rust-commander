//! Copy, move and delete, run on a worker thread.
//!
//! The rules this file exists to keep (see .agents/context/project.md): nothing destructive happens
//! silently, every operation is cancellable part-way through, and a filesystem that
//! misbehaves produces a question rather than a panic or a half-finished tree with no
//! explanation.
//!
//! The worker owns the filesystem work and sends [`Event`]s. When it needs a decision it
//! sends [`Event::Ask`] and *blocks* until an [`Answer`] comes back, so the UI is the only
//! thing that ever decides to overwrite or delete.

use std::fs::{self, File, FileTimes};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

const BUF: usize = 256 * 1024;
/// A move across devices removes sources a batch at a time: this many files, or
/// [`MOVE_BATCH_BYTES`] of them, whichever comes first.
const MOVE_BATCH_FILES: usize = 64;
const MOVE_BATCH_BYTES: u64 = 64 << 20;
/// Group commit (Round 033): a replaced file this small, on a local drive, waits in a
/// batch for one parallel sync of the batch instead of its own sync on the way.
const GROUP_FILE_MAX: u64 = 1 << 20;
/// A batch is synced at this many bytes...
const GROUP_BATCH_BYTES: u64 = 16 << 20;
/// ...or once its first file has waited this long, so the bar and the files keep moving.
const GROUP_BATCH_AGE: std::time::Duration = std::time::Duration::from_secs(1);
/// After this many batched files the job tries other thread counts on its next files
/// (the probe), and keeps the fastest. Smaller jobs keep the system's default.
const GROUP_PROBE_AFTER: u64 = 1000;
/// The thread counts the probe tries (the owner: 2 rather than 1, at most 32).
const GROUP_PROBE: [usize; 4] = [2, 8, 16, 32];
/// B3: new files under their real names wait for one parallel sync of their batch,
/// this many files (each holds its handle open until then)...
const FRESH_BATCH_FILES: usize = 128;
/// ...or this many bytes, or once the first has waited [`GROUP_BATCH_AGE`].
const FRESH_BATCH_BYTES: u64 = 64 << 20;
/// A big file copied under a temporary name is synced every this many bytes and the
/// journal told how far it got, so a run that dies goes on from there (Round 033): at
/// most this much is copied again. A file no bigger has no checkpoint.
pub const CHECKPOINT: u64 = 256 << 20;
/// Report at most this often, so a directory of small files does not drown the UI.
const PROGRESS_STEP: u64 = 1 << 20;
/// Bytes a `copy_file_range` step asks the kernel for: progress and Esc are seen
/// between steps (item 22).
#[cfg(any(target_os = "linux", target_os = "android"))]
const KERNEL_CHUNK: usize = 4 << 20;
/// And at least this often while anything moves.
const PROGRESS_EVERY: std::time::Duration = std::time::Duration::from_millis(250);

mod copy;
mod part;
mod remove;
mod request;
mod summary;
mod worker;

pub(crate) use part::*;
pub use remove::*;
pub use request::*;
pub use summary::*;
pub use worker::*;

#[cfg(test)]
mod tests;
