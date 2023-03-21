use std::fs;
use std::io;

use anyhow::Context;
use anyhow::Result;

use crate::cmd::Config;
use crate::cmd::Run;
use crate::config;
use crate::util;

impl Run for Config {
    fn run(&self) -> Result<()> {
        let path = config::Config::get_path()?;
        match fs::read(&path) {
            Ok(_) => util::Shell::edit_file(&self.editor, &path),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                let config_bytes = include_bytes!("../../config.yaml");
                util::write(&path, config_bytes)?;
                util::Shell::edit_file(&self.editor, &path)
            }
            Err(err) => Err(err).context("could not read config file"),
        }
    }
}
