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
        let branches = GitBranch::list().context("unable to list branch")?;
        if self.sync {
            return self.sync(&branches);
        }
        if self.args.is_empty() {
            self.show(&branches);
            return Ok(());
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
        GitBranch::ensure_no_uncommitted()?;
        self.fetch()?;
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
                    print!("{} {} {} ", style("+").green(), op, style(branch).magenta())
                }
                SyncBranchTask::Delete(branch) => {
                    print!("{} delete {} ", style("-").red(), style(branch).magenta())
                }
            }
        }
        println!();
        println!();
        util::confirm("do you want to process the synchronization")?;

        println!();
        for task in &tasks {
            match task {
                SyncBranchTask::Sync(op, branch) => {
                    if &current != branch {
                        // checkout to this branch to perform push/pull
                        Shell::git().args(["checkout", branch]).output()?;
                        current = branch;
                    }
                    Shell::git().arg(op).output()?;
                }
                SyncBranchTask::Delete(branch) => {
                    if &current == branch {
                        // we cannot delete branch when we are inside it, checkout
                        // to default branch first.
                        Shell::git().args(["checkout", default.as_str()]).output()?;
                        current = branch;
                    }
                    Shell::git().args(["branch", "-D", branch]).output()?;
                }
            }
        }
        if current != back {
            Shell::git().args(["checkout", back]).output()?;
        }

        Ok(())
    }

    fn fetch(&self) -> Result<()> {
        let mut git = Shell::git();
        git.args(["fetch", "--prune"]);
        git.exec()
    }
}
