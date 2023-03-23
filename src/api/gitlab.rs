use anyhow::{bail, Context, Result};
use gitlab::api::projects::merge_requests::{CreateMergeRequest, MergeRequestState, MergeRequests};
use gitlab::api::{self, groups, projects, Query};
use gitlab::types;

use crate::api::Provider;
use crate::errors;

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

    fn get_project(&self, name: impl AsRef<str>) -> Result<types::Project> {
        let endpoint = projects::Project::builder()
            .project(name.as_ref())
            .build()
            .context("unable to build gitlab project endpoint")?;
        let project = endpoint
            .query(&self.client)
            .context("unable to get project")?;
        Ok(project)
    }
}

impl Provider for Gitlab {
    fn list(&self, group: &str) -> Result<Vec<String>> {
        let endpoint = groups::projects::GroupProjects::builder()
            .group(group)
            .build()
            .context("unable to build gitlab group endpoint")?;
        let projects: Vec<types::Project> = api::paged(endpoint, api::Pagination::All)
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
            .project(opts.repo.clone())
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
}
