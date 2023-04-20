use anyhow::bail;
use anyhow::Result;
use console::style;

use crate::api;
use crate::cmd::Home;
use crate::cmd::Run;
use crate::config::{Config, Remote};
use crate::db::Database;
use crate::db::Epoch;
use crate::db::Keywords;
use crate::util;

impl Run for Home {
    fn run(&self) -> Result<()> {
        let mut db = Database::open()?;
        let cfg = Config::parse()?;
        let now = util::current_time()?;

        let (remote, repo_idx) = self.query(&mut db, &cfg, now)?;
        let repo = &db.repos[repo_idx];

        let path = repo.ensure_path(&cfg.workspace, remote)?;
        db.update(repo_idx, now);

        println!("{}", path.display());

        db.sort(now);
        db.save()?;

        Ok(())
    }
}

impl Home {
    fn query<'a>(
        &self,
        db: &mut Database,
        cfg: &'a Config,
        now: Epoch,
    ) -> Result<(&'a Remote, usize)> {
        if self.args.is_empty() {
            if db.repos.is_empty() {
                bail!("there is no repo in the database, please consider creating one")
            }
            let remote = cfg.must_get_remote(&db.repos[0].remote)?;
            let mut last_access = 0;
            let mut last_idx: usize = 0;
            for (idx, repo) in db.repos.iter().enumerate() {
                if repo.last_accessed > last_access {
                    last_access = repo.last_accessed;
                    last_idx = idx;
                }
            }
            return Ok((remote, last_idx));
        }

        if self.args.len() == 1 {
            let arg = &self.args[0];
            match cfg.get_remote(arg.as_str()) {
                Some(remote) => return Ok((remote, self.search_repo(db, arg, "")?)),
                None => {
                    let idx = db.match_keyword("", arg, &cfg.keyword_map)?;
                    if let None = cfg.keyword_map.get(arg) {
                        // Store keyword in database to make completion next time
                        let mut keyword_db = Keywords::open(now)?;
                        keyword_db.add(&arg, now);
                        keyword_db.save()?;
                    }
                    let remote = cfg.must_get_remote(&db.repos[idx].remote)?;
                    return Ok((remote, idx));
                }
            }
        }

        let remote_name = &self.args[0];
        let remote = cfg.must_get_remote(remote_name)?;
        let name = &self.args[1];

        if name.ends_with("/") {
            let name = name.trim_end_matches("/");
            if self.search {
                return Ok((remote, self.search_repo_remote(db, remote, name)?));
            }
            return Ok((remote, self.search_repo(db, remote_name, name)?));
        }

        if let Some(idx) = db.get(&remote.name, name) {
            return Ok((remote, idx));
        }
        if !self.create {
            if let Ok(idx) = db.match_keyword(remote_name, name, &cfg.keyword_map) {
                return Ok((remote, idx));
            }
        }

        Ok((remote, self.create_repo(db, remote_name, name)?))
    }

    fn search_repo<R, Q>(&self, db: &Database, remote: R, query: Q) -> Result<usize>
    where
        R: AsRef<str>,
        Q: AsRef<str>,
    {
        let mut items: Vec<usize> = Vec::with_capacity(db.repos.len());
        let mut keys: Vec<&str> = Vec::with_capacity(db.repos.len());
        for (idx, repo) in db.repos.iter().enumerate() {
            if repo.remote != remote.as_ref() {
                continue;
            }
            let key = match repo.name.strip_prefix(query.as_ref()) {
                Some(s) => s.trim_matches('/'),
                None => continue,
            };
            if key.is_empty() {
                continue;
            }
            items.push(idx);
            keys.push(key);
        }

        if items.is_empty() {
            bail!(
                "no matches repository with query {}",
                style(query.as_ref()).yellow()
            )
        }

        let mut fzf = util::Fzf::build()?;
        Ok(items[fzf.query(&keys)?])
    }

    fn search_repo_remote(
        &self,
        db: &mut Database,
        remote: &Remote,
        query: impl AsRef<str>,
    ) -> Result<usize> {
        let provider = api::create_provider(remote)?;

        util::print_operation(format!(
            "provider: list repo for {}",
            style(query.as_ref()).yellow()
        ));
        let repo_names = provider.list(query.as_ref())?;
        let mut keys = Vec::with_capacity(repo_names.len());
        for repo_name in &repo_names {
            let key = match repo_name.strip_prefix(query.as_ref()) {
                Some(key) => key.trim_matches('/'),
                None => &repo_name,
            };
            keys.push(key);
        }

        let mut fzf = util::Fzf::build()?;
        let idx = fzf.query(&keys)?;

        let repo_name = &repo_names[idx];
        if let Some(idx) = db.get(&remote.name, repo_name) {
            return Ok(idx);
        }
        self.create_repo(db, &remote.name, repo_name)
    }

    fn create_repo<R, N>(&self, db: &mut Database, remote: R, name: N) -> Result<usize>
    where
        R: AsRef<str>,
        N: AsRef<str>,
    {
        util::confirm(format!(
            "do you want to create {}",
            style(name.as_ref()).yellow()
        ))?;
        Ok(db.add(remote.as_ref(), name.as_ref(), ""))
    }
}
