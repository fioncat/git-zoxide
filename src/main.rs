use std::io::{self, Write};
use std::process::ExitCode;

mod cmd;
mod config;
mod db;
mod util;

use clap::Parser;

use crate::cmd::{Cmd, Run};

fn main() -> ExitCode {
    match Cmd::parse().run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            _ = writeln!(io::stderr(), "error: {err:?}");
            ExitCode::FAILURE
        }
    }
}
