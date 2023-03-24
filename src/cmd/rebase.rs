use anyhow::Result;

use crate::cmd::Rebase;
use crate::cmd::Run;
use crate::util;
use crate::util::GitBranch;
use crate::util::GitRemote;
use crate::util::Shell;

impl Run for Rebase {
    fn run(&self) -> Result<()> {
        GitBranch::ensure_no_uncommitted()?;
        let remote = GitRemote::build(self.upstream)?;
        let target = remote.target(util::option_arg(&self.args))?;
        Shell::git().args(["rebase", target.as_str()]).exec()?;

        Ok(())
    }
}
