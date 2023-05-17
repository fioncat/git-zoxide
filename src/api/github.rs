use anyhow::{bail, Context, Result};
use console::style;
use octocrab::{models, Octocrab};
use serde::Serialize;
use tokio::runtime::Runtime;

use crate::{
    api::{self, MergeOption, Provider},
    errors, util,
};

pub struct Github {
    runtime: Runtime,
    instance: Octocrab,

    query_opt: GithubQueryOption,
}

#[derive(Serialize, Debug)]
struct GithubQueryOption {
    per_page: u32,
}

#[derive(Debug)]
struct GithubPullOption {
    owner: String,
    name: String,

    head: String,

    head_owner: String,
}

impl Github {
    const QUERY_PER_PAGE: u32 = 200;

    pub fn new(token: impl AsRef<str>) -> Result<Box<dyn Provider>> {
        let mut builder = Octocrab::builder();
        if !token.as_ref().is_empty() {
            builder = builder.personal_token(token.as_ref().to_string());
        }
        // The octocrab can only run in tokio. Create a runtime for it.
        let runtime = Runtime::new().context("unable to create tokio runtime")?;
        let instance = runtime.block_on(async { builder.build() })?;
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

    fn get_default_branch(&self, repo: &str) -> Result<String> {
        let (owner, name) = Self::parse_repo_name(repo)?;
        let repo = self.get_repo(&owner, &name)?;
        match repo.default_branch {
            Some(b) => Ok(b),
            None => bail!("github did not return default branch"),
        }
    }

    fn get_upstream(&self, repo: &str) -> Result<String> {
        let (owner, name) = Self::parse_repo_name(repo)?;
        let repo = self.get_repo(&owner, &name)?;
        let name = Self::must_get_upstream(&repo)?;
        Ok(name.to_string())
    }

    fn get_merge(&self, opts: &MergeOption) -> Result<Option<String>> {
        let pr = Self::pr_options(opts)?;

        let query = match &opts.upstream {
            Some(upstream) => format!(
                "is:open is:pr author:{} head:{} base:{} repo:{}",
                pr.head_owner, opts.source, opts.target, upstream
            ),
            None => format!(
                "is:open is:pr head:{} base:{} repo:{}",
                opts.source, opts.target, opts.repo
            ),
        };
        let mut issues = self.runtime.block_on(
            self.instance
                .search()
                .issues_and_pull_requests(&query)
                .send(),
        )?;
        let issues = issues.take_items();
        if issues.is_empty() {
            return Ok(None);
        }
        let pr = &issues[0];
        return Ok(Some(pr.html_url.to_string()));
    }

    fn create_merge(&self, opts: &MergeOption) -> Result<String> {
        let pr = Self::pr_options(opts)?;
        let pr = self.runtime.block_on(
            self.instance
                .pulls(&pr.owner, &pr.name)
                .create(&opts.title, &pr.head, &opts.target)
                .body(&opts.body)
                .send(),
        )?;
        match &pr.html_url {
            Some(url) => Ok(url.to_string()),
            None => bail!("github didnot return html_url for pr"),
        }
    }

    fn get_repo_url(
        &self,
        name: &str,
        branch: Option<String>,
        _remote: &crate::config::Remote,
    ) -> Result<String> {
        api::get_repo_url("github.com", name, branch)
    }
}

impl Github {
    fn pr_options(opts: &MergeOption) -> Result<GithubPullOption> {
        let (mut owner, mut name) = Self::parse_repo_name(&opts.repo)?;

        let mut head_owner = String::new();
        let head: String;

        match &opts.upstream {
            Some(upstream) => {
                // Create PR to upstream, the operation object is upstream itself.
                // The base is upstream targetBranch, The head is "user:sourceBranch".
                // For example, merge "fioncat:kubernetes" to "kubernetes:kubernetes"
                // Branch is "master", the params are:
                //   repo: kubernetes/kubernetes
                //   base: master
                //   head: fioncat:master
                head_owner = owner;
                head = format!("{}:{}", head_owner, opts.source);
                (owner, name) = Self::parse_repo_name(upstream)?;
            }
            None => {
                head = opts.source.clone();
            }
        }

        Ok(GithubPullOption {
            owner,
            name,
            head,
            head_owner,
        })
    }

    fn parse_repo_name(repo: &str) -> Result<(String, String)> {
        let (owner, name) = util::split_name(repo);
        if owner.is_empty() || name.is_empty() {
            bail!("invalid github repository name {}", style(repo).yellow())
        }
        Ok((owner, name))
    }

    fn get_repo(&self, owner: &str, name: &str) -> Result<models::Repository> {
        let repo = self
            .runtime
            .block_on(self.instance.repos(owner, name).get())
            .context("unable to get repository from github")?;
        Ok(repo)
    }

    fn must_get_upstream<'a>(repo: &'a models::Repository) -> Result<&'a str> {
        match repo.fork {
            Some(ok) => {
                if ok {
                    if let None = repo.source {
                        bail!(errors::REPO_NO_UPSTREAM)
                    }
                    let source = repo.source.as_ref().unwrap();
                    if let None = source.full_name {
                        bail!(errors::REPO_NO_UPSTREAM)
                    }
                    let name = source.full_name.as_ref().unwrap();
                    Ok(name)
                } else {
                    bail!(errors::REPO_NO_UPSTREAM)
                }
            }
            None => bail!(errors::REPO_NO_UPSTREAM),
        }
    }
}
