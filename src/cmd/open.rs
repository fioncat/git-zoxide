use anyhow::Result;

use crate::api;
use crate::cmd::Open;
use crate::cmd::Run;
use crate::config::Config;
use crate::db::Database;
use crate::util;
use crate::util::GitBranch;

impl Run for Open {
    fn run(&self) -> Result<()> {
        let db = Database::open()?;
        let config = Config::parse()?;
        let repo = db.current(&config.workspace)?;
        let remote = config.must_get_remote(&repo.remote)?;
        let provider = api::create_provider(&remote)?;

        let mut branch = None;
        if self.branch {
            branch = Some(GitBranch::current()?);
        }
        let url = provider.get_repo_url(&repo.name, branch, &remote)?;
        util::open_url(url)?;

        Ok(())
    }
}
