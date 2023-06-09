use anyhow::Result;

use crate::cmd::Jump;
use crate::cmd::Run;
use crate::config::Config;
use crate::db::Database;
use crate::db::Keywords;
use crate::util;

impl Run for Jump {
    fn run(&self) -> Result<()> {
        let now = util::current_time()?;
        let mut db = Database::open()?;
        let mut keywords = Keywords::open(now)?;
        let config = Config::parse()?;

        let idx = db.match_keyword("", &self.keyword, &config.keyword_map)?;
        let repo = &db.repos[idx];

        let remote = config.must_get_remote(&repo.remote)?;
        let path = repo.ensure_path(&config.workspace, &remote)?;
        println!("{}", path.display());

        let (_, name) = util::split_name(&repo.name);
        if !name.eq(&self.keyword) {
            if let None = config.keyword_map.get(&self.keyword) {
                keywords.add(&self.keyword, now);
                keywords.save()?;
            }
        }

        db.sort(now);
        db.save()?;

        Ok(())
    }
}
