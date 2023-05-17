mod attach;
mod branch;
mod clean;
mod config;
mod detach;
mod home;
mod init;
mod jump;
mod list;
mod merge;
mod open;
mod rebase;
mod remove;
mod reset;
mod squash;
mod tag;

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
    Branch(Branch),
    Merge(Merge),
    Open(Open),
    Rebase(Rebase),
    Squash(Squash),
    Reset(Reset),
    Jump(Jump),
    Tag(Tag),
}

/// Print the home path for a repository, recommanded to use `zz` instead
#[derive(Debug, Parser)]
pub struct Home {
    /// Remote and name of the repo, you can use keyword to match repo
    #[clap(num_args = 0..=2)]
    pub args: Vec<String>,

    /// Create the repo
    #[clap(long, short)]
    pub create: bool,

    /// Use remote provider to search the repo
    #[clap(long, short)]
    pub search: bool,
}

/// Remove a repository
#[derive(Debug, Parser)]
pub struct Remove {
    /// The remote of the repo
    pub remote: String,
    /// The name of the repo
    pub name: String,

    /// Direct remove, skip confirm
    #[clap(long, short)]
    pub force: bool,
}

/// Clean unused directory in workspace
#[derive(Debug, Parser)]
pub struct Clean {
    /// Show repo to clean, do not execute
    #[clap(long)]
    pub dry_run: bool,
}

/// Attach current path to a repository
#[derive(Debug, Parser)]
pub struct Attach {
    /// The remote of the repo
    #[clap(required = true)]
    pub remote: String,

    /// The name of the repo
    #[clap(required = true)]
    pub name: String,

    /// The directory to attach, default is current path
    #[clap(long, short)]
    pub dir: Option<String>,

    /// Overwrite the git remote config
    #[clap(long, short)]
    pub remote_config: bool,

    /// Overwrite the git user config
    #[clap(long, short)]
    pub user_config: bool,
}

/// Detach current path from a repository
#[derive(Debug, Parser)]
pub struct Detach {
    /// The directory to detach, default is current path
    #[clap(long, short)]
    pub dir: Option<String>,
}

/// List remotes or repositories
#[derive(Debug, Parser)]
pub struct List {
    /// With remote or not
    #[clap(num_args = 0..=1)]
    pub args: Vec<String>,

    /// Show only group (for completion)
    #[clap(long)]
    pub group: bool,

    /// Show only keyword (for completion)
    #[clap(long)]
    pub keyword: bool,

    /// Show only remote (for completion)
    #[clap(long)]
    pub remote: bool,
}

/// Print the init script, please add `source <(git-zoxide init)` to your profile
#[derive(Debug, Parser)]
pub struct Init {
    /// The command name, default is `gz`
    #[clap(long)]
    pub cmd: Option<String>,

    /// The home command name, default is `zz`
    #[clap(long)]
    pub home_cmd: Option<String>,

    /// The jump command name, default is `zj`
    #[clap(long)]
    pub jump_cmd: Option<String>,
}

/// Edit config file
#[derive(Debug, Parser)]
pub struct Config {
    /// The editor to use, default will auto choose one from your machine
    #[clap(long, short)]
    pub editor: Option<String>,
}

/// Git branch operations
#[derive(Debug, Parser)]
pub struct Branch {
    /// Branch name, optional
    #[clap(num_args = 0..=1)]
    pub args: Vec<String>,

    /// Show all info, include branch status
    #[clap(long, short)]
    pub all: bool,

    /// Sync branch with remote
    #[clap(long, short)]
    pub sync: bool,

    /// Create a new branch
    #[clap(long, short)]
    pub create: bool,

    /// Delete branch
    #[clap(long, short)]
    pub delete: bool,

    /// Push change (create or delete) to remote
    #[clap(long, short)]
    pub push: bool,

    /// Show branch (for completion)
    #[clap(long)]
    pub cmp: bool,
}

/// Create or open MergeRequest or PullRequest
#[derive(Debug, Parser)]
pub struct Merge {
    /// Upstream mode, only used for forked repo
    #[clap(long, short)]
    pub upstream: bool,

    /// Source branch, default will use current branch
    #[clap(long, short)]
    pub source: Option<String>,

    /// Target branch, default will use HEAD branch
    #[clap(long, short)]
    pub target: Option<String>,
}

/// Open current repository in default browser
#[derive(Debug, Parser)]
pub struct Open {
    /// Open current branch
    #[clap(long, short)]
    pub branch: bool,
}

/// Rebase current branch
#[derive(Debug, Parser)]
pub struct Rebase {
    /// Rebase source (optional), default will use HEAD branch
    #[clap(num_args = 0..=1)]
    pub args: Vec<String>,

    /// Upstream mode, only used for forked repo
    #[clap(long, short)]
    pub upstream: bool,
}

/// Squash multiple commits into one
#[derive(Debug, Parser)]
pub struct Squash {
    /// Squash source (optional), default will use HEAD branch
    #[clap(num_args = 0..=1)]
    pub args: Vec<String>,

    /// Upstream mode, only used for forked repo
    #[clap(long, short)]
    pub upstream: bool,

    /// Commit message
    #[clap(long, short)]
    pub message: Option<String>,
}

/// Reset git to remote
#[derive(Debug, Parser)]
pub struct Reset {
    /// Reset source (optional), default will use current branch
    #[clap(num_args = 0..=1)]
    pub args: Vec<String>,

    /// Reset to HEAD branch
    #[clap(long, short)]
    pub default: bool,

    /// Upstream mode, only used for forked repo
    #[clap(long, short)]
    pub upstream: bool,
}

/// Quick jump to a repository (please use `gz` instead)
#[derive(Debug, Parser)]
pub struct Jump {
    /// Jump keyword
    pub keyword: String,
}

/// Git tag operations
#[derive(Debug, Parser)]
pub struct Tag {
    /// Tag name, optional
    #[clap(num_args = 0..=1)]
    pub args: Vec<String>,

    /// The rule name used to create tag
    #[clap(long, short)]
    pub rule: Option<String>,

    /// Create a new tag
    #[clap(long, short)]
    pub create: bool,

    /// Delete tag
    #[clap(long, short)]
    pub delete: bool,

    /// Push change (create or delete) to the remote
    #[clap(long, short)]
    pub push: bool,

    /// Show rules (for completion)
    #[clap(long)]
    pub show_rules: bool,
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
            Cmd::Branch(branch) => branch.run(),
            Cmd::Merge(merge) => merge.run(),
            Cmd::Open(open) => open.run(),
            Cmd::Rebase(rebase) => rebase.run(),
            Cmd::Squash(squash) => squash.run(),
            Cmd::Reset(reset) => reset.run(),
            Cmd::Jump(jump) => jump.run(),
            Cmd::Tag(tag) => tag.run(),
        }
    }
}
