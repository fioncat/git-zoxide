mod attach;
mod clean;
mod detach;
mod home;
mod init;
mod list;
mod remove;

use anyhow::Result;
use clap::Parser;

#[derive(Debug, Parser)]
#[clap(about, author)]
pub enum Cmd {
    Home(Home),
    Remove(Remove),
    Clean(Clean),
    Attach(Attach),
    Detach(Detach),
    List(List),
    Init(Init),
}

#[derive(Debug, Parser)]
pub struct Home {
    #[clap(num_args = 0..=2)]
    pub args: Vec<String>,

    #[clap(long, short)]
    pub create: bool,

    #[clap(long, short)]
    pub search: bool,
}

#[derive(Debug, Parser)]
pub struct Remove {
    pub remote: String,
    pub name: String,

    #[clap(long, short)]
    pub force: bool,
}

#[derive(Debug, Parser)]
pub struct Clean {
    #[clap(long)]
    pub dry_run: bool,
}

#[derive(Debug, Parser)]
pub struct Attach {
    #[clap(required = true)]
    pub remote: String,

    #[clap(required = true)]
    pub name: String,

    #[clap(long, short)]
    pub dir: Option<String>,

    #[clap(long, short)]
    pub remote_config: bool,

    #[clap(long, short)]
    pub user_config: bool,
}

#[derive(Debug, Parser)]
pub struct Detach {
    #[clap(long, short)]
    pub dir: Option<String>,
}

#[derive(Debug, Parser)]
pub struct List {
    #[clap(num_args = 0..=1)]
    pub args: Vec<String>,

    #[clap(long, short)]
    pub group: bool,
}

#[derive(Debug, Parser)]
pub struct Init {
    #[clap(long)]
    pub cmd: Option<String>,

    #[clap(long)]
    pub home_cmd: Option<String>,
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
            Cmd::Attach(attach) => attach.run(),
            Cmd::Detach(detach) => detach.run(),
            Cmd::List(list) => list.run(),
            Cmd::Init(init) => init.run(),
        }
    }
}
