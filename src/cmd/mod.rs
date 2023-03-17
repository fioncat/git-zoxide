mod clean;
mod home;
mod remove;

use anyhow::Result;
use clap::Parser;

#[derive(Debug, Parser)]
#[clap(about, author)]
pub enum Cmd {
    Home(Home),
    Remove(Remove),
    Clean(Clean),
}

#[derive(Debug, Parser)]
pub struct Home {
    #[clap(num_args = 0..=2)]
    pub args: Vec<String>,

    #[clap(long, short)]
    pub create: bool,
}

#[derive(Debug, Parser)]
pub struct Remove {
    #[clap(required = true)]
    pub remote: String,

    #[clap(required = true)]
    pub name: String,

    #[clap(long, short)]
    pub force: bool,
}

#[derive(Debug, Parser)]
pub struct Clean {
    #[clap(long)]
    pub dry_run: bool,
}

pub trait Run {
    fn run(&self) -> Result<()>;
}

impl Run for Cmd {
    fn run(&self) -> Result<()> {
        match self {
            Cmd::Home(home) => home.run(),
            Cmd::Remove(remove) => remove.run(),
            Cmd::Clean(clean) => clean.run(),
        }
    }
}
