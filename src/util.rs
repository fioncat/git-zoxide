use std::collections::HashSet;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt::Display;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::mem;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::str::FromStr;
use std::time::SystemTime;

use anyhow::{bail, Context, Result};
use chrono::offset::Local;

use crate::api;
use crate::config::Config;
use crate::db::{Database, Epoch, Repo};
use crate::errors::SilentExit;

use console::{style, StyledObject, Term};
use dialoguer::{theme::ColorfulTheme, Confirm};
use regex::{Captures, Regex};

pub const SECOND: Epoch = 1;
pub const MINUTE: Epoch = 60 * SECOND;
pub const HOUR: Epoch = 60 * MINUTE;
pub const DAY: Epoch = 24 * HOUR;
pub const WEEK: Epoch = 7 * DAY;
// pub const MONTH: Epoch = 30 * DAY;

/// Similar to [`fs::write`], but atomic (best effort on Windows).
pub fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<()> {
    let path = path.as_ref();
    let contents = contents.as_ref();
    let dir = path.parent().unwrap();

    // Create a tmpfile.
    let (mut tmp_file, tmp_path) = tmpfile(dir)?;
    let result = (|| {
        // Write to the tmpfile.
        _ = tmp_file.set_len(contents.len() as u64);
        tmp_file
            .write_all(contents)
            .with_context(|| format!("could not write to file: {}", tmp_path.display()))?;

        // Set the owner of the tmpfile (UNIX only).
        #[cfg(unix)]
        if let Ok(metadata) = path.metadata() {
            use std::os::unix::fs::MetadataExt;
            use std::os::unix::io::AsRawFd;

            use nix::unistd::{self, Gid, Uid};

            let uid = Uid::from_raw(metadata.uid());
            let gid = Gid::from_raw(metadata.gid());
            _ = unistd::fchown(tmp_file.as_raw_fd(), Some(uid), Some(gid));
        }

        // Close and rename the tmpfile.
        mem::drop(tmp_file);
        rename(&tmp_path, path)
    })();
    // In case of an error, delete the tmpfile.
    if result.is_err() {
        _ = fs::remove_file(&tmp_path);
    }
    result
}

/// Atomically create a tmpfile in the given directory.
fn tmpfile(dir: impl AsRef<Path>) -> Result<(File, PathBuf)> {
    const MAX_ATTEMPTS: usize = 5;
    const TMP_NAME_LEN: usize = 16;
    let dir = dir.as_ref();

    let mut attempts = 0;
    loop {
        attempts += 1;

        // Generate a random name for the tmpfile.
        let mut name = String::with_capacity(TMP_NAME_LEN);
        name.push_str("tmp_");
        while name.len() < TMP_NAME_LEN {
            name.push(fastrand::alphanumeric());
        }
        let path = dir.join(name);

        // Atomically create the tmpfile.
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => break Ok((file, path)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && attempts < MAX_ATTEMPTS => {}
            Err(e) => {
                break Err(e).with_context(|| format!("could not create file: {}", path.display()));
            }
        }
    }
}

/// Similar to [`fs::rename`], but with retries on Windows.
fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
    let from = from.as_ref();
    let to = to.as_ref();

    const MAX_ATTEMPTS: usize = if cfg!(windows) { 5 } else { 1 };
    let mut attempts = 0;

    loop {
        match fs::rename(from, to) {
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied && attempts < MAX_ATTEMPTS => {
                attempts += 1
            }
            result => {
                break result.with_context(|| {
                    format!(
                        "could not rename file: {} -> {}",
                        from.display(),
                        to.display()
                    )
                });
            }
        }
    }
}

pub fn split_name<'a>(query: impl AsRef<str>) -> (String, String) {
    let items: Vec<_> = query.as_ref().split("/").collect();
    let items_len = items.len();
    let mut group_buffer: Vec<String> = Vec::with_capacity(items_len - 1);
    let mut base = "";
    for (idx, item) in items.iter().enumerate() {
        if idx == items_len - 1 {
            base = item;
        } else {
            group_buffer.push(item.to_string());
        }
    }
    (group_buffer.join("/"), base.to_string())
}

pub fn current_time() -> Result<Epoch> {
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .context("system clock set to invalid time")?
        .as_secs();

    Ok(current_time)
}

pub fn expand_env(s: impl AsRef<str>) -> Result<String> {
    match shellexpand::full(s.as_ref()) {
        Ok(s) => Ok(s.to_string()),
        Err(err) => Err(err)
            .with_context(|| format!("could not expand env for {}", style(s.as_ref()).yellow())),
    }
}

pub fn confirm(msg: impl AsRef<str> + Into<String>) -> Result<()> {
    let result = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(msg)
        .interact_on(&Term::stderr());
    match result {
        Ok(ok) => {
            if !ok {
                bail!(SilentExit { code: 60 })
            }
            Ok(())
        }
        Err(err) => Err(err).context("could not do confirm prompt"),
    }
}

pub fn edit<S>(msg: S, ext: S, required: bool) -> Result<String>
where
    S: AsRef<str>,
{
    let mut editor = dialoguer::Editor::new();
    if !ext.as_ref().is_empty() {
        editor.extension(ext.as_ref());
    }
    let text = editor.edit(msg.as_ref()).context("unable to edit text")?;
    match text {
        Some(s) => {
            if required && s.is_empty() {
                bail!("edit content cannot be empty")
            }
            Ok(s)
        }
        None => bail!("you did not edit anything"),
    }
}

pub fn open_url(url: impl AsRef<str>) -> Result<()> {
    open::that(url.as_ref()).with_context(|| {
        format!(
            "unable to open url {} in default browser",
            style(url.as_ref()).yellow()
        )
    })
}

pub fn current_dir() -> Result<PathBuf> {
    env::current_dir().context("could not get current dir")
}

pub fn str_to_path(s: impl AsRef<str>) -> Result<PathBuf> {
    let path = PathBuf::from_str(s.as_ref())
        .with_context(|| format!("could not parse path {}", style(s.as_ref()).yellow()))?;
    fs::canonicalize(&path).with_context(|| {
        format!(
            "could not get absolute path for {}",
            style(s.as_ref()).yellow()
        )
    })
}

pub fn osstr_to_str<'a>(s: &'a OsStr) -> Result<&'a str> {
    match s.to_str() {
        Some(s) => Ok(s),
        None => bail!("could not parse path {}", PathBuf::from(s).display()),
    }
}

pub fn path_to_str<'a>(path: &'a PathBuf) -> Result<&'a str> {
    match path.to_str() {
        Some(path) => Ok(path),
        None => bail!("could not parse path: {}", path.display()),
    }
}

pub fn print_operation(s: impl AsRef<str>) {
    _ = writeln!(io::stderr(), "{} {}", style("==>").green(), s.as_ref());
}

pub fn option_arg<'a>(args: &'a Vec<String>) -> Option<&'a str> {
    if args.is_empty() {
        None
    } else {
        Some(args[0].as_str())
    }
}

const ERR_FZF_NOT_FOUND: &str = "could not find fzf, is it installed?";

pub struct Fzf(Child);

impl Fzf {
    pub fn build() -> Result<Fzf> {
        // TODO: support Windows
        let program = "fzf";
        let mut cmd = Command::new(program);
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped());

        match cmd.spawn() {
            Ok(child) => Ok(Fzf(child)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => bail!(ERR_FZF_NOT_FOUND),
            Err(err) => Err(err).context("could not launch fzf"),
        }
    }

    pub fn query<S>(&mut self, keys: &Vec<S>) -> Result<usize>
    where
        S: AsRef<str>,
    {
        let mut input = String::with_capacity(keys.len());
        for key in keys {
            input.push_str(key.as_ref());
            input.push_str("\n");
        }

        let handle = self.0.stdin.as_mut().unwrap();
        if let Err(err) = write!(handle, "{}", input) {
            return Err(err).context("could not write to fzf");
        }

        mem::drop(self.0.stdin.take());

        let mut stdout = self.0.stdout.take().unwrap();
        let mut output = String::new();
        stdout
            .read_to_string(&mut output)
            .context("failed to read from fzf")?;
        let output = output.trim();
        let status = self.0.wait().context("wait failed on fzf")?;
        match status.code() {
            Some(0) => match keys.iter().position(|s| s.as_ref() == output) {
                Some(idx) => Ok(idx),
                None => bail!("could not find key {}", output),
            },
            Some(1) => bail!("no match found"),
            Some(2) => bail!("fzf returned an error"),
            Some(130) => bail!(SilentExit { code: 130 }),
            Some(128..=254) | None => bail!("fzf was terminated"),
            _ => bail!("fzf returned an unknown error"),
        }
    }
}

pub struct Shell {
    cmd: Command,
    program: OsString,
    mute: bool,
}

impl Shell {
    pub fn new(name: impl AsRef<OsStr>) -> Shell {
        let mut cmd = Command::new(name.as_ref());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::inherit());
        cmd.stdin(Stdio::inherit());
        Shell {
            cmd,
            program: name.as_ref().to_os_string(),
            mute: false,
        }
    }

    pub fn git() -> Shell {
        Self::new("git")
    }

    pub fn bash(script: impl AsRef<str>) -> Shell {
        let mut shell = Self::new("bash");
        shell.arg("-c");
        // FIXME: We add `> /dev/stderr` at the end of the script to ensure that
        // the script does not output any content to stdout. This method is not
        // applicable to Windows and a more universal method is needed.
        let script = format!("{} > /dev/stderr", script.as_ref());
        shell.arg(script.as_str());
        // The raw command was changed, donot print anything, let caller
        // to print msg.
        shell.mute = true;
        shell
    }

    pub fn cmd_exists(name: impl AsRef<OsStr>) -> bool {
        let str = match name.as_ref().to_str() {
            Some(s) => s,
            None => return false,
        };
        let mut cmd = Command::new("bash");
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.stdin(Stdio::inherit());

        cmd.arg("-c");
        cmd.arg(format!("command -v {}", str));

        match cmd.output() {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    pub fn inherit(&mut self) -> &mut Self {
        self.cmd.stdout(Stdio::inherit());
        self
    }

    pub fn select_cmd<S, I>(names: I) -> Option<S>
    where
        S: AsRef<OsStr>,
        I: IntoIterator<Item = S>,
    {
        for name in names {
            if Self::cmd_exists(name.as_ref()) {
                return Some(name);
            }
        }
        None
    }

    pub fn edit_file(editor: &Option<String>, path: &PathBuf) -> Result<()> {
        let editor = match editor {
            Some(e) => e.as_str(),
            None => match Self::select_cmd(["nvim", "vim", "vi"]) {
                Some(e) => e,
                None => {
                    bail!("could not find valid editor in your machine, please config it manually")
                }
            },
        };

        let mut cmd = Command::new(editor);
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());
        cmd.stdin(Stdio::inherit());
        cmd.arg(path.display().to_string());

        match cmd.output() {
            Ok(_) => Ok(()),
            Err(_) => bail!(SilentExit { code: 101 }),
        }
    }

    pub fn with_path(&mut self, path: &PathBuf) -> &mut Self {
        self.cmd.current_dir(path);
        self
    }

    pub fn with_git_path<S>(&mut self, path: S) -> &mut Self
    where
        S: AsRef<str>,
    {
        self.args(["-C", path.as_ref()]);
        self
    }

    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.cmd.args(args);
        self
    }

    pub fn arg<S>(&mut self, arg: S) -> &mut Self
    where
        S: AsRef<OsStr>,
    {
        self.cmd.arg(arg);
        self
    }

    pub fn env<K, V>(&mut self, key: K, val: V) -> &mut Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.cmd.env(key, val);
        self
    }

    pub fn exec(&mut self) -> Result<String> {
        let program = osstr_to_str(&self.program)?;
        self.print_cmd(program)?;
        let mut child = match self.cmd.spawn() {
            Ok(child) => child,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                bail!("could not find command {}", "")
            }
            Err(e) => return Err(e).with_context(|| format!("could not launch {}", "")),
        };

        let mut output = String::new();

        if let Some(mut stdout) = child.stdout.take() {
            stdout
                .read_to_string(&mut output)
                .with_context(|| format!("failed to read from {}", program))?;
        }

        let status = child
            .wait()
            .with_context(|| format!("failed to wait for {}", program))?;
        match status.code() {
            Some(0) => Ok(output.trim().to_string()),
            _ => bail!(SilentExit { code: 101 }),
        }
    }

    fn print_cmd(&self, program: &str) -> Result<()> {
        if self.mute {
            return Ok(());
        }
        let args = self.cmd.get_args();
        let args: Vec<&OsStr> = args.collect();
        let mut strs: Vec<&str> = Vec::with_capacity(args.len() + 1);
        strs.push(program);
        for arg in &args {
            let str = match arg.to_str() {
                Some(s) => s,
                None => continue,
            };
            strs.push(str);
        }
        let cmd_str = strs.join(" ");
        _ = writeln!(
            io::stderr(),
            "{} {}",
            style("==>").cyan(),
            style(cmd_str).bold()
        );
        Ok(())
    }
}

pub enum BranchStatus {
    Sync,
    Gone,
    Ahead,
    Behind,
    Conflict,
    Detached,
}

impl BranchStatus {
    pub fn display(&self) -> StyledObject<&'static str> {
        match self {
            Self::Sync => style("sync").green(),
            Self::Gone => style("gone").red(),
            Self::Ahead => style("ahead").yellow(),
            Self::Behind => style("behind").yellow(),
            Self::Conflict => style("conflict").yellow().bold(),
            Self::Detached => style("detached").red(),
        }
    }
}

pub struct GitBranch {
    pub name: String,
    pub status: BranchStatus,

    pub current: bool,
}

impl GitBranch {
    const BRANCH_REGEX: &str = r"^(\*)*[ ]*([^ ]*)[ ]*([^ ]*)[ ]*(\[[^\]]*\])*[ ]*(.*)$";
    const HEAD_BRANCH_PREFIX: &str = "HEAD branch:";

    pub fn list() -> Result<Vec<GitBranch>> {
        let re = Regex::new(Self::BRANCH_REGEX).expect("parse git branch regex");
        let mut git = Shell::git();
        git.args(["branch", "-vv"]);

        let output = git.exec().context("unable to execute git branch command")?;
        let lines: Vec<&str> = output.split("\n").collect();
        let mut branches: Vec<GitBranch> = Vec::with_capacity(lines.len());
        for line in lines {
            let branch = Self::parse(&re, line)?;
            branches.push(branch);
        }

        Ok(branches)
    }

    pub fn default() -> Result<String> {
        Self::default_by_remote("origin")
    }

    pub fn default_by_remote(remote: &str) -> Result<String> {
        print_operation(format!(
            "try to get default branch for {}",
            style(remote).yellow()
        ));
        let head_ref = format!("refs/remotes/{}/HEAD", remote);
        let remote_ref = format!("refs/remotes/{}/", remote);

        let mut git = Shell::git();
        git.args(["symbolic-ref", &head_ref]);
        if let Ok(out) = git.exec() {
            if out.is_empty() {
                bail!("default branch is empty")
            }
            return match out.strip_prefix(&remote_ref) {
                Some(s) => Ok(s.to_string()),
                None => bail!("invalid ref output by git: {}", style(out).yellow()),
            };
        }
        // If failed, user might not switch to this branch yet, let's
        // use "git show <remote>" instead to get default branch.
        let mut git = Shell::git();
        git.args(["remote", "show", remote]);
        let output = git.exec()?;
        let lines: Vec<&str> = output.split("\n").collect();
        for line in lines {
            if let Some(branch) = line.trim().strip_prefix(Self::HEAD_BRANCH_PREFIX) {
                let branch = branch.trim();
                if branch.is_empty() {
                    bail!("default branch returned by git remote show is empty")
                }
                return Ok(branch.to_string());
            }
        }

        bail!("no default branch returned by git remote show, please check your git command")
    }

    pub fn current() -> Result<String> {
        Shell::git().args(["branch", "--show-current"]).exec()
    }

    pub fn ensure_no_uncommitted() -> Result<()> {
        let mut git = Shell::git();
        git.args(["status", "-s"]);
        let output = git.exec()?;
        if !output.is_empty() {
            let lines: Vec<&str> = output.split("\n").collect();
            let (word, call) = if lines.len() == 1 {
                ("change", "it")
            } else {
                ("changes", "them")
            };
            bail!(
                "you have {} uncommitted {}, please handle {} first",
                lines.len(),
                word,
                call
            )
        }
        Ok(())
    }

    fn parse(re: &Regex, line: impl AsRef<str>) -> Result<GitBranch> {
        let parse_err = format!(
            "invalid branch description {}, please check your git command",
            style(line.as_ref()).yellow()
        );
        let mut iter = re.captures_iter(line.as_ref());
        let caps = match iter.next() {
            Some(caps) => caps,
            None => bail!(parse_err),
        };
        // We have 6 captures:
        //   0 -> line itself
        //   1 -> current branch
        //   2 -> branch name
        //   3 -> commit id
        //   4 -> remote description
        //   5 -> commit message
        if caps.len() != 6 {
            bail!(parse_err)
        }
        let mut current = false;
        if let Some(_) = caps.get(1) {
            current = true;
        }

        let name = match caps.get(2) {
            Some(name) => name.as_str().trim(),
            None => bail!("{}: missing name", parse_err),
        };

        let status = match caps.get(4) {
            Some(remote_desc) => {
                let remote_desc = remote_desc.as_str();
                let behind = remote_desc.contains("behind");
                let ahead = remote_desc.contains("ahead");

                if remote_desc.contains("gone") {
                    BranchStatus::Gone
                } else if ahead && behind {
                    BranchStatus::Conflict
                } else if ahead {
                    BranchStatus::Ahead
                } else if behind {
                    BranchStatus::Behind
                } else {
                    BranchStatus::Sync
                }
            }
            None => BranchStatus::Detached,
        };

        Ok(GitBranch {
            name: name.to_string(),
            status,
            current,
        })
    }
}

pub struct GitRemote(String);

impl GitRemote {
    pub fn list() -> Result<Vec<GitRemote>> {
        let output = Shell::git().arg("remote").exec()?;
        let remotes: Vec<GitRemote> = output
            .split("\n")
            .map(|s| GitRemote(s.to_string()))
            .collect();
        Ok(remotes)
    }

    pub fn build(upstream: bool) -> Result<GitRemote> {
        if !upstream {
            return Ok(GitRemote(String::from("origin")));
        }

        let remotes = Self::list()?;
        let upstream_remote = remotes
            .into_iter()
            .find(|remote| remote.0.as_str() == "upstream");
        if let Some(remote) = upstream_remote {
            return Ok(remote);
        }

        let db = Database::open()?;
        let config = Config::parse()?;
        let repo = db.current(&config.workspace)?;
        let remote_config = config.must_get_remote(&repo.remote)?;
        let provider = api::create_provider(&remote_config)?;

        print_operation(format!(
            "provider: try to get upstream for {}",
            style(&repo.name).yellow()
        ));
        let upstream = provider.get_upstream(&repo.name)?;

        let clone = match &remote_config.clone {
            Some(clone) => clone,
            None => bail!("require clone config for remote, please check your config"),
        };

        let upstream_repo = Repo {
            remote: remote_config.name.clone(),
            name: upstream,
            path: String::new(),
            last_accessed: 0,
            accessed: 0.0,
        };
        let url = upstream_repo.clone_url(clone);

        confirm(format!(
            "do you want to set upstream to {}",
            style(&url).yellow()
        ))?;
        Shell::git()
            .args(["remote", "add", "upstream", url.as_str()])
            .exec()?;
        Ok(GitRemote(String::from("upstream")))
    }

    pub fn target(&self, branch: Option<&str>) -> Result<String> {
        let (target, branch) = match branch {
            Some(branch) => (format!("{}/{}", self.0, branch), branch.to_string()),
            None => {
                let branch = GitBranch::default_by_remote(&self.0)?;
                (format!("{}/{}", self.0, branch), branch)
            }
        };
        Shell::git()
            .args(["fetch", self.0.as_str(), branch.as_str()])
            .exec()?;
        Ok(target)
    }
}

pub struct GitTag(String);

impl Display for GitTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl GitTag {
    const NUM_REGEX: &str = r"\d+";
    const PLACEHOLDER_REGEX: &str = r"\{(\d+|%[yYmMdD])(\+)*}";

    pub fn from_str(s: impl AsRef<str>) -> GitTag {
        GitTag(s.as_ref().to_string())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn list() -> Result<Vec<GitTag>> {
        let tags: Vec<_> = Shell::git()
            .arg("tag")
            .exec()?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| GitTag(line.trim().to_string()))
            .collect();
        Ok(tags)
    }

    pub fn latest() -> Result<GitTag> {
        Shell::git()
            .args(["fetch", "origin", "--prune-tags"])
            .exec()?;
        let output = Shell::git()
            .args(["describe", "--tags", "--abbrev=0"])
            .exec()?;
        let tag = output.trim().to_string();
        if tag.is_empty() {
            bail!("no latest tag")
        }
        Ok(GitTag(tag))
    }

    pub fn apply_rule(&self, rule: impl AsRef<str>) -> Result<GitTag> {
        let num_re = Regex::new(Self::NUM_REGEX).context("unable to parse num regex")?;
        let ph_re = Regex::new(Self::PLACEHOLDER_REGEX)
            .context("unable to parse rule placeholder regex")?;

        let nums: Vec<i32> = num_re
            .captures_iter(self.0.as_str())
            .map(|caps| {
                caps.get(0)
                    .unwrap()
                    .as_str()
                    .parse()
                    // The caps here MUST be number, so it is safe to use expect here
                    .expect("unable to parse num caps")
            })
            .collect();

        let mut with_date = false;
        let result = ph_re.replace_all(rule.as_ref(), |caps: &Captures| {
            let rep = caps.get(1).unwrap().as_str();
            let idx: usize = match rep.parse() {
                Ok(idx) => idx,
                Err(_) => {
                    with_date = true;
                    return rep.to_string();
                }
            };
            let num = if idx >= nums.len() { 0 } else { nums[idx] };
            let plus = caps.get(2);
            match plus {
                Some(_) => format!("{}", num + 1),
                None => format!("{}", num),
            }
        });
        let mut result = result.to_string();
        if with_date {
            let date = Local::now();
            result = date.format(&result).to_string();
        }

        Ok(GitTag(result))
    }
}

pub struct EmptyDir {
    path: PathBuf,
    subs: Vec<EmptyDir>,

    empty: bool,
    keep: bool,
}

impl EmptyDir {
    pub fn scan<S>(path: S, exclude: &Vec<PathBuf>) -> Result<EmptyDir>
    where
        S: AsRef<str>,
    {
        let mut exclude_set: HashSet<&PathBuf> = HashSet::with_capacity(exclude.len());
        for s in exclude {
            exclude_set.insert(&s);
        }

        let path = PathBuf::from_str(path.as_ref()).context("invalid scan path")?;
        let mut root = EmptyDir {
            path,
            subs: vec![],
            empty: false,
            keep: true,
        };
        root.walk(&exclude_set)?;
        root.mark();
        root.keep = true;
        Ok(root)
    }

    fn walk(&mut self, exclude: &HashSet<&PathBuf>) -> Result<()> {
        let subs = match fs::read_dir(&self.path) {
            Ok(dir) => dir,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("could not read dir {}", self.path.display()));
            }
        };
        for sub in subs {
            let sub = sub.context("could not read sub directory")?;
            let meta = sub
                .metadata()
                .context("could not read meta data for sub directory")?;
            if !meta.is_dir() {
                self.keep = true;
                continue;
            }
            if sub.file_name() == ".git" {
                self.keep = true;
                continue;
            }
            let sub_path = self.path.join(sub.file_name());
            if let Some(_) = exclude.get(&sub_path) {
                self.keep = true;
                continue;
            }
            let mut sub_dir = EmptyDir {
                path: sub_path,
                subs: vec![],
                empty: false,
                keep: false,
            };
            sub_dir.walk(exclude)?;
            self.subs.push(sub_dir);
        }
        if self.subs.is_empty() {
            self.empty = true;
            return Ok(());
        }
        Ok(())
    }

    fn mark(&mut self) {
        if self.subs.is_empty() {
            return;
        }
        for sub in &mut self.subs {
            sub.mark();
            if !sub.empty {
                self.empty = false;
            }
        }
    }

    pub fn list<'a>(&'a self, list: &mut Vec<&'a OsStr>) {
        if self.empty && !self.keep {
            list.push(self.path.as_os_str());
            return;
        }
        for sub in &self.subs {
            sub.list(list);
        }
    }

    pub fn clean(&self) -> Result<()> {
        if self.empty && !self.keep {
            return fs::remove_dir(&self.path).with_context(|| {
                format!("could not remove empty directory {}", self.path.display())
            });
        }
        for sub in &self.subs {
            if let Err(err) = sub.clean() {
                return Err(err);
            }
        }
        Ok(())
    }
}
