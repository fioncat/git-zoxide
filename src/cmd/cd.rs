use std::path::PathBuf;
use std::str::FromStr;

use anyhow::bail;
use anyhow::Result;

use crate::cmd::Run;
use crate::cmd::CD;
use crate::config;
use crate::db;
use crate::util;

impl Run for CD {
    fn run(&self) -> Result<()> {
        let config = config::Config::parse()?;
        let mut db = db::Database::open()?;

        let repo = match self.select_repo(&db) {
            Some(repo) => repo,
            None => self.create_repo(&mut db, &config)?,
        };
        println!("repo = {:?}", repo);

        db.save()?;
        Ok(())
    }
}

impl CD {
    fn select_repo<'a>(&'a self, db: &'a db::Database) -> Option<&'a db::Repo> {
        if self.args.is_empty() {
            return db.get_latest();
        }
        if self.args.len() == 1 {
            return db.contains(&self.args[0]);
        }

        let remote = &self.args[0];
        let query = &self.args[1];
        if query.ends_with("/") {
            return None;
        }

        if let Some(repo) = db.get(remote, query) {
            return Some(repo);
        }
        if self.create {
            return None;
        }

        if !query.contains("/") {
            return db.contains_remote(remote, query);
        }

        let (prefix, query) = util::split_query(query);
        db.contains_remote_prefix(remote, prefix, query)
    }

    fn create_repo<'a>(
        &'a self,
        db: &'a mut db::Database,
        cfg: &'a config::Config,
    ) -> Result<&'a db::Repo> {
        if !self.create {
            println!("do you want to create repo?");
        }
        match self.args.len() {
            0 | 1 => bail!("no repository in database"),
            _ => {
                let remote = &self.args[0];
                let query = &self.args[1];
                if let Some(repo) = db.get(remote, query) {
                    return Ok(repo);
                }
                Ok(db.add(remote, query, "", 0))
            }
        }
    }
}
