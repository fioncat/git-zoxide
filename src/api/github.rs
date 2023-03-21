use std::sync::Arc;

use anyhow::{Context, Result};
use octocrab::{auth, initialise, models, Octocrab};
use serde::Serialize;
use tokio::runtime::Runtime;

use crate::api::Provider;

pub struct Github {
    runtime: Runtime,
    instance: Arc<Octocrab>,

    query_opt: GithubQueryOption,
}

#[derive(Serialize, Debug)]
struct GithubQueryOption {
    per_page: u32,
}

impl Github {
    const QUERY_PER_PAGE: u32 = 200;

    pub fn new(token: impl AsRef<str>) -> Result<Box<dyn Provider>> {
        let builder = Octocrab::builder().oauth(auth::OAuth {
            access_token: secrecy::SecretString::new(token.as_ref().to_string()),
            token_type: "Bearer".to_string(),
            scope: vec![],
        });
        let instance = initialise(builder)?;
        let runtime = Runtime::new().context("unable to create tokio runtime")?;
        let query_opt = GithubQueryOption {
            per_page: Self::QUERY_PER_PAGE,
        };
        Ok(Box::new(Github {
            runtime,
            instance,
            query_opt,
        }))
    }
}

impl Provider for Github {
    fn list(&self, group: &str) -> Result<Vec<String>> {
        let url = format!("users/{}/repos", group);

        let repos: Vec<models::Repository> = self
            .runtime
            .block_on(self.instance.get(url, Some(&self.query_opt)))?;
        let mut names: Vec<String> = Vec::with_capacity(repos.len());
        for repo in repos {
            if let Some(name) = repo.full_name {
                names.push(name);
            }
        }

        Ok(names)
    }
}
