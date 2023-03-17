use anyhow::Result;

use crate::cmd::List;
use crate::cmd::Run;
use crate::config::Config;
use crate::db::Database;

impl Run for List {
    fn run(&self) -> Result<()> {
        let cfg = Config::parse()?;
        if self.args.is_empty() {
            for remote in &cfg.remotes {
                println!("{}", remote.name);
            }
            return Ok(());
        }

        cfg.must_get_remote(&self.args[0])?;
        let db = Database::open()?;
        for repo in &db.repos {
            if repo.remote.as_str() != &self.args[0] {
                continue;
            }
            println!("{}", repo.name);
        }

        Ok(())
    }
}
