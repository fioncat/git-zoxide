use anyhow::bail;
use anyhow::Result;
use console::style;

use crate::api;
use crate::api::MergeOption;
use crate::api::Provider;
use crate::cmd::Merge;
use crate::cmd::Run;
use crate::config::Config;
use crate::db::Database;
use crate::db::Repo;
use crate::util;
use crate::util::GitBranch;

impl Run for Merge {
    fn run(&self) -> Result<()> {
        GitBranch::ensure_no_uncommitted()?;
        let db = Database::open()?;
        let config = Config::parse()?;
        let repo = db.current(&config.workspace)?;
        let remote = config.must_get_remote(&repo.remote)?;
        let provider = api::create_provider(&remote)?;

        let mut upstream = None;
        if self.upstream {
            util::print_operation(format!(
                "provider: get upstream for {}",
                style(&repo.name).yellow()
            ));
            upstream = Some(provider.get_upstream(&repo.name)?);
        }

        let mut opts = self.options(repo, &provider, &upstream)?;
        opts.upstream = upstream;
        if let None = opts.upstream {
            if opts.source.eq(&opts.target) {
                bail!("could not merge myself")
            }
        }

        util::print_operation(format!(
            "provider: query merge for {}",
            style(&repo.name).yellow()
        ));
        let merge = provider.get_merge(&opts)?;

        let url = match merge {
            Some(url) => url,
            None => self.create(&mut opts, &provider)?,
        };

        util::open_url(url.as_str())?;

        Ok(())
    }
}

impl Merge {
    const TITLE_EMPTY: &str = "merge title cannot be empty";

    fn options(
        &self,
        repo: &Repo,
        provider: &Box<dyn Provider>,
        upstream: &Option<String>,
    ) -> Result<MergeOption> {
        let target = match &self.target {
            Some(t) => t.to_string(),
            None => match upstream {
                Some(upstream) => {
                    util::print_operation(format!(
                        "provider: get default branch for upstream {}",
                        style(&upstream).yellow()
                    ));
                    provider.get_default_branch(&upstream)?
                }
                None => GitBranch::default()?,
            },
        };

        let source = match &self.source {
            Some(s) => s.to_string(),
            None => GitBranch::current()?,
        };

        Ok(MergeOption {
            repo: repo.name.clone(),
            upstream: None,
            title: String::new(),
            body: String::new(),
            source,
            target,
        })
    }

    fn create(&self, opts: &mut MergeOption, provider: &Box<dyn Provider>) -> Result<String> {
        (opts.title, opts.body) = self.input()?;

        println!();
        println!("Ready to create merge: {}", opts.display());
        println!("Title: {}", style(&opts.title).yellow());
        println!("Body: {}", style(opts.body_display()).yellow());
        println!();

        util::confirm("continue")?;

        util::print_operation(format!(
            "provider: create merge request {}",
            style(&opts.title).yellow()
        ));
        provider.create_merge(opts)
    }

    fn input(&self) -> Result<(String, String)> {
        let template = include_bytes!("../../files/merge_request.md");
        let template = String::from_utf8_lossy(template);

        let edited = util::edit(template.as_ref(), ".md", true)?;

        let lines: Vec<&str> = edited.split("\n").collect();
        let mut title = None;
        let mut body_lines: Vec<&str> = vec![];
        for line in lines {
            let line = line.trim();
            if line.starts_with("#") {
                title = Some(line.strip_prefix("#").unwrap().trim());
                continue;
            }
            if let Some(_) = title {
                body_lines.push(line);
            }
        }
        if let None = title {
            bail!(Self::TITLE_EMPTY)
        }
        let title = title.unwrap();
        if title.is_empty() {
            bail!(Self::TITLE_EMPTY)
        }
        let body = body_lines.join("\n");

        Ok((title.to_string(), body.trim().to_string()))
    }
}
