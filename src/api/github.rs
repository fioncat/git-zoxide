use std::sync::Arc;

use anyhow::Result;
use futures::executor::block_on;
use octocrab::{initialise, models, Octocrab};

use crate::api::Provider;

pub struct Github {
    instance: Arc<Octocrab>,
}

impl Github {
    pub fn new(token: impl AsRef<str>) -> Result<Box<dyn Provider>> {
        let builder = Octocrab::builder().personal_token(token.as_ref().to_string());
        let instance = initialise(builder)?;
        Ok(Box::new(Github { instance }))
    }
}

impl Provider for Github {
    fn list(&self, group: &str) -> Result<Vec<String>> {
        let url = format!("users/{}/repos", group);
        let repos: Vec<models::Repository> = block_on(self.instance.get(url, None::<&()>))?;
        let mut names: Vec<String> = Vec::with_capacity(repos.len());
        for repo in repos {
            if let Some(name) = repo.full_name {
                names.push(name);
            }
        }

        Ok(names)
    }
}
