//! Windows-only conveniences, spelled the same way where they do not exist.
//!
//! `creation_flags` is how a child process is told not to open a console
//! window. On a system with no such concept the call still has to compile,
//! or every shared call site needs its own `cfg`, so here it does nothing.
#![cfg(not(target_os = "windows"))]

pub trait CmdExt {
    fn creation_flags(&mut self, flags: u32) -> &mut Self;
}

impl CmdExt for std::process::Command {
    fn creation_flags(&mut self, _flags: u32) -> &mut Self {
        self
    }
}
