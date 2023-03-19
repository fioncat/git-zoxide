use std::collections::HashSet;

use anyhow::Result;

use crate::cmd::List;
use crate::cmd::Run;
use crate::config::Config;
use crate::db::Database;
use crate::util;

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
        if self.group {
            let mut group_set: HashSet<_> = HashSet::new();
            for repo in &db.repos {
                let (group, _) = util::split_name(&repo.name);
                if let Some(_) = group_set.get(&group) {
                    continue;
                }
                println!("{}/", group);
                group_set.insert(group);
            }
            return Ok(());
        }

        for repo in &db.repos {
            if repo.remote.as_str() != &self.args[0] {
                continue;
            }
            println!("{}", repo.name);
        }

        Ok(())
    }
}
