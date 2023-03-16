use std::fs;
use std::io;
use std::path::PathBuf;

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use console::style;

use crate::cmd::Run;
use crate::cmd::CD;
use crate::config::{Clone, Config, Remote};
use crate::db::{Database, Repo};
use crate::util;

impl Run for CD {
    fn run(&self) -> Result<()> {
        let mut db = Database::open()?;
        let cfg = Config::parse()?;
        let now = util::current_time()?;

        let (remote, repo_idx) = self.query(&mut db, &cfg)?;

        self.ensure_path(&cfg, &db.repos[repo_idx], remote)?;
        db.update(repo_idx, now);

        println!("remote = {:?}", remote);
        println!("repo = {:?}", &db.repos[repo_idx]);

        db.sort(now);
        db.save()?;

        Ok(())
    }
}

impl CD {
    fn query<'a>(&self, db: &mut Database, cfg: &'a Config) -> Result<(&'a Remote, usize)> {
        if self.args.is_empty() {
            if db.repos.is_empty() {
                bail!("there is no repo in the database, please consider creating one")
            }
            let remote = cfg.must_get_remote(&db.repos[0].remote)?;
            return Ok((remote, 0));
        }

        if self.args.len() == 1 {
            let arg = &self.args[0];
            match cfg.get_remote(arg.as_str()) {
                Some(remote) => return Ok((remote, self.search_repo(db, arg, "")?)),
                None => {
                    let idx = self.match_repo(db, "", arg)?;
                    let remote = cfg.must_get_remote(&db.repos[idx].remote)?;
                    return Ok((remote, idx));
                }
            }
        }

        let remote_name = &self.args[0];
        let remote = cfg.must_get_remote(remote_name)?;
        let name = &self.args[1];

        if name.ends_with("/") {
            return Ok((remote, self.search_repo(db, remote_name, name)?));
        }

        if let Some(idx) = db.get(&remote.name, name) {
            return Ok((remote, idx));
        }
        if !self.create {
            if let Ok(idx) = self.match_repo(db, remote_name, name) {
                return Ok((remote, idx));
            }
        }

        util::confirm(format!("do you want to create {}", style(name).yellow()))?;
        Ok((remote, db.add(remote_name, name, "")))
    }

    fn search_repo<R, Q>(&self, db: &Database, remote: R, query: Q) -> Result<usize>
    where
        R: AsRef<str>,
        Q: AsRef<str>,
    {
        let query = query.as_ref().trim_end_matches("/");

        let mut items: Vec<usize> = Vec::with_capacity(db.repos.len());
        let mut keys: Vec<&str> = Vec::with_capacity(db.repos.len());
        for (idx, repo) in db.repos.iter().enumerate() {
            if repo.remote != remote.as_ref() {
                continue;
            }
            let key = match repo.name.strip_prefix(query) {
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
            bail!("no matches repository with query {}", style(query).yellow())
        }

        let mut fzf = util::Fzf::create()?;
        Ok(items[fzf.query(&keys)?])
    }

    fn match_repo<R, Q>(&self, db: &Database, remote: R, query: Q) -> Result<usize>
    where
        R: AsRef<str>,
        Q: AsRef<str>,
    {
        let (group, base) = util::split_name(query.as_ref());
        let opt = db.repos.iter().position(|repo| {
            if remote.as_ref() != "" && repo.remote != remote.as_ref() {
                return false;
            }
            let (repo_group, repo_base) = util::split_name(&repo.name);
            if group == "" {
                return repo_base.contains(&base);
            }

            repo_group == group && repo_base.contains(&base)
        });
        match opt {
            Some(idx) => Ok(idx),
            None => bail!(
                "could not find repository matches {}",
                style(query.as_ref()).yellow()
            ),
        }
    }

    fn ensure_path(&self, cfg: &Config, repo: &Repo, remote: &Remote) -> Result<()> {
        let path = repo.path(&cfg.workspace)?;
        match fs::read_dir(&path) {
            Ok(_) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => match &remote.clone {
                Some(clone) => self.clone(repo, &clone),
                None => self.create_dir(&path),
            },
            Err(err) => Err(err)
                .with_context(|| format!("could not read repository directory {}", path.display())),
        }
    }

    fn clone(&self, repo: &Repo, clone: &Clone) -> Result<()> {
        let mut ssh = clone.use_ssh;
        if !ssh && clone.ssh_groups != "" {
            let (group, _) = util::split_name(&repo.name);
            if let Some(_) = clone.ssh_groups.split(';').find(|s| s == &group) {
                ssh = true;
            }
        }

        let url = if ssh {
            format!("git@{}:{}.git", clone.domain, repo.name)
        } else {
            format!("https://{}/{}.git", clone.domain, repo.name)
        };

        println!("git clone {}", url);

        Ok(())
    }

    fn create_dir(&self, path: &PathBuf) -> Result<()> {
        fs::create_dir_all(&path)
            .with_context(|| format!("unable to create repository directory: {}", path.display()))
    }
}
