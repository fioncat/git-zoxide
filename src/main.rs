use std::io::{self, Write};
use std::process::ExitCode;

mod cmd;
mod config;
mod db;
mod errors;
mod util;

use clap::Parser;
use console::style;

use crate::cmd::{Cmd, Run};
use crate::errors::SilentExit;

fn main() -> ExitCode {
    match Cmd::parse().run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => match err.downcast::<SilentExit>() {
            Ok(SilentExit { code }) => code.into(),
            Err(err) => {
                _ = writeln!(io::stderr(), "{}: {err:?}", style("error").red());
                ExitCode::FAILURE
            }
        },
    }
}
