use std::io;
use std::io::Write;

use anyhow::bail;
use anyhow::Result;
use console::style;

use crate::cmd::Attach;
use crate::cmd::Run;
use crate::config::Config;
use crate::db::Database;
use crate::util;

impl Run for Attach {
    fn run(&self) -> Result<()> {
        let mut db = Database::open()?;
        let cfg = Config::parse()?;

        let path = match &self.dir {
            Some(dir) => util::str_to_path(dir)?,
            None => util::current_dir()?,
        };

        let remote = cfg.must_get_remote(&self.remote)?;
        if let Some(_) = db.get(&self.remote, &self.name) {
            bail!(
                "repository {}:{} is already exists",
                style(&self.remote).yellow(),
                style(&self.name).yellow()
            )
        }

        let path_str = util::osstr_to_str(path.as_os_str())?;
        if let Some(_) = db.get_by_path(path_str) {
            bail!(
                "path {} has already bound to anthor repository, please consider detach first",
                style(path_str).yellow()
            )
        }

        let idx = db.add(&self.remote, &self.name, path_str);
        if self.remote_config {
            if let Some(clone) = &remote.clone {
                let url = db.repos[idx].clone_url(clone);
                util::Shell::git()?
                    .with_git_path(path_str)
                    .args(["remote", "set-url", "origin"])
                    .arg(url)
                    .exec()?;
            }
        }
        if self.user_config {
            if let Some(user) = &remote.user {
                util::Shell::git()?
                    .with_git_path(path_str)
                    .args(["config", "user.name"])
                    .arg(&user.name)
                    .exec()?;
                util::Shell::git()?
                    .with_git_path(path_str)
                    .args(["config", "user.email"])
                    .arg(&user.email)
                    .exec()?;
            }
        }

        db.save()?;

        _ = writeln!(io::stderr(), "{} attached", style(path_str).yellow());
        Ok(())
    }
}
