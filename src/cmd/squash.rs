use anyhow::bail;
use anyhow::Result;
use console::style;

use crate::cmd::Run;
use crate::cmd::Squash;
use crate::util;
use crate::util::GitBranch;
use crate::util::GitRemote;
use crate::util::Shell;

impl Run for Squash {
    fn run(&self) -> Result<()> {
        GitBranch::ensure_no_uncommitted()?;
        let remote = GitRemote::build(self.upstream)?;
        let target = remote.target(util::option_arg(&self.args))?;

        let commits = Self::commits_between(&target)?;
        if commits.is_empty() {
            bail!("no commit to squash")
        }
        if commits.len() == 1 {
            bail!(
                "only found one commit ahead {}, no need to squash",
                style(&target).yellow()
            )
        }

        println!();
        println!(
            "Found {} commits ahead {}:",
            style(commits.len()).yellow(),
            style(&target).yellow()
        );
        for commit in &commits {
            println!("  * {}", commit);
        }
        println!();
        util::confirm("continue")?;
        println!();

        let set = format!("HEAD~{}", commits.len());
        Shell::git()
            .args(["reset", "--soft", set.as_str()])
            .exec()?;

        let mut args = vec!["commit"];
        if let Some(msg) = &self.message {
            args.push("-m");
            args.push(msg);
        }
        Shell::git().args(&args).inherit().exec()?;

        Ok(())
    }
}

impl Squash {
    fn commits_between(target: &str) -> Result<Vec<String>> {
        let target = format!("HEAD...{}", target);
        let output = Shell::git()
            .args([
                "log",
                "--left-right",
                "--cherry-pick",
                "--oneline",
                target.as_str(),
            ])
            .exec()?;
        let commits: Vec<String> = output
            .split("\n")
            .filter(|line| {
                // If the commit message output by "git log xxx" does not start
                // with "<", it means that this commit is from the target branch.
                // Since we only list commits from current branch, ignore such
                // commits.
                line.trim().starts_with("<")
            })
            .map(|line| line.strip_prefix("<").unwrap().to_string())
            .collect();
        Ok(commits)
    }
}
