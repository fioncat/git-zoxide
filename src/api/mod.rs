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
        if self.body.is_empty() {
            return String::from("<empty>");
        }
        let lines = self.body.split("\n").count();
        let word = if lines <= 1 { "line" } else { "lines" };
        format!("{} {}", lines, word)
    }
}

pub trait Provider {
    // list all repos for a group, the group can be owner or org in Github.
    fn list(&self, group: &str) -> Result<Vec<String>>;

    // Get default branch name.
    fn get_default_branch(&self, repo: &str) -> Result<String>;

    // Get upstream repo name. Only work for forked repo. This will return
    // `errors.REPO_NO_UPSTREAM` for no forked repo.
    fn get_upstream(&self, repo: &str) -> Result<String>;

    // Try to get URL for merge request (or PR for Github). If merge request
    // not exists, return Ok(None).
    fn get_merge(&self, opts: &MergeOption) -> Result<Option<String>>;
    // Create merge request (or PR for Github), and return its URL.
    fn create_merge(&self, opts: &MergeOption) -> Result<String>;

    // Get web url for repo.
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
