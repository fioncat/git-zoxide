use std::io;
use std::io::Write;

use anyhow::bail;
use anyhow::Result;
use console::style;

use crate::cmd::Detach;
use crate::cmd::Run;
use crate::db::Database;
use crate::util;

impl Run for Detach {
    fn run(&self) -> Result<()> {
        let mut db = Database::open()?;

        let path = match &self.dir {
            Some(dir) => util::str_to_path(dir)?,
            None => util::current_dir()?,
        };

        let path_str = util::osstr_to_str(path.as_os_str())?;
        let idx = match db.get_by_path(path_str) {
            Some(idx) => idx,
            None => bail!(
                "path {} did not bound to any repository",
                style(path_str).yellow()
            ),
        };

        db.repos.remove(idx);
        db.save()?;

        _ = writeln!(io::stderr(), "{} detached", style(path_str).yellow());
        Ok(())
    }
}
