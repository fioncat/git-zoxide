mod github;
mod gitlab;

use std::{path::PathBuf, str::FromStr};

use anyhow::{bail, Result};
use console::style;

use crate::config::{self, Remote};

pub struct MergeOption {
    pub repo: String,
    pub upstream: Option<String>,

    pub title: String,
    pub body: String,

    pub source: String,
    pub target: String,
}

impl MergeOption {
    pub fn display(&self) -> String {
        match &self.upstream {
            Some(upstream) => format!(
                "{}:{} => {}:{}",
                style(&self.repo).yellow(),
                style(&self.source).magenta(),
                style(upstream).yellow(),
                style(&self.target).magenta()
            ),
            None => format!(
                "{} => {}",
                style(&self.source).magenta(),
                style(&self.target).magenta()
            ),
        }
    }

    pub fn body_display(&self) -> String {
        let lines = self.body.split("\n").count();
        let word = if lines <= 1 { "line" } else { "lines" };
        format!("{} {}", lines, word)
    }
}

pub trait Provider {
    fn list(&self, group: &str) -> Result<Vec<String>>;

    fn get_default_branch(&self, repo: &str) -> Result<String>;

    fn get_upstream(&self, repo: &str) -> Result<String>;
    fn get_merge(&self, opts: &MergeOption) -> Result<Option<String>>;
    fn create_merge(&self, opts: &MergeOption) -> Result<String>;

    fn get_repo_url(&self, name: &str, branch: Option<String>, remote: &Remote) -> Result<String>;
}

pub fn create_provider(remote: &Remote) -> Result<Box<dyn Provider>> {
    if let None = remote.api {
        bail!(
            "remote {} does not enable api provider, please config it first",
            style(&remote.name).yellow()
        )
    }
    let api = remote.api.as_ref().unwrap();
    match api.provider {
        config::Provider::Github => github::Github::new(&api.token),
        config::Provider::Gitlab => gitlab::Gitlab::new(&api.url, &api.token),
    }
}

fn get_repo_url(domain: &str, name: &str, branch: Option<String>) -> Result<String> {
    let mut path = PathBuf::from_str(domain)?.join(name);
    if let Some(branch) = branch {
        path = path.join("tree").join(branch);
    }
    Ok(format!("https://{}", path.display()))
}
