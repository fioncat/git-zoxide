use anyhow::{bail, Context, Result};
use gitlab::api::{groups, Query};
use gitlab::types::Project;

use crate::api::Provider;

pub struct Gitlab {
    client: gitlab::Gitlab,
}

impl Gitlab {
    pub fn new<U, T>(url: U, token: T) -> Result<Box<dyn Provider>>
    where
        U: AsRef<str>,
        T: AsRef<str>,
    {
        if url.as_ref().is_empty() {
            bail!("for gitlab provider, you must specify api url, please check your config")
        }
        let client = gitlab::Gitlab::new(url.as_ref(), token.as_ref())
            .context("unable to init gitlab client")?;
        Ok(Box::new(Gitlab { client }))
    }
}

impl Provider for Gitlab {
    fn list(&self, group: &str) -> Result<Vec<String>> {
        let endpoint = groups::projects::GroupProjects::builder()
            .group(group)
            .build()
            .context("unable to build gitlab group endpoint")?;
        let projects: Vec<Project> = endpoint
            .query(&self.client)
            .context("unable to query gitlab projects")?;

        let mut repos: Vec<String> = Vec::with_capacity(projects.len());
        for project in projects {
            repos.push(project.path_with_namespace);
        }

        Ok(repos)
    }
}
