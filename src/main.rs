use std::io::{self, Write};
use std::process::ExitCode;

mod api;
mod cmd;
mod config;
mod db;
mod errors;
mod util;

use clap::Parser;
use console::{self, style};

use crate::cmd::{Cmd, Run};
use crate::errors::SilentExit;

fn main() -> ExitCode {
    console::set_colors_enabled(true);
    match Cmd::parse().run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => match err.downcast::<SilentExit>() {
            Ok(SilentExit { code }) => code.into(),
            Err(err) => {
                if util::is_printed() {
                    _ = writeln!(io::stderr());
                }
                _ = writeln!(io::stderr(), "{}: {err:?}", style("error").red());
                ExitCode::FAILURE
            }
        },
    }
}
