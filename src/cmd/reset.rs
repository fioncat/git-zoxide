use anyhow::Result;

use crate::cmd::Reset;
use crate::cmd::Run;
use crate::util;
use crate::util::GitBranch;
use crate::util::GitRemote;
use crate::util::Shell;

impl Run for Reset {
    fn run(&self) -> Result<()> {
        GitBranch::ensure_no_uncommitted()?;
        let remote = GitRemote::build(self.upstream)?;
        let target = match util::option_arg(&self.args) {
            Some(branch) => remote.target(Some(branch))?,
            None => {
                if !self.upstream && !self.default {
                    let current = GitBranch::current()?;
                    remote.target(Some(&current))?
                } else {
                    remote.target(None)?
                }
            }
        };

        Shell::git()
            .args(["reset", "--hard", target.as_str()])
            .exec()?;

        Ok(())
    }
}
