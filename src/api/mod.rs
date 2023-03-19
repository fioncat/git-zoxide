mod github;

use anyhow::{bail, Result};
use console::style;

use crate::config::{self, Remote};

pub trait Provider {
    fn list(&self, group: &str) -> Result<Vec<String>>;
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
        config::Provider::Gitlab => bail!("currently we do not support gitlab"),
    }
}
