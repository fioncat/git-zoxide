use std::fs;
use std::io;
use std::path::PathBuf;

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use console::style;

use crate::api;
use crate::cmd::Home;
use crate::cmd::Run;
use crate::config::{Clone, Config, Remote, User};
use crate::db::{Database, Repo};
use crate::util::{self, Shell};

impl Run for Home {
    fn run(&self) -> Result<()> {
        let mut db = Database::open()?;
        let cfg = Config::parse()?;
        let now = util::current_time()?;

        let (remote, repo_idx) = self.query(&mut db, &cfg)?;

        self.ensure_path(&cfg, &db.repos[repo_idx], remote)?;
        db.update(repo_idx, now);

        let repo = &db.repos[repo_idx];
        let path = repo.path(&cfg.workspace)?;
        println!("{}", path.display());

        db.sort(now);
        db.save()?;

        Ok(())
    }
}

impl Home {
    fn query<'a>(&self, db: &mut Database, cfg: &'a Config) -> Result<(&'a Remote, usize)> {
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
            if let Ok(idx) = self.match_repo(db, remote_name, name) {
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
            "call provider api to list repo for {}",
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
                Some(clone) => self.clone(repo, &clone, &path, &remote.user),
                None => self.create_dir(repo, remote, &path),
            },
            Err(err) => Err(err)
                .with_context(|| format!("could not read repository directory {}", path.display())),
        }
    }

    fn clone(&self, repo: &Repo, clone: &Clone, path: &PathBuf, user: &Option<User>) -> Result<()> {
        let url = repo.clone_url(clone);

        let path = util::path_to_str(path)?;

        let mut git = Shell::git();
        git.arg("clone").args([url.as_str(), path]).exec()?;

        if let Some(user) = user {
            Shell::git()
                .with_git_path(path)
                .args(["config", "user.name"])
                .arg(&user.name)
                .exec()?;
            Shell::git()
                .with_git_path(path)
                .args(["config", "user.email"])
                .arg(&user.email)
                .exec()?;
        }

        Ok(())
    }

    fn create_dir(&self, repo: &Repo, remote: &Remote, path: &PathBuf) -> Result<()> {
        fs::create_dir_all(&path).with_context(|| {
            format!("unable to create repository directory: {}", path.display())
        })?;
        let path_str = util::path_to_str(path)?;
        Shell::git().with_git_path(path_str).arg("init").exec()?;
        self.after_create(repo, remote, path)
    }

    fn after_create(&self, repo: &Repo, remote: &Remote, path: &PathBuf) -> Result<()> {
        if let Some(script) = &remote.on_create {
            let lines: Vec<&str> = script.split("\n").collect();
            for line in lines {
                if line.is_empty() {
                    continue;
                }
                let mut bash = Shell::bash(line);

                bash.env("REPO_NAME", &repo.name);
                bash.env("REMOTE", &remote.name);

                bash.with_path(path);

                bash.exec()?;
            }
        }
        Ok(())
    }
}
