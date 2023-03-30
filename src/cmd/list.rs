use std::collections::HashSet;

use anyhow::Result;

use crate::cmd::List;
use crate::cmd::Run;
use crate::config::Config;
use crate::db::{Database, Keywords};
use crate::util;

impl Run for List {
    fn run(&self) -> Result<()> {
        let cfg = Config::parse()?;

        if self.keyword {
            let now = util::current_time()?;
            let db = Database::open()?;
            let keywords = Keywords::open(now)?;
            let mut name_set = HashSet::with_capacity(db.repos.len() + keywords.data.len());
            for repo in &db.repos {
                let (_, name) = util::split_name(&repo.name);
                if let Some(_) = name_set.get(&name) {
                    continue;
                }
                println!("{}", name);
                name_set.insert(name);
            }

            let keywords = keywords.list();
            for keyword in keywords {
                if let Some(_) = name_set.get(keyword) {
                    continue;
                }
                name_set.insert(keyword.to_string());
                println!("{}", keyword);
            }

            let mut keys: Vec<_> = cfg
                .keyword_map
                .iter()
                .map(|(key, _)| key.to_string())
                .collect();
            keys.sort_by(|s1, s2| s1.cmp(&s2));
            for key in keys {
                if let None = name_set.get(&key) {
                    println!("{}", key);
                }
            }

            return Ok(());
        }

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
