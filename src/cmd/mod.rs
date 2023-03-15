mod cd;

use anyhow::Result;
use clap::Parser;

#[derive(Debug, Parser)]
#[clap(about, author)]
pub enum Cmd {
    CD(CD),
}

#[derive(Debug, Parser)]
pub struct CD {
    #[clap(num_args = 0..=2)]
    pub args: Vec<String>,

    #[clap(long, short)]
    pub create: bool,
}

pub trait Run {
    fn run(&self) -> Result<()>;
}

impl Run for Cmd {
    fn run(&self) -> Result<()> {
        match self {
            Cmd::CD(cd) => cd.run(),
        }
    }
}
