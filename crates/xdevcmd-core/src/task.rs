//! A piece of work on a thread of its own, with one result: a checksum, an entry
//! extracted to view, a picture decoded, a compare of two trees. One type for them
//! (Round 035, W8): each had its own channel and stop flag, and most read the result
//! with `try_recv().ok()`, so a worker that ended without one (a panic) was waited for
//! forever, as H-10's copy job was. `poll` tells that end apart.
//!
//! A file job (copy, move, delete) is `ops::Handle`: it asks questions and reports as
//! it goes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;

/// Where a task is.
#[derive(Debug, PartialEq, Eq)]
pub enum Poll<T> {
    /// Still working.
    Pending,
    /// Its result.
    Done(T),
    /// It ended without one: its thread panicked. Say so, and stop waiting.
    Died,
}

pub struct Task<T> {
    rx: Receiver<T>,
    stop: Arc<AtomicBool>,
}

impl<T: Send + 'static> Task<T> {
    /// `work` on a thread of its own, given the stop flag to look at now and then (or
    /// to hand on: an archive's hooks keep it).
    pub fn spawn(work: impl FnOnce(&Arc<AtomicBool>) -> T + Send + 'static) -> Self {
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        std::thread::spawn(move || {
            let _ = tx.send(work(&flag));
        });
        Self { rx, stop }
    }

    /// Its result if it is in; read once.
    pub fn poll(&self) -> Poll<T> {
        match self.rx.try_recv() {
            Ok(v) => Poll::Done(v),
            Err(TryRecvError::Empty) => Poll::Pending,
            Err(TryRecvError::Disconnected) => Poll::Died,
        }
    }

    /// Ask it to stop at its next look at the flag.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn settle<T: Send + 'static>(t: &Task<T>) -> Poll<T> {
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            match t.poll() {
                Poll::Pending if Instant::now() < end => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                p => return p,
            }
        }
    }

    #[test]
    fn its_result_comes_once() {
        let t = Task::spawn(|_| 42);
        assert_eq!(settle(&t), Poll::Done(42));
        assert_eq!(t.poll(), Poll::Died, "nothing more will come");
    }

    #[test]
    fn a_worker_that_panics_is_seen_to_have_died() {
        let t: Task<u8> = Task::spawn(|_| panic!("the worker died"));
        assert_eq!(settle(&t), Poll::Died);
    }

    #[test]
    fn stop_is_seen_by_the_work() {
        let t = Task::spawn(|stop| {
            let end = Instant::now() + Duration::from_secs(5);
            while !stop.load(Ordering::Relaxed) && Instant::now() < end {
                std::thread::sleep(Duration::from_millis(2));
            }
            stop.load(Ordering::Relaxed)
        });
        t.stop();
        assert_eq!(settle(&t), Poll::Done(true));
    }
}
