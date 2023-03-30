use std::fs;
use std::io;
use std::{path::PathBuf, str::FromStr};

use anyhow::{Context, Result};

use serde::{Deserialize, Serialize};

use crate::config::User;
use crate::{
    config::{Clone, Remote},
    util::{self, Shell, DAY, HOUR, WEEK},
};

pub type Epoch = u64;
pub type Rank = f64;

#[derive(Debug, Deserialize, Serialize)]
pub struct Repo {
    pub remote: String,
    pub name: String,
    pub path: String,

    pub last_accessed: Epoch,
    pub accessed: Rank,
}

impl Repo {
    pub fn score(&self, now: Epoch) -> Rank {
        let duration = now.saturating_sub(self.last_accessed);
        if duration < HOUR {
            self.accessed * 4.0
        } else if duration < DAY {
            self.accessed * 2.0
        } else if duration < WEEK {
            self.accessed * 0.5
        } else {
            self.accessed * 0.25
        }
    }

    pub fn path<S>(&self, workspace: S) -> Result<PathBuf>
    where
        S: AsRef<str>,
    {
        let buf = if !self.path.is_empty() {
            PathBuf::from_str(&self.path)
        } else {
            PathBuf::from_str(workspace.as_ref())
        };
        match buf {
            Ok(buf) => {
                if self.path.is_empty() {
                    Ok(buf.join(&self.remote).join(&self.name))
                } else {
                    Ok(buf)
                }
            }
            Err(err) => Err(err).context("could not parse repo path"),
        }
    }

    pub fn ensure_path(&self, workspace: impl AsRef<str>, remote: &Remote) -> Result<PathBuf> {
        let path = self.path(workspace.as_ref())?;
        match fs::read_dir(&path) {
            Ok(_) => Ok(path),
            Err(err) if err.kind() == io::ErrorKind::NotFound => match &remote.clone {
                Some(clone) => {
                    self.ensure_clone(clone, &path, &remote.user)?;
                    Ok(path)
                }
                None => {
                    self.ensure_create(&remote, &path)?;
                    Ok(path)
                }
            },
            Err(err) => Err(err)
                .with_context(|| format!("could not read repository directory {}", path.display())),
        }
    }

    fn ensure_clone(&self, clone: &Clone, path: &PathBuf, user: &Option<User>) -> Result<()> {
        let url = self.clone_url(clone);

        let path = util::path_to_str(path)?;

        let mut git = Shell::git();
        git.arg("clone").args([url.as_str(), path]).exec()?;

        if let Some(user) = user {
            Shell::git()
                .with_git_path(path)
                .args(["config", "user.name"])
                .arg(&user.name)
                .exec()?;
            Shell::git()
                .with_git_path(path)
                .args(["config", "user.email"])
                .arg(&user.email)
                .exec()?;
        }

        Ok(())
    }

    fn ensure_create(&self, remote: &Remote, path: &PathBuf) -> Result<()> {
        fs::create_dir_all(&path).with_context(|| {
            format!("unable to create repository directory: {}", path.display())
        })?;
        let path_str = util::path_to_str(path)?;
        Shell::git().with_git_path(path_str).arg("init").exec()?;
        if let Some(script) = &remote.on_create {
            let lines: Vec<&str> = script.split("\n").collect();
            for line in lines {
                if line.is_empty() {
                    continue;
                }
                let mut bash = Shell::bash(line);

                bash.env("REPO_NAME", &self.name);
                bash.env("REMOTE", &remote.name);

                bash.with_path(path);

                bash.exec()?;
            }
        }
        Ok(())
    }

    pub fn clone_url(&self, cfg: &Clone) -> String {
        let mut ssh = cfg.use_ssh;
        if !ssh && cfg.ssh_groups != "" {
            let (group, _) = util::split_name(&self.name);
            if let Some(_) = cfg.ssh_groups.split(';').find(|s| s == &group) {
                ssh = true;
            }
        }

        if ssh {
            format!("git@{}:{}.git", cfg.domain, self.name)
        } else {
            format!("https://{}/{}.git", cfg.domain, self.name)
        }
    }
}
