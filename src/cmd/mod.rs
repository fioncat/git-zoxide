mod attach;
mod clean;
mod config;
mod detach;
mod home;
mod init;
mod list;
mod remove;

use anyhow::Result;
use clap::Parser;

#[derive(Debug, Parser)]
#[clap(about, author, version)]
pub enum Cmd {
    Home(Home),
    Remove(Remove),
    Clean(Clean),
    Attach(Attach),
    Detach(Detach),
    List(List),
    Init(Init),
    Config(Config),
}

// Home print the path for a repository
#[derive(Debug, Parser)]
#[command(about = "Print the home path for a repository, recommanded to use `zz` instead")]
pub struct Home {
    #[clap(num_args = 0..=2)]
    pub args: Vec<String>,

    // Create the repo
    #[clap(long, short)]
    pub create: bool,

    #[clap(long, short)]
    pub search: bool,
}

#[derive(Debug, Parser)]
#[command(about = "Remove a repository")]
pub struct Remove {
    pub remote: String,
    pub name: String,

    #[clap(long, short)]
    pub force: bool,
}

#[derive(Debug, Parser)]
#[command(about = "Clean unused directory in workspace")]
pub struct Clean {
    #[clap(long)]
    pub dry_run: bool,
}

#[derive(Debug, Parser)]
#[command(about = "Attach current path to a repository")]
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
#[command(about = "Detach current path from a repository")]
pub struct Detach {
    #[clap(long, short)]
    pub dir: Option<String>,
}

#[derive(Debug, Parser)]
#[command(about = "List remotes or repositories")]
pub struct List {
    #[clap(num_args = 0..=1)]
    pub args: Vec<String>,

    #[clap(long, short)]
    pub group: bool,
}

#[derive(Debug, Parser)]
#[command(about = "Print the init script, please add `source <(git-zoxide init)` to your profile")]
pub struct Init {
    // cmd name, default is `gz`
    #[clap(long)]
    pub cmd: Option<String>,

    #[clap(long)]
    pub home_cmd: Option<String>,
}

#[derive(Debug, Parser)]
#[command(about = "Edit config file")]
pub struct Config {
    #[clap(long, short)]
    pub editor: Option<String>,
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
            Cmd::Config(config) => config.run(),
        }
    }
}
