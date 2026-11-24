use super::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    pub done: u64,
    /// A checked resume ([`Request::verify`]): files that looked identical but held
    /// other bytes than their source, so were copied again. Also counted in `done`.
    pub repaired: u64,
    pub skipped: u64,
    /// Of `skipped`: a file and a folder of the same name, which never replace each
    /// other (#5). Said, and the Job box stays open for it.
    pub kind_skips: u64,
    pub failed: u64,
    pub bytes: u64,
    pub cancelled: bool,
    pub errors: Vec<String>,
    /// A delete to the trash: where each item went, for undo.
    pub trashed: Vec<crate::trashed::Trashed>,
    /// A continued move: sources left where they were, because an identical copy was
    /// already at the destination (a move across drives killed between the copy and
    /// the removal of its source). Never removed without a question.
    pub left: u64,
    /// Safe operations were on but this job's journal could not be made: why. The job ran
    /// as with safe operations off; a kill now is not tidied up at the next start.
    pub journal_off: Option<String>,
    /// A copy: what it made, in the order made (a directory before what is in it), for
    /// its undo. Empty past [`MADE_CAP`], with `made_over` set.
    pub made: Vec<Made>,
    pub made_over: bool,
    /// A copy: files it replaced with no trash to keep the old one in. Undo leaves those.
    pub replaced_for_good: u64,
    /// Files replaced whose old version went to the trash (Round 031).
    pub replaced_to_trash: u64,
    /// The job's worker stopped on an internal error (a panic) and never said how it
    /// ended: what it did is not all listed here (`made` among it). Set by the side
    /// that saw it go; a worker never sets it.
    pub stopped: bool,
    /// A move: each source that went, and where it is now (the sources the request
    /// named, not what is under them). Lists kept in the app follow them (Round 031).
    pub relocated: Vec<(PathBuf, PathBuf)>,
    /// Files whose bytes the platform's fast copy moved (item 22): cloned by the drive,
    /// and copied by the kernel (`copy_file_range`). The rest, xdevcmd read and wrote.
    pub cloned: u64,
    pub by_kernel: u64,
}

impl Summary {
    pub fn describe(&self, kind: Kind) -> String {
        let verb = match kind {
            Kind::Copy => "copied",
            Kind::Move => "moved",
            Kind::Delete => "deleted",
            Kind::Extract => "extracted",
            Kind::Pack => "packed",
        };
        // What check C found comes first and whole: the status line elides the middle
        // of a long line, and the job box cuts its end (hand test, 2026-11).
        let mut s = if self.repaired > 0 {
            format!("{} found different and copied again; ", self.repaired)
        } else {
            String::new()
        };
        s.push_str(&format!("{} {verb}", self.done));
        // The engine that ran (item 22), when it was not xdevcmd's own loop: named
        // alone when it did every file, counted when engines were mixed.
        let fast = match (self.cloned, self.by_kernel) {
            (0, 0) => None,
            (n, 0) if n == self.done => Some("cloned".to_string()),
            (0, n) if n == self.done => Some("by the kernel".to_string()),
            (0, k) => Some(format!("{k} by the kernel")),
            (c, 0) => Some(format!("{c} cloned")),
            (c, k) => Some(format!("{c} cloned, {k} by the kernel")),
        };
        if let Some(f) = fast {
            s.push_str(&format!(" ({f})"));
        }
        if self.skipped > 0 {
            s.push_str(&format!(", {} skipped", self.skipped));
            // #5: said every time, so a file and a folder of one name are never a
            // silent skip.
            if self.kind_skips > 0 {
                s.push_str(&format!(
                    " ({} a file and a folder of the same name: never replaced)",
                    self.kind_skips
                ));
            }
        }
        if self.failed > 0 {
            s.push_str(&format!(", {} failed", self.failed));
        }
        if self.replaced_to_trash > 0 {
            s.push_str(&format!(
                ", {} replaced (the old {} in the trash)",
                self.replaced_to_trash,
                if self.replaced_to_trash == 1 {
                    "one"
                } else {
                    "ones"
                }
            ));
        }
        if self.left > 0 {
            s.push_str(&format!(
                ", {} left where {} {}: already there",
                self.left,
                if self.left == 1 { "it" } else { "they" },
                if self.left == 1 { "was" } else { "were" },
            ));
        }
        if self.cancelled {
            s.push_str(" (cancelled)");
        }
        if let Some(why) = &self.journal_off {
            s.push_str(&format!("; safe operations off for this job: {why}"));
        }
        s
    }
}
