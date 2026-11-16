//! Why xdevcmd did not do what was asked, and the next safe thing to do (plan A10).
//!
//! Each refusal has a stable `code()`, the same for a person and, later, for an agent
//! (MCP): the status text may change, the code does not. `next()` names what to press;
//! every code has a test that presses it and sees the action go through. The debug
//! log gets one line per refusal, `refused code=… next=…`, with no names.

/// A refusal, with what to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Alt+C with text typed in the shell and not run: done in a new shell instead,
    /// the typed text left as it is (#1).
    DirtyLine,
    /// The same, with every shell tab in use: nothing typed (#1).
    Busy,
}

impl Refusal {
    /// Stable: an agent and a test match on it.
    pub fn code(self) -> &'static str {
        match self {
            Refusal::DirtyLine => "dirty_line",
            Refusal::Busy => "busy",
        }
    }

    /// What to press next, or `None` when the refusal already did the safe thing.
    pub fn next(self) -> Option<&'static str> {
        match self {
            Refusal::DirtyLine => None,
            // In the shell: clears its line. Windows' consoles do not all clear it
            // with Ctrl+C (known issue): closing a shell tab makes room there.
            Refusal::Busy if cfg!(unix) => Some("Ctrl+C"),
            Refusal::Busy => Some("close_tab"),
        }
    }

    /// One line in the debug log, no names.
    pub fn log(self) {
        if xdevcmd_core::debuglog::on(xdevcmd_core::debuglog::Topic::Errors) {
            xdevcmd_core::debuglog::Event::new("refused")
                .word("code", self.code())
                .word("next", self.next().unwrap_or("done"))
                .write();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Codes are part of the interface: a change here is a change for agents too.
    #[test]
    fn codes_and_next_keys_are_stable() {
        let all = [Refusal::DirtyLine, Refusal::Busy];
        let got: Vec<_> = all.iter().map(|r| (r.code(), r.next())).collect();
        let busy_next = if cfg!(unix) { "Ctrl+C" } else { "close_tab" };
        assert_eq!(got, vec![("dirty_line", None), ("busy", Some(busy_next))]);
    }
}
