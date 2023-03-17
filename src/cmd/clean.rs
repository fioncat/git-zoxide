use std::path::PathBuf;

use anyhow::Result;

use crate::cmd::Clean;
use crate::cmd::Run;
use crate::config::Config;
use crate::db::Database;
use crate::util;

impl Run for Clean {
    fn run(&self) -> Result<()> {
        let db = Database::open()?;
        let cfg = Config::parse()?;

        let paths = db.list_paths(&cfg.workspace)?;
        let empty_dir = util::EmptyDir::scan(&cfg.workspace, &paths)?;

        if self.dry_run {
            let mut dirs = vec![];
            empty_dir.list(&mut dirs);
            for dir in dirs {
                println!("{}", PathBuf::from(dir).display());
            }
            return Ok(());
        }
        empty_dir.clean()
    }
}
