use std::fmt::{self, Display, Formatter};

/// Custom error type for early exit.
#[derive(Debug)]
pub struct SilentExit {
    pub code: u8,
}

impl Display for SilentExit {
    fn fmt(&self, _: &mut Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

pub const REPO_NO_UPSTREAM: &str =
    "this repo is not forked from another repo, so it has no upstream";
