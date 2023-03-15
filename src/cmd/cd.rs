use std::borrow::Borrow;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::bail;
use anyhow::Result;
use console::style;

use crate::cmd::Run;
use crate::cmd::CD;
use crate::config;
use crate::db;
use crate::util;

impl Run for CD {
    fn run(&self) -> Result<()> {
        let config = config::Config::parse()?;
        let mut db = db::Database::open()?;
        let now = util::current_time()?;

        let repo = match self.get_repo(&db)? {
            Some(repo) => repo,
            None => {
                let remote = &self.args[0];
                let name = &self.args[1];
                match db.get(remote, name) {
                    Some(repo) => repo,
                    None => self.create_repo(&mut db, remote, name, now)?,
                }
            }
        };

        println!("repo = {:?}", repo);

        db.sort(now);
        db.save()?;
        Ok(())
    }
}

impl CD {
    fn get_repo<'a>(&'a self, db: &'a db::Database) -> Result<Option<&'a db::Repo>> {
        if self.args.is_empty() {
            let repo = db.get_latest();
            return match repo {
                None => bail!("there has no repository yet, please consider create one"),
                Some(repo) => Ok(Some(repo)),
            };
        }

        let remote = &self.args[0];
        if self.args.len() == 1 {
            let repo = db.contains(remote);
            return match repo {
                None => bail!(
                    "could not find repository match keyword {}",
                    style(remote).yellow()
                ),
                Some(repo) => Ok(Some(repo)),
            };
        }

        let query = &self.args[1];
        if query.ends_with("/") {
            return self.search_repo(db, remote, query);
        }

        if let Some(repo) = db.get(remote, query) {
            return Ok(Some(repo));
        }
        if self.create {
            return Ok(None);
        }

        if !query.contains("/") {
            return Ok(db.contains_remote(remote, query));
        }

        let (prefix, query) = util::split_query(query);
        Ok(db.contains_remote_prefix(remote, prefix, query))
    }

    fn search_repo<'a>(
        &'a self,
        db: &'a db::Database,
        remote: &'a String,
        query: &'a String,
    ) -> Result<Option<&'a db::Repo>> {
        let query = query.trim_end_matches('/');

        let repos = db.list_remote(remote);

        let mut items: Vec<&db::Repo> = Vec::with_capacity(repos.len());
        let mut keys: Vec<&str> = Vec::with_capacity(repos.len());
        for repo in repos.iter() {
            let key = match repo.name.strip_prefix(query) {
                Some(s) => s.trim_matches('/'),
                None => continue,
            };
            if key.is_empty() {
                continue;
            }
            items.push(repo);
            keys.push(key);
        }
        if items.is_empty() {
            bail!("no matches repository with query {}", style(query).yellow())
        }

        let mut fzf = util::Fzf::create()?;
        Ok(Some(repos[fzf.query(&keys)?]))
    }

    fn create_repo<'a>(
        &'a self,
        db: &'a mut db::Database,
        remote: &'a String,
        name: &'a String,
        now: db::Epoch,
    ) -> Result<&'a db::Repo> {
        if !self.create {
            util::confirm(format!("do you want to create {}", style(name).yellow()))?;
        }
        Ok(db.add(remote, name, "", now))
    }
}
