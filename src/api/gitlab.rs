use anyhow::{bail, Context, Result};

// Gitlab api
use gitlab::api;
use gitlab::api::groups::projects::GroupProjects;
use gitlab::api::projects::merge_requests::{CreateMergeRequest, MergeRequestState, MergeRequests};
use gitlab::api::projects::Project;
use gitlab::api::{Pagination, Query};

// Gitlab models
use gitlab::types;

use crate::api::Provider;
use crate::errors;

pub struct Gitlab {
    client: gitlab::Gitlab,
}

impl Provider for Gitlab {
    fn list(&self, group: &str) -> Result<Vec<String>> {
        let endpoint = GroupProjects::builder()
            .group(group)
            .build()
            .context("unable to build gitlab group endpoint")?;
        let projects: Vec<types::Project> = api::paged(endpoint, Pagination::All)
            .query(&self.client)
            .context("unable to query gitlab projects")?;

        let mut repos: Vec<String> = Vec::with_capacity(projects.len());
        for project in projects {
            repos.push(project.path_with_namespace);
        }

        Ok(repos)
    }

    fn get_default_branch(&self, repo: &str) -> Result<String> {
        let project = self.get_project(repo)?;
        match project.default_branch {
            Some(b) => Ok(b),
            None => bail!("gitlab did not return default branch"),
        }
    }

    fn get_upstream(&self, repo: &str) -> Result<String> {
        let project = self.get_project(repo)?;
        match project.forked_from_project {
            Some(up) => Ok(up.path_with_namespace),
            None => bail!(errors::REPO_NO_UPSTREAM),
        }
    }

    fn get_merge(&self, opts: &super::MergeOption) -> Result<Option<String>> {
        if let Some(_) = opts.upstream {
            bail!("sorry, gitlab now does not support upstream features")
        }
        let endpoint = MergeRequests::builder()
            .state(MergeRequestState::Opened)
            .project(opts.repo.as_str())
            .target_branch(&opts.target)
            .source_branch(&opts.source)
            .build()
            .context("unable to build gitlab merge_requests endpoint")?;
        let mrs: Vec<types::MergeRequest> = endpoint
            .query(&self.client)
            .context("unable to query merge_request")?;
        if mrs.is_empty() {
            return Ok(None);
        }
        Ok(Some(mrs[0].web_url.clone()))
    }

    fn create_merge(&self, opts: &super::MergeOption) -> Result<String> {
        if let Some(_) = opts.upstream {
            bail!("sorry, gitlab now does not support upstream features")
        }
        let endpoint = CreateMergeRequest::builder()
            .project(opts.repo.as_str())
            .title(&opts.title)
            .source_branch(&opts.source)
            .target_branch(&opts.target)
            .build()
            .context("unable to build create_merge_request endpoint")?;
        let mr: types::MergeRequest = endpoint
            .query(&self.client)
            .context("unable to create merge_request")?;

        Ok(mr.web_url)
    }

    fn get_repo_url(
        &self,
        name: &str,
        branch: Option<String>,
        remote: &crate::config::Remote,
    ) -> Result<String> {
        if let None = remote.clone {
            bail!("you must provide clone config to get gitlab repo url, please check your config")
        }
        let clone = remote.clone.as_ref().unwrap();
        crate::api::get_repo_url(&clone.domain, name, branch)
    }
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

    fn get_project(&self, name: impl AsRef<str>) -> Result<types::Project> {
        let endpoint = Project::builder()
            .project(name.as_ref())
            .build()
            .context("unable to build gitlab project endpoint")?;
        let project = endpoint
            .query(&self.client)
            .context("unable to get project")?;
        Ok(project)
    }
}
