use std::{path::PathBuf, str::FromStr};

use anyhow::{Context, Result};

use serde::{Deserialize, Serialize};

use crate::util::{DAY, HOUR, WEEK};

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
}
