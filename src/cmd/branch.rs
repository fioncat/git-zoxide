use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use console::style;
use pad::PadStr;

use crate::cmd::Branch;
use crate::cmd::Run;
use crate::util;
use crate::util::BranchStatus;
use crate::util::GitBranch;
use crate::util::Shell;

impl Run for Branch {
    fn run(&self) -> Result<()> {
        if self.sync {
            GitBranch::ensure_no_uncommitted()?;
            self.fetch()?;
        }
        let branches = GitBranch::list().context("unable to list branch")?;
        if self.sync {
            return self.sync(&branches);
        }
        if self.args.is_empty() {
            self.show(&branches);
            return Ok(());
        }
        if self.delete {
            return self.delete(&branches);
        }
        if self.args.is_empty() {
            bail!("require branch name")
        }
        let name = &self.args[0];
        if self.create {
            Shell::git().args(["checkout", "-b", name]).exec()?;
        } else {
            Shell::git().args(["checkout", name]).exec()?;
        }
        if self.push {
            Shell::git()
                .args(["push", "--set-upstream", "origin", name])
                .exec()?;
        }

        Ok(())
    }
}

enum SyncBranchTask<'a> {
    Sync(&'a str, &'a str),
    Delete(&'a str),
}

impl Branch {
    fn show(&self, branches: &Vec<GitBranch>) {
        if branches.is_empty() {
            return;
        }
        if !self.all {
            for branch in branches {
                println!("{}", branch.name);
            }
            return;
        }
        let pad = branches.iter().max_by_key(|s| s.name.len()).unwrap();
        let pad = pad.name.len();

        for branch in branches {
            println!(
                "{} {}",
                branch
                    .name
                    .as_str()
                    .pad_to_width_with_alignment(pad, pad::Alignment::Left),
                branch.status.display(),
            );
        }
    }

    fn sync(&self, branches: &Vec<GitBranch>) -> Result<()> {
        let default = GitBranch::default().context("unable to get default branch")?;

        let mut back = &default;
        let mut tasks: Vec<SyncBranchTask> = vec![];
        let mut current: &str = "";
        for branch in branches {
            if branch.current {
                current = branch.name.as_str();
                match branch.status {
                    BranchStatus::Gone => {}
                    _ => back = &branch.name,
                }
            }
            let task = match branch.status {
                BranchStatus::Ahead => Some(SyncBranchTask::Sync("push", branch.name.as_str())),
                BranchStatus::Behind => Some(SyncBranchTask::Sync("pull", branch.name.as_str())),
                BranchStatus::Gone => {
                    if branch.name == default {
                        // we cannot delete default branch
                        continue;
                    }
                    Some(SyncBranchTask::Delete(branch.name.as_str()))
                }
                _ => None,
            };
            if let Some(task) = task {
                tasks.push(task);
            }
        }

        println!();
        if tasks.is_empty() {
            println!("nothing to do");
            return Ok(());
        }

        println!("backup branch is {}", style(back).magenta());
        let word = if tasks.len() == 1 { "Task" } else { "Tasks" };
        println!("{} ({}):", word, tasks.len());
        for task in &tasks {
            match task {
                SyncBranchTask::Sync(op, branch) => {
                    println!("{} {} {} ", style("+").green(), op, style(branch).magenta())
                }
                SyncBranchTask::Delete(branch) => {
                    println!("{} delete {} ", style("-").red(), style(branch).magenta())
                }
            }
        }
        println!();
        util::confirm("do you want to process the synchronization")?;

        println!();
        for task in tasks {
            match task {
                SyncBranchTask::Sync(op, branch) => {
                    if current != branch {
                        // checkout to this branch to perform push/pull
                        Shell::git().args(["checkout", branch]).exec()?;
                        current = branch;
                    }
                    Shell::git().arg(op).exec()?;
                }
                SyncBranchTask::Delete(branch) => {
                    if current == branch {
                        // we cannot delete branch when we are inside it, checkout
                        // to default branch first.
                        Shell::git().args(["checkout", default.as_str()]).exec()?;
                        current = branch;
                    }
                    Shell::git().args(["branch", "-D", branch]).exec()?;
                }
            }
        }
        if current != back {
            Shell::git().args(["checkout", back]).exec()?;
        }

        Ok(())
    }

    fn fetch(&self) -> Result<()> {
        let mut git = Shell::git();
        git.args(["fetch", "--prune"]);
        git.exec()?;
        Ok(())
    }

    fn delete(&self, branches: &Vec<GitBranch>) -> Result<()> {
        let branch = match self.args.len() {
            0 => Self::must_get_current_branch(branches)?,
            _ => match branches.iter().find(|b| b.name.eq(&self.args[0])) {
                Some(b) => b,
                None => bail!("could not find branch {}", style(&self.args[0]).yellow()),
            },
        };

        if branch.current {
            GitBranch::ensure_no_uncommitted()?;
            let default = GitBranch::default()?;
            if branch.name.eq(&default) {
                bail!("could not delete default branch")
            }
            Shell::git().args(["checkout", default.as_str()]).exec()?;
        }

        Shell::git().args(["branch", "-D", &branch.name]).exec()?;
        if self.push {
            Shell::git()
                .args(["push", "origin", "--delete", &branch.name])
                .exec()?;
        }
        Ok(())
    }

    fn must_get_current_branch(branches: &Vec<GitBranch>) -> Result<&GitBranch> {
        match branches.iter().find(|b| b.current) {
            Some(b) => Ok(b),
            None => bail!("could not find current branch"),
        }
    }
}
