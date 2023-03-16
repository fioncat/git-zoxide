use std::fs;
use std::io;

use anyhow::Context;
use anyhow::Result;

use crate::cmd::Remove;
use crate::cmd::Run;

use crate::config::Config;
use crate::db::{Database, Repo};
use crate::errors::SilentExit;
use crate::util;

impl Run for Remove {
    fn run(&self) -> Result<()> {
        let mut db = Database::open()?;
        let cfg = Config::parse()?;

        let idx = db.must_get(&self.remote, &self.name)?;
        self.ensure_path(&cfg, &db.repos[idx])?;

        db.repos.remove(idx);
        db.save()?;
        Ok(())
    }
}

impl Remove {
    fn ensure_path(&self, cfg: &Config, repo: &Repo) -> Result<()> {
        let path = repo.path(&cfg.workspace)?;
        match fs::read_dir(&path) {
            Ok(_) => {
                let mut remove = self.force;
                if !remove {
                    match util::confirm(format!("do you want to remove {}", path.display())) {
                        Ok(_) => remove = true,
                        Err(err) => match err.downcast::<SilentExit>() {
                            Ok(_) => return Ok(()),
                            Err(err) => return Err(err),
                        },
                    };
                }
                if remove {
                    fs::remove_dir_all(&path)?;
                }
                Ok(())
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err)
                .with_context(|| format!("could not read repository directory {}", path.display())),
        }
    }
}
