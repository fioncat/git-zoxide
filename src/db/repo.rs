use std::{path::PathBuf, str::FromStr};

use anyhow::{Context, Result};

use serde::{Deserialize, Serialize};

use crate::{
    config::Clone,
    util::{self, DAY, HOUR, WEEK},
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
                    Ok(buf.join(&self.name))
                } else {
                    Ok(buf)
                }
            }
            Err(err) => Err(err).context("could not parse repo path"),
        }
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
